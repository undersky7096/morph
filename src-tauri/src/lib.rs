mod commands;
pub mod convert;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::get_targets, commands::convert_files])
        .run(tauri::generate_context!())
        .expect("error while running Morph");
}
