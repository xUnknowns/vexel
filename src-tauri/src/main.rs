#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod layers;
mod mcp;
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

    tauri::Builder::default()
        .manage(project_state)
        .plugin(tauri_plugin_dialog::init())
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
            tools::load_project_file,
            tools::export_configured,
            // Layer operations
            tools::add_image_overlay_layer,
            tools::remove_layer_background_color,
            tools::add_text_layer,
            tools::add_shape_layer,
            tools::update_layer_transform,
            tools::update_layer_effects,
            tools::set_layer_outline,
            tools::reorder_layer,
            tools::toggle_layer_visibility,
            tools::toggle_layer_lock,
            tools::duplicate_layer,
            tools::delete_layer,
            tools::undo_action,
            tools::redo_action,
            // MCP
            mcp::connect_claude_desktop,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Veltrax Studios");
}
