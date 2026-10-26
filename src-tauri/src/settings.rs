//! Réglages de Morph : fichier JSON dans le dossier de configuration de l'application,
//! diffusés à toutes les fenêtres (événement `settings-changed`) à chaque modification.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::convert::output::Duplicates;
use crate::edit::cutout;

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

/// Langue de l'interface : « system » (celle de l'ordinateur) ou le code d'une langue qui a
/// un catalogue dans `locales/` (« fr », « en »…).
pub type Language = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub language: Language,
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
    /// Fichier de sortie déjà présent : garder les deux, remplacer, ou demander.
    pub duplicates: Duplicates,
    /// Modèle du détourage : efficacité (IS-Net) ou qualité (BiRefNet).
    pub cutout: cutout::Quality,
    /// Détourage calculé par la carte graphique (Windows : DirectML), quand c'est possible.
    pub gpu: bool,
    /// Lancement à l'ouverture de session (reflète l'état réel du système).
    pub autostart: bool,
    /// Écran d'accueil déjà vu (ou passé).
    pub onboarded: bool,
    /// Préréglages de l'outil Compresser (compte Morph : synchronisés entre ordinateurs).
    pub compress_presets: Vec<CompressPreset>,
}

/// Réglages de Compresser enregistrés sous un nom (« Discord », « Envoi par mail »…).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressPreset {
    pub name: String,
    pub level: crate::edit::compress::Level,
    #[serde(default)]
    pub reduce_colors: bool,
    /// Taille visée (audio, vidéo), en Mo ; aucune : par niveau.
    #[serde(default)]
    pub target_mb: Option<f64>,
}

/// Au plus autant de préréglages (la synchronisation est limitée à 8 Ko).
pub const MAX_PRESETS: usize = 12;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: "system".into(),
            theme: ThemeMode::System,
            accent: None,
            background: None,
            radial_opacity: 82,
            favorites: Vec::new(),
            mixed_batch: MixedBatch::Groups,
            save_mode: SaveMode::Beside,
            save_folder: None,
            duplicates: Duplicates::Keep,
            cutout: cutout::Quality::Fast,
            gpu: true,
            autostart: false,
            onboarded: false,
            compress_presets: Vec::new(),
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
        self.compress_presets = std::mem::take(&mut self.compress_presets)
            .into_iter()
            .filter_map(|mut p| {
                p.name = p.name.trim().chars().take(40).collect();
                p.target_mb = p.target_mb.filter(|t| t.is_finite() && *t > 0.0 && *t < 100_000.0);
                (!p.name.is_empty()).then_some(p)
            })
            .take(MAX_PRESETS)
            .collect();
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

/// Réglages en mémoire ; lus depuis le fichier à la première demande (une fenêtre peut
/// les demander avant la fin du démarrage).
#[derive(Default)]
pub struct State(RwLock<Option<Settings>>);

fn file(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("settings.json"))
}

/// Lecture du fichier, sans jamais perdre les réglages de l'utilisateur : une lecture
/// ratée (fichier momentanément verrouillé) est réessayée, et un fichier illisible est
/// gardé de côté (`settings.json.bak`) avant que de nouveaux réglages ne le remplacent.
fn read(path: &Path) -> Settings {
    if !path.exists() {
        return Settings::default();
    }
    let mut text = None;
    for attempt in 0..3 {
        match std::fs::read_to_string(path) {
            Ok(t) => {
                text = Some(t);
                break;
            }
            Err(e) => {
                eprintln!("[morph] lecture des réglages ({attempt}) : {e}");
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
        }
    }
    match text.as_deref().map(serde_json::from_str::<Settings>) {
        Some(Ok(s)) => s.sanitized(),
        other => {
            eprintln!("[morph] réglages illisibles ({other:?}) : copie dans settings.json.bak");
            let _ = std::fs::copy(path, path.with_extension("json.bak"));
            Settings::default()
        }
    }
}

/// Lecture du fichier ; `autostart` reflète l'état réel du système.
/// Une entrée de démarrage disparue alors que le réglage est actif (désinstallation suivie
/// d'une réinstallation qui a gardé les réglages) est recréée.
fn read_from_disk(app: &AppHandle) -> Settings {
    let mut s = file(app).map(|p| read(&p)).unwrap_or_default();
    let launcher = app.autolaunch();
    let enabled = launcher.is_enabled().unwrap_or(false);
    s.autostart = enabled || (s.autostart && launcher.enable().is_ok());
    s
}

/// Au démarrage. Premier lancement (aucun fichier) : Morph se lance à l'ouverture de
/// session par défaut, puisque le Shift + glisser n'existe que s'il tourne.
pub fn load(app: &AppHandle) {
    let first_run = file(app).is_some_and(|p| !p.exists());
    let installer_language = take_installer_language(app);
    let s = get(app);
    if first_run {
        // Langue choisie dans l'installeur Windows ; sinon, celle du système.
        let language = installer_language.unwrap_or(s.language.clone());
        if let Err(e) = set(app, Settings { autostart: true, language, ..s }) {
            eprintln!("[morph] réglages par défaut : {e}");
        }
    }
}

/// Langue choisie dans l'installeur (fichier `installer-language` écrit par ses hooks),
/// lue puis supprimée : elle ne sert qu'au premier lancement.
fn take_installer_language(app: &AppHandle) -> Option<Language> {
    let path = app.path().app_config_dir().ok()?.join("installer-language");
    let text = std::fs::read_to_string(&path).ok()?;
    let _ = std::fs::remove_file(&path);
    let code = text.trim();
    (!code.is_empty()).then(|| code.to_string())
}

pub fn get(app: &AppHandle) -> Settings {
    let state = app.state::<State>();
    if let Some(s) = state.0.read().unwrap().as_ref() {
        return s.clone();
    }
    let mut slot = state.0.write().unwrap();
    slot.get_or_insert_with(|| {
        let s = read_from_disk(app);
        crate::i18n::apply(&s.language);
        cutout::set_gpu(s.gpu);
        s
    })
    .clone()
}

/// Enregistre, applique (lancement au démarrage) et diffuse à toutes les fenêtres.
pub fn set(app: &AppHandle, new: Settings) -> Result<Settings, String> {
    let new = new.sanitized();
    let old = get(app);
    if new.autostart != old.autostart {
        let launcher = app.autolaunch();
        let result = if new.autostart { launcher.enable() } else { launcher.disable() };
        result.map_err(|e| format!("{}{e}", crate::i18n::tr("core.start_at_sign")))?;
    }
    if let Some(path) = file(app) {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(&new).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
    }
    crate::i18n::apply(&new.language);
    cutout::set_gpu(new.gpu);
    *app.state::<State>().0.write().unwrap() = Some(new.clone());
    let _ = app.emit("settings-changed", &new);
    if new.language != old.language {
        crate::tray::relabel(app);
    }
    crate::icon::refresh(app);
    crate::account::settings_changed(app, &old, &new);
    Ok(new)
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
