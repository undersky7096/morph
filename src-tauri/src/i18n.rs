//! Langue des textes montrés par le cœur (notifications, menu de l'icône, titres de
//! fenêtres, messages d'erreur) : celle choisie dans les Réglages, ou celle du système.
//! Les textes restent écrits dans le code, en paires : `tr("Réglages", "Settings")`.
//! Les messages internes (détails techniques, journaux) restent en français.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::settings::Language;

static ENGLISH: AtomicBool = AtomicBool::new(false);

/// Applique la langue des réglages (appelé à chaque lecture ou changement des réglages).
pub fn apply(language: Language) {
    let english = match language {
        Language::Fr => false,
        Language::En => true,
        Language::System => !system_is_french(),
    };
    ENGLISH.store(english, Ordering::Relaxed);
}

/// Langue du système : français si sa locale commence par « fr ».
pub fn system_is_french() -> bool {
    sys_locale::get_locale().is_some_and(|l| l.to_lowercase().starts_with("fr"))
}

pub fn english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// Texte dans la langue de l'interface.
pub fn tr<T>(fr: T, en: T) -> T {
    if english() { en } else { fr }
}
