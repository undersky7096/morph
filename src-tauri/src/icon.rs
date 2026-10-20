//! Icône de Morph en fonctionnement (zone de notification, fenêtres), aux couleurs de
//! l'apparence : M noir sur carré blanc en clair, M blanc sur carré noir en sombre, et
//! carré de la couleur de fond personnalisée (M blanc ou noir selon sa luminosité).
//! L'icône fixe (exe, installeur, raccourcis) reste celle des fichiers `icons/`.

use std::sync::{Mutex, OnceLock};

use image::{GrayImage, imageops};
use tauri::image::Image;
use tauri::{AppHandle, Manager, Theme, WebviewWindow};

use crate::settings::{self, Settings, ThemeMode};

/// Le M seul, blanc sur transparent (généré par `scripts/icons.py`).
const MARK_PNG: &[u8] = include_bytes!("../icons/mark.png");

// Géométrie partagée avec `scripts/icons.py`.
const TILE: f32 = 0.90;
const RADIUS: f32 = 0.23;
const MARK: f32 = 0.60;

const TRAY_SIZE: u32 = 64;
const WINDOW_SIZE: u32 = 128;

const WHITE: [u8; 3] = [255, 255, 255];
const BLACK: [u8; 3] = [17, 17, 17];
/// Couleur du M du logo.
const INK: [u8; 3] = [24, 24, 24];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colors {
    tile: [u8; 3],
    ink: [u8; 3],
}

fn hex(c: &str) -> Option<[u8; 3]> {
    let byte = |i: usize| u8::from_str_radix(c.get(i..i + 2)?, 16).ok();
    Some([byte(1)?, byte(3)?, byte(5)?])
}

/// Luminance relative (0 = noir, 1 = blanc), comme côté interface (`settings.ts`).
fn luminance([r, g, b]: [u8; 3]) -> f32 {
    let lin = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

/// Couleurs de l'icône pour des réglages ; `os_dark` : thème du système (réglage « Système »).
pub fn colors(s: &Settings, os_dark: bool) -> Colors {
    if let Some(bg) = s.background.as_deref().and_then(hex) {
        let ink = if luminance(bg) < 0.3 { WHITE } else { INK };
        return Colors { tile: bg, ink };
    }
    let dark = match s.theme {
        ThemeMode::System => os_dark,
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
    };
    if dark { Colors { tile: BLACK, ink: WHITE } } else { Colors { tile: WHITE, ink: INK } }
}

fn mark() -> &'static GrayImage {
    static MARK_ALPHA: OnceLock<GrayImage> = OnceLock::new();
    MARK_ALPHA.get_or_init(|| {
        let rgba = image::load_from_memory(MARK_PNG).expect("icons/mark.png").to_rgba8();
        GrayImage::from_fn(rgba.width(), rgba.height(), |x, y| image::Luma([rgba.get_pixel(x, y)[3]]))
    })
}

/// Icône carrée de `size` px, en RGBA.
pub fn render(size: u32, c: Colors) -> Vec<u8> {
    let s = size as f32;
    let half = s * TILE / 2.0;
    let r = s * TILE * RADIUS;
    let m = (s * TILE * MARK).round() as u32;
    let glyph = imageops::resize(mark(), m, m, imageops::FilterType::Lanczos3);
    let offset = (size - m) / 2;

    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            // Distance signée au carré arrondi, au centre du pixel : bord anticrénelé.
            let (px, py) = ((x as f32 + 0.5 - s / 2.0).abs(), (y as f32 + 0.5 - s / 2.0).abs());
            let (qx, qy) = (px - (half - r), py - (half - r));
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            let d = outside + qx.max(qy).min(0.0) - r;
            let tile_alpha = (0.5 - d).clamp(0.0, 1.0);

            let ink = match (x.checked_sub(offset), y.checked_sub(offset)) {
                (Some(gx), Some(gy)) if gx < m && gy < m => glyph.get_pixel(gx, gy)[0] as f32 / 255.0,
                _ => 0.0,
            };
            for i in 0..3 {
                out.push((c.ink[i] as f32 * ink + c.tile[i] as f32 * (1.0 - ink)).round() as u8);
            }
            out.push((tile_alpha * 255.0).round() as u8);
        }
    }
    out
}

fn os_dark(app: &AppHandle) -> bool {
    app.get_webview_window(crate::radial::LABEL)
        .and_then(|w| w.theme().ok())
        .is_some_and(|t| t == Theme::Dark)
}

fn current(app: &AppHandle) -> Colors {
    colors(&settings::get(app), os_dark(app))
}

/// Dernières couleurs appliquées : rien à refaire si l'apparence n'a pas changé.
static APPLIED: Mutex<Option<Colors>> = Mutex::new(None);

/// Met l'icône à jour partout (après un changement de réglages ou de thème du système).
pub fn refresh(app: &AppHandle) {
    let c = current(app);
    if APPLIED.lock().unwrap().replace(c) == Some(c) {
        return;
    }
    if let Some(tray) = app.tray_by_id(crate::tray::ID) {
        let _ = tray.set_icon(Some(Image::new_owned(render(TRAY_SIZE, c), TRAY_SIZE, TRAY_SIZE)));
    }
    let icon = render(WINDOW_SIZE, c);
    for window in app.webview_windows().values() {
        let _ = window.set_icon(Image::new_owned(icon.clone(), WINDOW_SIZE, WINDOW_SIZE));
    }
}

/// Icône d'une fenêtre qui vient d'être créée.
pub fn apply_to(window: &WebviewWindow) {
    let c = current(window.app_handle());
    let _ = window.set_icon(Image::new_owned(render(WINDOW_SIZE, c), WINDOW_SIZE, WINDOW_SIZE));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_theme_and_custom_background() {
        let light = Settings { theme: ThemeMode::Light, ..Settings::default() };
        assert_eq!(colors(&light, true), Colors { tile: WHITE, ink: INK });
        let system = Settings::default();
        assert_eq!(colors(&system, true), Colors { tile: BLACK, ink: WHITE });
        let custom_dark = Settings { background: Some("#16112a".into()), ..Settings::default() };
        assert_eq!(colors(&custom_dark, false), Colors { tile: [0x16, 0x11, 0x2a], ink: WHITE });
        let custom_light = Settings { background: Some("#f7f3ea".into()), ..Settings::default() };
        assert_eq!(colors(&custom_light, true).ink, INK);
    }

    #[test]
    fn renders_tile_with_transparent_corners() {
        let c = Colors { tile: WHITE, ink: INK };
        let px = render(64, c);
        assert_eq!(px.len(), 64 * 64 * 4);
        assert_eq!(px[3], 0, "coin transparent");
        let center = ((32 * 64 + 32) * 4) as usize;
        assert_eq!(px[center + 3], 255, "carré opaque au centre");
    }
}
