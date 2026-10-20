//! Réglages de Morph : fichier JSON dans le dossier de configuration de l'application,
//! diffusés à toutes les fenêtres (événement `settings-changed`) à chaque modification.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

/// Lots de types mélangés : un sous-menu par type, ou seulement les formats communs à tous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MixedBatch {
    #[default]
    Groups,
    Common,
}

/// Où enregistrer les résultats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SaveMode {
    /// À côté de l'original.
    #[default]
    Beside,
    /// Dans `save_folder`.
    Folder,
    /// Dossier demandé à chaque opération (par l'interface, qui le transmet).
    Ask,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub theme: ThemeMode,
    /// Couleur d'accent (`#rrggbb`) ; aucune : celle du thème.
    pub accent: Option<String>,
    /// Teinte du fond (`#rrggbb`) ; aucune : celle du thème.
    pub background: Option<String>,
    /// Opacité des menus radiaux, en % (40…100).
    pub radial_opacity: u8,
    /// Formats affichés en premier dans le menu, dans cet ordre.
    pub favorites: Vec<String>,
    pub mixed_batch: MixedBatch,
    pub save_mode: SaveMode,
    pub save_folder: Option<String>,
    /// Lancement à l'ouverture de session (reflète l'état réel du système).
    pub autostart: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: ThemeMode::System,
            accent: None,
            background: None,
            radial_opacity: 82,
            favorites: Vec::new(),
            mixed_batch: MixedBatch::Groups,
            save_mode: SaveMode::Beside,
            save_folder: None,
            autostart: false,
        }
    }
}

impl Settings {
    /// Valeurs ramenées dans leurs bornes (fichier modifié à la main, ancienne version…).
    fn sanitized(mut self) -> Self {
        let hex = |c: Option<String>| {
            c.filter(|c| c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|d| d.is_ascii_hexdigit()))
        };
        self.accent = hex(self.accent);
        self.background = hex(self.background);
        self.radial_opacity = self.radial_opacity.clamp(40, 100);
        self.favorites.dedup();
        if self.save_mode == SaveMode::Folder && self.save_folder.as_deref().is_none_or(str::is_empty) {
            self.save_mode = SaveMode::Beside;
        }
        self
    }

    /// Dossier où écrire un résultat : `chosen` (mode « demander »), le dossier fixe,
    /// ou `None` (à côté de l'original).
    pub fn output_dir(&self, chosen: Option<&str>) -> Option<PathBuf> {
        match (self.save_mode, chosen) {
            (_, Some(dir)) if !dir.is_empty() => Some(PathBuf::from(dir)),
            (SaveMode::Folder, _) => self.save_folder.as_ref().map(PathBuf::from),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct State(pub RwLock<Settings>);

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("settings.json"))
}

fn read(path: &Path) -> Settings {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str::<Settings>(&s).ok())
        .unwrap_or_default()
        .sanitized()
}

/// Charge les réglages au démarrage ; `autostart` reflète l'état réel du système.
pub fn load(app: &AppHandle) {
    let mut s = file(app).map(|p| read(&p)).unwrap_or_default();
    s.autostart = app.autolaunch().is_enabled().unwrap_or(false);
    *app.state::<State>().0.write().unwrap() = s;
}

pub fn get(app: &AppHandle) -> Settings {
    app.state::<State>().0.read().unwrap().clone()
}

/// Enregistre, applique (lancement au démarrage) et diffuse à toutes les fenêtres.
pub fn set(app: &AppHandle, new: Settings) -> Result<Settings, String> {
    let new = new.sanitized();
    let old = get(app);
    if new.autostart != old.autostart {
        let launcher = app.autolaunch();
        let result = if new.autostart { launcher.enable() } else { launcher.disable() };
        result.map_err(|e| format!("lancement au démarrage : {e}"))?;
    }
    if let Some(path) = file(app) {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(&new).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
    }
    *app.state::<State>().0.write().unwrap() = new.clone();
    let _ = app.emit("settings-changed", &new);
    Ok(new)
}

/// Dossier de sortie d'une opération, d'après les réglages et le choix éventuel de l'utilisateur.
pub fn output_dir(app: &AppHandle, chosen: Option<&str>) -> Option<PathBuf> {
    get(app).output_dir(chosen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_partial_and_invalid_files() {
        let s: Settings = serde_json::from_str(r##"{"theme":"dark","accent":"red","radialOpacity":5,"saveMode":"folder"}"##).unwrap();
        let s = s.sanitized();
        assert_eq!(s.theme, ThemeMode::Dark);
        assert_eq!(s.accent, None);
        assert_eq!(s.radial_opacity, 40);
        // Dossier fixe sans dossier : retour à « à côté de l'original ».
        assert_eq!(s.save_mode, SaveMode::Beside);
    }

    #[test]
    fn resolves_output_dir() {
        let mut s = Settings { save_mode: SaveMode::Folder, save_folder: Some("D:/Sorties".into()), ..Settings::default() };
        assert_eq!(s.output_dir(None), Some(PathBuf::from("D:/Sorties")));
        s.save_mode = SaveMode::Ask;
        assert_eq!(s.output_dir(Some("E:/Choisi")), Some(PathBuf::from("E:/Choisi")));
        s.save_mode = SaveMode::Beside;
        assert_eq!(s.output_dir(None), None);
    }
}
