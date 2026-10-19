//! Fenêtre du menu radial : transparente, sans bordure, affichée centrée sous le curseur.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition};

use crate::convert::formats::{self, TargetMenu};

pub const LABEL: &str = "radial";

/// Dernier menu demandé, conservé pour le webview s'il n'était pas encore prêt
/// au moment de l'ouverture (ex : lancement avec des fichiers en arguments).
#[derive(Default)]
pub struct Pending(pub Mutex<Option<RadialPayload>>);

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Serialize)]
pub struct RadialPayload {
    /// Identifiant croissant : le webview ignore une demande déjà reçue
    /// (elle peut arriver à la fois par l'événement et par `take_pending`).
    id: u64,
    paths: Vec<String>,
    menu: TargetMenu,
}

/// Ouvre le menu des formats pour `paths`, centré sur la position actuelle du curseur.
pub fn show(app: &AppHandle, paths: Vec<String>) -> tauri::Result<()> {
    let window = app.get_webview_window(LABEL).ok_or(tauri::Error::WindowNotFound)?;
    let cursor = app.cursor_position()?;
    let size = window.outer_size()?;
    let (w, h) = (size.width as i32, size.height as i32);
    let mut x = cursor.x as i32 - w / 2;
    let mut y = cursor.y as i32 - h / 2;

    // Garde le menu entièrement visible sur l'écran du curseur.
    if let Some(monitor) = app.monitor_from_point(cursor.x, cursor.y)? {
        let area = monitor.work_area();
        let (left, top) = (area.position.x, area.position.y);
        let right = left + area.size.width as i32 - w;
        let bottom = top + area.size.height as i32 - h;
        x = x.min(right).max(left);
        y = y.min(bottom).max(top);
    }
    window.set_position(PhysicalPosition::new(x, y))?;

    let refs: Vec<&Path> = paths.iter().map(Path::new).collect();
    let menu = formats::menu_for(&refs);
    let payload = RadialPayload { id: NEXT_ID.fetch_add(1, Ordering::Relaxed), paths, menu };
    *app.state::<Pending>().0.lock().unwrap() = Some(payload.clone());
    app.emit_to(LABEL, "radial-open", payload)?;

    window.show()?;
    window.set_focus()?;
    Ok(())
}

/// Récupère (et consomme) le menu en attente.
pub fn take_pending(app: &AppHandle) -> Option<RadialPayload> {
    app.state::<Pending>().0.lock().unwrap().take()
}

pub fn hide(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(LABEL) {
        window.hide()?;
    }
    Ok(())
}
