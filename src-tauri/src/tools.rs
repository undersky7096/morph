//! Fenêtre des outils d'édition (Crop, Compress, Edit), créée à la demande.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "tool";

#[derive(Clone, Serialize)]
pub struct ToolPayload {
    tool: String,
    paths: Vec<String>,
}

/// Outil demandé, récupéré par le webview au montage s'il vient d'être créé.
#[derive(Default)]
pub struct Pending(pub Mutex<Option<ToolPayload>>);

pub fn open(app: &AppHandle, tool: String, paths: Vec<String>) -> tauri::Result<()> {
    let payload = ToolPayload { tool, paths };
    *app.state::<Pending>().0.lock().unwrap() = Some(payload.clone());

    if let Some(window) = app.get_webview_window(LABEL) {
        app.emit_to(LABEL, "tool-open", payload)?;
        window.unminimize()?;
        window.show()?;
        window.set_focus()?;
    } else {
        WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
            .title("Morph")
            .inner_size(980.0, 700.0)
            .min_inner_size(640.0, 480.0)
            .decorations(false)
            .center()
            .focused(true)
            .build()?;
    }
    Ok(())
}

pub fn take_pending(app: &AppHandle) -> Option<ToolPayload> {
    app.state::<Pending>().0.lock().unwrap().take()
}
