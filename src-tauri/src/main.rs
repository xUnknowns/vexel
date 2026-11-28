#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![recursion_limit = "256"]

mod batch;
mod brand;
mod layers;
mod mcp;
mod storage;
mod templates;
mod tools;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("--mcp") {
        mcp::run_mcp_stdio();
        return;
    }

    let project_state: layers::SharedProjectState =
        std::sync::Arc::new(std::sync::Mutex::new(layers::ProjectState::default()));
    let state_for_server = project_state.clone();
    let state_for_close = project_state.clone();

    tauri::Builder::default()
        .manage(project_state)
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(move |_window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                // Flush autosave and mark clean shutdown
                if let Ok(s) = state_for_close.lock() {
                    if s.has_project {
                        if let Ok(dir) = storage::ensure_subdir("autosave") {
                            if let Ok(json) = serde_json::to_string(&s.current) {
                                let _ = storage::atomic_write_str(&dir.join("autosave.veltrax"), &json);
                            }
                        }
                        if let Some(ref p) = s.project_file_path {
                            if s.dirty {
                                if let Ok(json) = serde_json::to_string_pretty(&s.current) {
                                    let _ = storage::atomic_write_str(std::path::Path::new(p), &json);
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
