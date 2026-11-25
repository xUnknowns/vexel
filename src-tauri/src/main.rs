#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tools;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            tools::adjust_brightness_contrast,
            tools::transform_image,
            tools::crop_image,
            tools::resize_image,
            tools::apply_filter,
            tools::export_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Vexel");
}
