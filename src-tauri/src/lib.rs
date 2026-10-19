mod commands;
pub mod convert;
mod radial;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(radial::Pending::default())
        .setup(|app| {
            // `morph fichier1 fichier2…` ouvre directement le menu des formats
            // (utilisé par « Ouvrir avec Morph » sous Linux, et pratique pour tester).
            let files: Vec<String> = std::env::args()
                .skip(1)
                .filter(|a| std::path::Path::new(a).is_file())
                .collect();
            if !files.is_empty() {
                radial::show(app.handle(), files)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_targets,
            commands::convert_files,
            commands::open_radial,
            commands::take_radial_payload,
            commands::close_radial,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Morph");
}
