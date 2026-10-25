//! Mini-fenêtre de progression des conversions longues, en bas à droite de l'écran.

use tauri::{AppHandle, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

pub const LABEL: &str = "progress";
const WIDTH: f64 = 400.0;
const HEIGHT: f64 = 360.0;
const MARGIN: i32 = 16;

pub fn show(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(LABEL) {
        window.unminimize()?;
        return window.show();
    }
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title(crate::i18n::tr("core.progress_morph"))
        .inner_size(WIDTH, HEIGHT)
        .min_inner_size(320.0, 220.0)
        .decorations(false)
        .always_on_top(true)
        .focused(false)
        .visible(false)
        .build()?;
    crate::icon::apply_to(&window);
    // En bas à droite de l'écran principal, au-dessus de la barre des tâches.
    if let Some(monitor) = window.primary_monitor()? {
        let area = monitor.work_area();
        let size = window.outer_size()?;
        let x = area.position.x + area.size.width as i32 - size.width as i32 - MARGIN;
        let y = area.position.y + area.size.height as i32 - size.height as i32 - MARGIN;
        window.set_position(PhysicalPosition::new(x, y))?;
    }
    window.show()
}
