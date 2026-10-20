mod commands;
pub mod convert;
pub mod edit;
mod radial;
mod tools;
#[cfg(windows)]
mod shift_drag;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(radial::Pending::default())
        .manage(tools::Pending::default())
        .setup(|app| {
            #[cfg(windows)]
            shift_drag::start(app.handle().clone());

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
            commands::open_actions,
            commands::take_radial_payload,
            commands::close_radial,
            commands::open_tool,
            commands::take_tool_payload,
            commands::read_image,
            commands::crop_image,
            commands::compress_files,
            commands::compress_preview,
            commands::file_sizes,
            commands::save_edited,
            commands::read_media,
            commands::audio_peaks,
            commands::audio_preview,
            commands::trim_audio,
            commands::log_frontend_error,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Morph");
}
