#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![recursion_limit = "256"]

mod batch;
mod brand;
mod layers;
mod mcp;
mod storage;
mod support;
mod templates;
mod tools;

fn cli_usage() -> &'static str {
    "Veltrax Studios headless usage:\n\
     veltrax-studios.exe --render <template-id|name|.vtemplate> --out <file> [--vars <json>] [--format png|jpeg|webp] [--quality 1-100] [--size WIDTHxHEIGHT]\n\
     veltrax-studios.exe --batch <template-id|name|.vtemplate> --data <csv|json> --out <folder> [--pattern <name-pattern>] [--format png|jpeg|webp] [--quality 1-100]\n\
     veltrax-studios.exe --mcp  (MCP stdio server)"
}

fn parse_cli_options(args: &[String]) -> Result<std::collections::HashMap<String, String>, String> {
    let mut options = std::collections::HashMap::new();
    let mut index = 0;
    while index < args.len() {
        let flag = &args[index];
        if !flag.starts_with("--") {
            return Err(format!("Unexpected argument '{}'.\n{}", flag, cli_usage()));
        }
        let value = args
            .get(index + 1)
            .filter(|value| !value.starts_with("--"))
            .ok_or_else(|| format!("Missing value for '{}'.\n{}", flag, cli_usage()))?;
        if options.insert(flag.clone(), value.clone()).is_some() {
            return Err(format!("Option '{}' was provided more than once.", flag));
        }
        index += 2;
    }
    Ok(options)
}

fn required_option<'a>(
    options: &'a std::collections::HashMap<String, String>,
    name: &str,
) -> Result<&'a str, String> {
    options
        .get(name)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("Missing required option '{}'.\n{}", name, cli_usage()))
}

fn parse_cli_size(raw: &str) -> Result<(u32, u32), String> {
    let (width, height) = raw
        .split_once('x')
        .or_else(|| raw.split_once('X'))
        .ok_or_else(|| "Size must be written WIDTHxHEIGHT, e.g. 1280x720.".to_string())?;
    let width = width
        .parse::<u32>()
        .map_err(|_| "Invalid width in --size.".to_string())?;
    let height = height
        .parse::<u32>()
        .map_err(|_| "Invalid height in --size.".to_string())?;
    if width == 0
        || height == 0
        || width > 16_384
        || height > 16_384
        || (width as u64) * (height as u64) > 100_000_000
    {
        return Err("Size must be between 1 and 16384 px per side and at most 100 MP.".to_string());
    }
    Ok((width, height))
}

fn load_cli_values(
    path: Option<&String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    let Some(path) = path else {
        return Ok(std::collections::HashMap::new());
    };
    let raw =
        std::fs::read_to_string(path).map_err(|e| format!("Could not read vars JSON: {}", e))?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("Invalid vars JSON: {}", e))?;
    let object = value
        .as_object()
        .ok_or_else(|| "Vars JSON must be an object with variable names as keys.".to_string())?;
    Ok(object
        .iter()
        .map(|(key, value)| {
            let text = match value {
                serde_json::Value::String(text) => text.clone(),
                serde_json::Value::Null => String::new(),
                other => other.to_string(),
            };
            (key.clone(), text)
        })
        .collect())
}

