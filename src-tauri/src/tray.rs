//! Icône de la zone de notification : clic = Réglages, menu = Réglages / Quitter.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

/// La fenêtre principale sert de fenêtre de Réglages.
pub const SETTINGS: &str = "main";
pub const ID: &str = "morph";

/// Ouvre les Réglages. La fenêtre n'existe que tant qu'elle est ouverte (créée ici, détruite
/// à la fermeture) : Morph au repos dans la zone de notification reste léger. Une fenêtre
/// créée s'affiche quand son interface est prête (`reveal_settings`), sans flash blanc.
pub fn show_settings(app: &AppHandle) -> tauri::Result<()> {
    show_settings_at(app, None)
}

/// Ouvre les Réglages sur une section (`#about`…), ou sur la dernière affichée.
pub fn show_settings_at(app: &AppHandle, section: Option<&str>) -> tauri::Result<()> {
    if app.get_webview_window(SETTINGS).is_some() {
        if let Some(section) = section {
            app.emit_to(SETTINGS, "settings-section", section)?;
        }
        return reveal_settings(app);
    }
    let url = section.map_or_else(|| "index.html".to_string(), |s| format!("index.html#{s}"));
    let window = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App(url.into()))
        .title("Réglages · Morph")
        .inner_size(880.0, 640.0)
        .min_inner_size(720.0, 520.0)
        .center()
        .visible(false)
        .build()?;
    crate::icon::apply_to(&window);
    Ok(())
}

pub fn reveal_settings(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(SETTINGS) {
        // Afficher puis restaurer : au premier affichage, Windows applique le mode demandé par
        // le programme qui a lancé Morph (l'installeur le lance réduit).
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
    }
    Ok(())
}

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let settings = MenuItem::with_id(app, "settings", "Réglages", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quitter Morph", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&settings, &PredefinedMenuItem::separator(app)?, &quit])?;

    let mut builder = TrayIconBuilder::with_id(ID)
        .tooltip("Morph")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "settings" => {
                let _ = show_settings(app);
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let _ = show_settings(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}
