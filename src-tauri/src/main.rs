#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod mcp;
mod tools;

fn main() {
    // ── MCP stdio mode ───────────────────────────────────────────────────
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|s| s.as_str()) == Some("--mcp") {
        mcp::run_mcp_stdio();
        return;
    }

    // ── Tauri GUI mode ───────────────────────────────────────────────────
    let editor_state: mcp::SharedEditorState =
        std::sync::Arc::new(std::sync::Mutex::new(mcp::EditorState::default()));
    let state_for_server = editor_state.clone();

    tauri::Builder::default()
        .manage(editor_state)
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let handle = app.handle().clone();
            mcp::start_internal_server(state_for_server, handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Image tools
            tools::get_image_dimensions,
            tools::create_blank_canvas,
            tools::adjust_brightness_contrast_saturation,
            tools::transform_image,
            tools::crop_image,
            tools::resize_image,
            tools::apply_filter,
            tools::export_file,
            tools::export_image_configured,
            tools::add_text_layer,
            tools::get_canvas_preview,
            // MCP / state
            mcp::update_editor_state,
            mcp::get_editor_state_info,
            mcp::connect_claude_desktop,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Veltrax Studios");
}