fn cli_format(options: &std::collections::HashMap<String, String>, out: &str) -> String {
    options
        .get("--format")
        .cloned()
        .or_else(|| {
            std::path::Path::new(out)
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "png".to_string())
}

fn run_headless_cli(args: &[String]) -> Result<(), String> {
    if args.first().map(String::as_str) == Some("--help")
        || args.first().map(String::as_str) == Some("-h")
    {
        println!("{}", cli_usage());
        return Ok(());
    }
    let action = args
        .first()
        .map(String::as_str)
        .ok_or_else(|| cli_usage().to_string())?;
    let template_ref = args
        .get(1)
        .filter(|value| !value.starts_with("--"))
        .ok_or_else(|| format!("Missing template after '{}'.\n{}", action, cli_usage()))?;
    let options = parse_cli_options(&args[2..])?;
    let out = required_option(&options, "--out")?;
    let quality = options
        .get("--quality")
        .map(|value| {
            value
                .parse::<u8>()
                .map_err(|_| "--quality must be an integer from 1 to 100.".to_string())
        })
        .transpose()?
        .unwrap_or(90);
    if !(1..=100).contains(&quality) {
        return Err("--quality must be from 1 to 100.".to_string());
    }

    match action {
        "--render" => {
            if let Some(unknown) = options.keys().find(|key| {
                !["--out", "--vars", "--format", "--quality", "--size"].contains(&key.as_str())
            }) {
                return Err(format!("Unknown option '{}' for --render.", unknown));
            }
            let vars = load_cli_values(options.get("--vars"))?;
            let size = options
                .get("--size")
                .map(|value| parse_cli_size(value))
                .transpose()?;
            let format = cli_format(&options, out);
            let output = std::path::Path::new(out);
            if let Some(parent) = output
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            batch::cli_render_template(template_ref, &vars, output, &format, quality, size)?;
            println!("Rendered image: {}", output.display());
            Ok(())
        }
        "--batch" => {
            if let Some(unknown) = options.keys().find(|key| {
                !["--out", "--data", "--pattern", "--format", "--quality"].contains(&key.as_str())
            }) {
                return Err(format!("Unknown option '{}' for --batch.", unknown));
            }
            let data_path = required_option(&options, "--data")?;
            let raw = std::fs::read_to_string(data_path)
                .map_err(|e| format!("Could not read batch data: {}", e))?;
            let rows = batch::parse_batch_rows(&raw)?;
            let format = cli_format(&options, "output.png");
            let pattern = options
                .get("--pattern")
                .map(String::as_str)
                .unwrap_or("{index}");
            let summary = batch::cli_batch_render(
                template_ref,
                &rows,
                std::path::Path::new(out),
                pattern,
                &format,
                quality,
            )?;
            for path in &summary.saved {
                println!("Saved: {}", path);
            }
            for error in &summary.errors {
                eprintln!("Error: {}", error);
            }
            if !summary.errors.is_empty() {
                return Err(format!(
                    "Batch completed with {} error(s).",
                    summary.errors.len()
                ));
            }
            println!("Batch complete: {} image(s).", summary.saved.len());
            Ok(())
        }
        _ => Err(format!(
            "Unknown headless option '{}'.\n{}",
            action,
            cli_usage()
        )),
    }
}

#[cfg(windows)]
fn attach_parent_console() {
    #[link(name = "kernel32")]
    extern "system" {
        fn AttachConsole(process_id: u32) -> i32;
    }
    const ATTACH_PARENT_PROCESS: u32 = u32::MAX;
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(windows))]
fn attach_parent_console() {}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let project_state: layers::SharedProjectState =
        std::sync::Arc::new(std::sync::Mutex::new(layers::ProjectState::default()));
    support::install_global_panic_hook(project_state.clone());
    support::append_app_log(
        "INFO",
        &format!("Veltrax Studios {} started.", env!("CARGO_PKG_VERSION")),
    );
    if args.get(1).map(|s| s.as_str()) == Some("--mcp") {
        mcp::run_mcp_stdio();
        return;
    }
    if args.get(1).map(|s| s.starts_with("--")).unwrap_or(false) {
        attach_parent_console();
        match run_headless_cli(&args[1..]) {
            Ok(()) => std::process::exit(0),
            Err(error) => {
                eprintln!("{}", error);
                std::process::exit(1);
            }
        }
    }

    let state_for_server = project_state.clone();
    let state_for_close = project_state.clone();

    tauri::Builder::default()
        .manage(project_state)
        // Updater extension point: register tauri-plugin-updater here after configuring its feed and signing keys.
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(move |_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Flush autosave and mark clean shutdown
                if let Ok(s) = state_for_close.lock() {
                    if s.has_project {
                        if let Ok(dir) = storage::ensure_subdir("autosave") {
                            if let Ok(json) = serde_json::to_string(&s.current) {
                                let _ =
                                    storage::atomic_write_str(&dir.join("autosave.veltrax"), &json);
                            }
                        }
                        if let Some(ref p) = s.project_file_path {
                            if s.dirty {
                                if let Ok(json) = serde_json::to_string_pretty(&s.current) {
                                    let _ =
                                        storage::atomic_write_str(std::path::Path::new(p), &json);
                                }
                            }
                        }
                    }
                }
                storage::clear_session_lock();
            }
        })
        .setup(move |app| {
            let handle = app.handle().clone();
            mcp::start_internal_server(state_for_server, handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Project & state
            tools::get_image_dimensions,
            tools::set_editor_active,
            tools::get_project_state,
            tools::rename_project,
            tools::create_project,
            tools::open_base_image,
            tools::save_project_file,
            tools::save_current_project,
            tools::load_project_file,
            tools::check_missing_assets,
            tools::export_configured,
            tools::export_batch,
            batch::export_multi_size_async,
            batch::batch_render_template,
            batch::cancel_batch_export,
            tools::trim_transparency,
            tools::save_autosave,
            tools::check_autosave,
            tools::recover_autosave,
            tools::clear_autosave,
            storage::get_app_settings,
            storage::save_app_settings,
            support::get_app_version,
            support::get_feedback_diagnostics,
            support::open_feedback_site,
            support::record_local_error,
            storage::get_recent_projects,
            storage::remove_recent_project,
            storage::relink_recent_project,
            storage::export_user_data_zip,
            storage::import_user_data_zip,
            tools::get_default_projects_dir,
            tools::get_user_library,
            tools::save_to_user_library,
            tools::delete_from_user_library,
            tools::get_brand_kit,
            tools::save_brand_kit,
            brand::list_brand_kits,
            brand::save_brand_kit_profile,
            brand::delete_brand_kit_profile,
            brand::set_active_brand_kit,
            brand::add_brand_logo,
            tools::import_font,
            tools::list_imported_fonts,
            tools::delete_imported_font,
            // Canvas background & resize
            tools::set_canvas_background,
            tools::resize_canvas,
            // Layer operations
            tools::push_undo_checkpoint,
            tools::add_image_overlay_layer,
            tools::replace_image_layer,
            tools::remove_layer_background_color,
            tools::add_text_layer,
            tools::update_text_layer,
            tools::update_text_advanced,
            tools::crop_canvas,
            tools::add_shape_layer,
            tools::add_shape_advanced,
            tools::update_shape_layer,
            tools::add_gradient_layer,
            tools::add_watermark_layer,
            tools::update_layer_transform,
            tools::transform_layers,
            tools::flip_layer,
            tools::update_layer_effects,
            tools::set_layer_outline,
            tools::update_layer_shadow,
            tools::update_layer_glow,
            tools::update_layer_mask,
            tools::set_clip_to_below,
            tools::render_drag_layers,
            tools::magic_wand_mask,
            tools::lasso_mask,
            tools::brush_mask_stroke,
            tools::reset_alpha_mask,
            tools::ai_remove_background,
            tools::reorder_layer,
            tools::toggle_layer_visibility,
            tools::toggle_layer_lock,
            tools::duplicate_layer,
            tools::duplicate_layers,
            tools::delete_layer,
            tools::delete_layers,
            tools::paste_layers_from_json,
            tools::add_image_bytes_layer,
            tools::group_layers,
            tools::ungroup_layers,
            tools::toggle_group_visibility,
            tools::toggle_group_lock,
            tools::align_layers,
            tools::distribute_layers,
            tools::undo_action,
            tools::redo_action,
            tools::list_project_templates,
            tools::save_as_template,
            tools::delete_template,
            tools::apply_template_to_canvas,
            templates::list_all_templates,
            templates::save_current_as_vtemplate,
            templates::preview_vtemplate_filled,
            templates::create_project_from_vtemplate,
            templates::export_vtemplate_file,
            templates::import_vtemplate_file,
            templates::delete_vtemplate,
            tools::add_smart_text_layer,
            tools::list_brand_sizes,
            tools::export_all_brand_sizes,
            tools::get_project_snapshot_json,
            tools::get_layer_thumbnails,
            tools::get_history_info,
            // MCP
            mcp::connect_claude_desktop,
            mcp::check_mcp_real_status,
            mcp::scan_desktop_ai_clients,
            mcp::connect_ai_client,
            mcp::disconnect_ai_client,
            mcp::connect_all_detected,
            mcp::get_mcp_snippets,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Veltrax Studios");
}
