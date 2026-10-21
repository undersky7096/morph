mod commands;
pub mod convert;
pub mod edit;
mod icon;
mod jobs;
mod linux;
mod notify;
mod progress;
mod radial;
mod settings;
mod tools;
mod tray;
mod updates;
#[cfg(windows)]
mod shift_drag;

use tauri::{AppHandle, Manager, WindowEvent};

/// Lancement par l'ouverture de session : Morph reste discret dans la zone de notification.
const HIDDEN_ARG: &str = "--hidden";

/// Arguments de la ligne de commande (premier lancement, ou relayés par une seconde instance) :
/// - `morph fichier1 fichier2…` ouvre le menu des formats (« Ouvrir avec Morph » sous Linux) ;
/// - `morph --edit|--crop|--compress fichiers…` ouvre directement l'outil ;
/// - `morph` seul ouvre les Réglages ; `morph --hidden` (démarrage de session) ne montre rien.
fn handle_args(app: &AppHandle, args: &[String]) -> tauri::Result<()> {
    let tool = args.iter().find_map(|a| match a.as_str() {
        "--edit" => Some("edit"),
        "--crop" => Some("crop"),
        "--compress" => Some("compress"),
        _ => None,
    });
    let files: Vec<String> = args.iter().filter(|a| std::path::Path::new(a).is_file()).cloned().collect();
    if !files.is_empty() {
        match tool {
            Some(tool) => tools::open(app, tool.to_string(), files),
            None => radial::show(app, files),
        }
    } else if !args.iter().any(|a| a == HIDDEN_ARG) || !settings::get(app).onboarded {
        // Les Réglages, ou l'écran d'accueil tant qu'il n'a pas été vu.
        tray::show_settings(app)
    } else {
        Ok(())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Lancée par l'utilisateur (double-clic, « Ouvrir avec »), cette instance a le droit de
    // passer au premier plan : elle le cède à l'instance déjà ouverte qui affichera le menu.
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{ASFW_ANY, AllowSetForegroundWindow};
        let _ = AllowSetForegroundWindow(ASFW_ANY);
    }

    tauri::Builder::default()
        // En premier : une seconde instance transmet ses arguments à la première, puis se ferme
        // (un seul hook Shift+glisser, une seule icône).
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let args: Vec<String> = argv.into_iter().skip(1).collect();
            if let Err(e) = handle_args(app, &args) {
                eprintln!("[morph] arguments relayés : {e}");
            }
        }))
        .plugin(tauri_plugin_autostart::Builder::new().args([HIDDEN_ARG]).build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(radial::Pending::default())
        .manage(tools::Pending::default())
        .manage(settings::State::default())
        .manage(jobs::Jobs::default())
        .manage(jobs::Tasks::default())
        .manage(updates::State::default())
        .setup(|app| {
            settings::load(app.handle());
            if let Ok(dir) = app.path().app_local_data_dir() {
                edit::cutout::set_models_dir(dir.join("models"));
            }
            if let Ok(dir) = app.path().resource_dir() {
                edit::cutout::set_runtime(dir.join(edit::cutout::RUNTIME_LIB));
                convert::pdf::set_library(dir.join(convert::pdf::LIBRARY));
            }
            convert::engines::warm_up();
            tray::create(app.handle())?;
            icon::refresh(app.handle());
            #[cfg(windows)]
            shift_drag::start(app.handle().clone());
            updates::start(app.handle());

            let args: Vec<String> = std::env::args().skip(1).collect();
            handle_args(app.handle(), &args)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Thème du système changé : l'icône suit (réglage « Système »).
            if let WindowEvent::ThemeChanged(_) = event {
                icon::refresh(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_targets,
            commands::format_catalog,
            commands::start_conversion,
            commands::list_jobs,
            commands::cancel_job,
            commands::retry_job,
            commands::skip_conflicts,
            commands::clear_jobs,
            commands::detach_job,
            commands::show_progress,
            commands::cancel_task,
            commands::reveal,
            commands::reveal_settings,
            commands::update_status,
            commands::check_update,
            commands::install_update,
            commands::open_link,
            commands::open_radial,
            commands::open_actions,
            commands::open_tools,
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
            commands::cutout_image,
            commands::cutout_download,
            commands::read_media,
            commands::audio_peaks,
            commands::audio_preview,
            commands::trim_audio,
            commands::allow_media,
            commands::video_preview,
            commands::video_info,
            commands::edit_video,
            commands::get_settings,
            commands::set_settings,
            commands::quit,
            commands::platform,
            commands::system_integrated,
            commands::set_system_integration,
            commands::log_frontend_error,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Morph");
}
