//! Disponibilité des moteurs de conversion, vérifiée au démarrage puis gardée : une
//! conversion dont un moteur manque n'est pas lancée, et le menu l'affiche grisée avec la
//! raison au lieu d'échouer après coup. LibreOffice, installé à part, est recherché de
//! nouveau tant qu'il est absent (il peut être installé pendant que Morph tourne).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use super::catalog::Engine;
use super::{ffmpeg, office, pandoc, pdf};

/// Moteur indisponible : le message à montrer, et une page pour l'obtenir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Missing {
    pub reason: &'static str,
    pub link: Option<&'static str>,
}

const REINSTALL: Option<&str> = None;
pub const LIBREOFFICE_URL: &str = "https://www.libreoffice.org/download/download-libreoffice/";
pub const LIBREOFFICE_MISSING: Missing = Missing {
    reason: "Nécessite LibreOffice (gratuit) : clique pour l'installer, puis réessaie.",
    link: Some(LIBREOFFICE_URL),
};

fn check(engine: Engine) -> Option<Missing> {
    let missing = |reason, link| Some(Missing { reason, link });
    match engine {
        Engine::Image | Engine::Svg => None,
        Engine::Ffmpeg => {
            let ok = ffmpeg::command().arg("-version").output().is_ok_and(|o| o.status.success());
            if ok { None } else { missing("Le moteur de conversion (ffmpeg) est introuvable : réinstalle Morph.", REINSTALL) }
        }
        Engine::Pandoc => {
            let ok = pandoc::command().arg("--version").output().is_ok_and(|o| o.status.success());
            if ok { None } else { missing("Le moteur de documents (pandoc) est introuvable : réinstalle Morph.", REINSTALL) }
        }
        Engine::Pdf => {
            if pdf::library_present() { None } else { missing("Le moteur PDF (pdfium) est introuvable : réinstalle Morph.", REINSTALL) }
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
        Engine::Image | Engine::Svg => None,
        Engine::Office => office_status(),
        _ => table().get(&engine).copied().flatten(),
    }
}
