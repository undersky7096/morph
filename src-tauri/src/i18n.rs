//! Langue des textes montrés par le cœur (notifications, menu de l'icône, titres de
//! fenêtres, messages d'erreur) : celle choisie dans les Réglages, ou celle du système.
//! Les textes vivent dans les catalogues `locales/<code>.json`, partagés avec l'interface
//! (clés `core.*` pour le cœur) : `tr("core.settings")` → « Réglages ».
//! Les catalogues sont intégrés au binaire (liste générée par `build.rs`) : ajouter une
//! langue = ajouter son fichier. Une clé absente retombe sur l'anglais, puis le français.
//! Les messages internes (détails techniques, journaux) restent en français.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::{OnceLock, RwLock};

use serde_json::Value;

// CATALOGS: &[(&str, &str)] = &[("en", include_str!(…)), ("fr", …)]
include!(concat!(env!("OUT_DIR"), "/locales.rs"));

const FALLBACK: &str = "en";
const SOURCE: &str = "fr";

/// Entrée d'un catalogue : texte simple, ou formes du pluriel (`one`, `other`).
enum Entry {
    Text(String),
    Plural { one: Option<String>, other: String },
}

/// Catalogues lus une fois pour toutes : leurs textes vivent jusqu'à la fin du programme.
fn catalogs() -> &'static HashMap<&'static str, HashMap<String, Entry>> {
    static ALL: OnceLock<HashMap<&'static str, HashMap<String, Entry>>> = OnceLock::new();
    ALL.get_or_init(|| {
        CATALOGS
            .iter()
            .map(|(code, json)| {
                let map: HashMap<String, Value> = serde_json::from_str(json).expect("catalogue de langue invalide");
                let entries = map
                    .into_iter()
                    .filter_map(|(k, v)| match v {
                        Value::String(s) => Some((k, Entry::Text(s))),
                        Value::Object(o) => {
                            let get = |f: &str| o.get(f).and_then(Value::as_str).map(str::to_string);
                            Some((k, Entry::Plural { one: get("one"), other: get("other")? }))
                        }
                        _ => None,
                    })
                    .collect();
                (*code, entries)
            })
            .collect()
    })
}

/// Code de la langue effective (« fr », « en »…).
static CURRENT: RwLock<&'static str> = RwLock::new(SOURCE);

/// Applique la langue des réglages (« system » ou un code), appelé à chaque lecture ou
/// changement des réglages.
pub fn apply(language: &str) {
    let code = available(language).unwrap_or_else(system_language);
    *CURRENT.write().unwrap() = code;
}

/// Le code, s'il a un catalogue.
fn available(code: &str) -> Option<&'static str> {
    catalogs().keys().find(|c| **c == code).copied()
}

/// Langue du système si un catalogue existe pour elle, sinon l'anglais.
pub fn system_language() -> &'static str {
    let system = sys_locale::get_locale().unwrap_or_default().to_lowercase();
    available(system.split(['-', '_']).next().unwrap_or("")).unwrap_or(FALLBACK)
}

fn current() -> &'static str {
    *CURRENT.read().unwrap()
}

fn entry(key: &str) -> Option<&'static Entry> {
    let all = catalogs();
    [current(), FALLBACK, SOURCE].iter().find_map(|code| all.get(code)?.get(key))
}

/// Texte dans la langue de l'interface (la clé elle-même si elle n'existe nulle part).
pub fn tr(key: &'static str) -> &'static str {
    match entry(key) {
        Some(Entry::Text(s)) => s,
        Some(Entry::Plural { other, .. }) => other,
        None => key,
    }
}

/// Le texte d'une clé dans chaque langue disponible.
pub fn tr_all(key: &'static str) -> Vec<&'static str> {
    let mut all: Vec<&'static str> = catalogs()
        .values()
        .filter_map(|c| match c.get(key)? {
            Entry::Text(s) => Some(s.as_str()),
            Entry::Plural { other, .. } => Some(other.as_str()),
        })
        .collect();
    all.sort_unstable();
    all.dedup();
    all
}

/// Remplace `{nom}` par sa valeur.
fn fill(text: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = text.to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), &value.to_string());
    }
    out
}

/// Texte avec des valeurs : `trf("core.file_converted", &[("target", &"WEBP")])`.
pub fn trf(key: &'static str, args: &[(&str, &dyn Display)]) -> String {
    fill(tr(key), args)
}

/// Pluriel : `trn("core.files_converted", n, &[…])` ; `{n}` = le nombre. En français 0 et 1
/// sont au singulier, dans les autres langues seulement 1.
pub fn trn(key: &'static str, n: usize, args: &[(&str, &dyn Display)]) -> String {
    let text = match entry(key) {
        Some(Entry::Plural { one, other }) => {
            let singular = if current() == "fr" { n <= 1 } else { n == 1 };
            match one {
                Some(one) if singular => one.as_str(),
                _ => other.as_str(),
            }
        }
        Some(Entry::Text(s)) => s.as_str(),
        None => key,
    };
    let mut all: Vec<(&str, &dyn Display)> = vec![("n", &n)];
    all.extend_from_slice(args);
    fill(text, &all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogues_ont_les_memes_cles() {
        let all = catalogs();
        let fr = &all["fr"];
        for (code, entries) in all {
            for key in fr.keys() {
                assert!(entries.contains_key(key), "{code} : clé manquante {key}");
            }
            for key in entries.keys() {
                assert!(fr.contains_key(key), "{code} : clé inconnue {key}");
            }
        }
    }

    #[test]
    fn langue_et_pluriel() {
        apply("fr");
        assert_eq!(tr("core.settings"), "Réglages");
        assert_eq!(trn("core.files_converted", 1, &[("target", &"WEBP")]), "1 fichier converti en WEBP");
        assert_eq!(trn("core.files_converted", 3, &[("target", &"WEBP")]), "3 fichiers convertis en WEBP");
        apply("en");
        assert_eq!(tr("core.settings"), "Settings");
        assert_eq!(trn("core.files_converted", 1, &[("target", &"WEBP")]), "1 file converted to WEBP");
        apply("xx");
        assert!(["fr", "en"].contains(&current()));
        assert_eq!(tr("core.cle_inexistante"), "core.cle_inexistante");
    }
}
