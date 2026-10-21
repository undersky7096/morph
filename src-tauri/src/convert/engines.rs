//! Disponibilité des moteurs de conversion, vérifiée au démarrage puis gardée : une
//! conversion dont un moteur manque n'est pas lancée, et le menu l'affiche grisée avec la
//! raison au lieu d'échouer après coup. LibreOffice, installé à part, est recherché de
//! nouveau tant qu'il est absent (il peut être installé pendant que Morph tourne).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use super::catalog::Engine;
use super::{archive, ffmpeg, office, pandoc, pdf};
use crate::i18n::tr;

/// Moteur indisponible : le message à montrer (en français et en anglais), et une page
/// pour l'obtenir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Missing {
    fr: &'static str,
    en: &'static str,
    pub link: Option<&'static str>,
}

impl Missing {
    /// Message dans la langue de l'interface.
    pub fn reason(&self) -> &'static str {
        tr(self.fr, self.en)
    }
}

const REINSTALL: Option<&str> = None;
pub const LIBREOFFICE_URL: &str = "https://www.libreoffice.org/download/download-libreoffice/";
pub const LIBREOFFICE_MISSING: Missing = Missing {
    fr: "Nécessite LibreOffice (gratuit) : clique pour l'installer, puis réessaie.",
    en: "Requires LibreOffice (free): click to install it, then try again.",
    link: Some(LIBREOFFICE_URL),
};

pub const RAR_MISSING: Missing = Missing {
    fr: "Pour ouvrir les RAR, installe bsdtar (paquet libarchive-tools), unrar ou 7-Zip, puis réessaie.",
    en: "To open RAR files, install bsdtar (libarchive-tools package), unrar or 7-Zip, then try again.",
    link: None,
};

fn check(engine: Engine) -> Option<Missing> {
    let missing = |fr, en, link| Some(Missing { fr, en, link });
    match engine {
        Engine::Image | Engine::Svg | Engine::Archive => None,
        Engine::Rar => archive::rar_tool().is_none().then_some(RAR_MISSING),
        Engine::Ffmpeg => {
            let ok = ffmpeg::command().arg("-version").output().is_ok_and(|o| o.status.success());
            if ok {
                None
            } else {
                missing(
                    "Le moteur de conversion (ffmpeg) est introuvable : réinstalle Morph.",
                    "The conversion engine (ffmpeg) is missing: reinstall Morph.",
                    REINSTALL,
                )
            }
        }
        Engine::Pandoc => {
            let ok = pandoc::command().arg("--version").output().is_ok_and(|o| o.status.success());
            if ok {
                None
            } else {
                missing(
                    "Le moteur de documents (pandoc) est introuvable : réinstalle Morph.",
                    "The document engine (pandoc) is missing: reinstall Morph.",
                    REINSTALL,
                )
            }
        }
        Engine::Pdf => {
            if pdf::library_present() {
                None
            } else {
                missing("Le moteur PDF (pdfium) est introuvable : réinstalle Morph.", "The PDF engine (pdfium) is missing: reinstall Morph.", REINSTALL)
            }
        }
        Engine::Office => office::soffice().is_none().then_some(LIBREOFFICE_MISSING),
    }
}

const CACHED: [Engine; 3] = [Engine::Ffmpeg, Engine::Pandoc, Engine::Pdf];

fn table() -> &'static HashMap<Engine, Option<Missing>> {
    static TABLE: OnceLock<HashMap<Engine, Option<Missing>>> = OnceLock::new();
    TABLE.get_or_init(|| CACHED.into_iter().map(|e| (e, check(e))).collect())
}

/// Vérifie les moteurs en arrière-plan, pour que le premier menu n'attende pas.
pub fn warm_up() {
    std::thread::spawn(|| {
        table();
        office_status();
    });
}

/// LibreOffice : gardé une fois trouvé, recherché de nouveau tant qu'il manque.
fn office_status() -> Option<Missing> {
    static FOUND: Mutex<Option<PathBuf>> = Mutex::new(None);
    let mut found = FOUND.lock().unwrap();
    if found.is_some() {
        return None;
    }
    *found = office::soffice();
    found.is_none().then_some(LIBREOFFICE_MISSING)
}

/// Raison pour laquelle `engine` est indisponible, ou `None` s'il fonctionne.
pub fn missing(engine: Engine) -> Option<Missing> {
    match engine {
        Engine::Image | Engine::Svg | Engine::Archive => None,
        // Programme du système : recherché à chaque fois (rapide), il peut être installé
        // pendant que Morph tourne.
        Engine::Rar => check(Engine::Rar),
        Engine::Office => office_status(),
        _ => table().get(&engine).copied().flatten(),
    }
}
