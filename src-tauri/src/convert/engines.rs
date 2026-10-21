//! Disponibilité des moteurs de conversion, vérifiée une fois (au démarrage, puis gardée) :
//! une conversion dont un moteur manque n'est pas lancée, et le menu l'affiche grisée avec
//! la raison au lieu d'échouer après coup.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::catalog::Engine;
use super::ffmpeg;

/// `None` : disponible ; sinon, le message à montrer.
fn check(engine: Engine) -> Option<&'static str> {
    match engine {
        Engine::Image | Engine::Svg => None,
        Engine::Ffmpeg => {
            let ok = ffmpeg::command().arg("-version").output().is_ok_and(|o| o.status.success());
            (!ok).then_some("Le moteur de conversion (ffmpeg) est introuvable : réinstalle Morph.")
        }
    }
}

fn table() -> &'static HashMap<Engine, Option<&'static str>> {
    static TABLE: OnceLock<HashMap<Engine, Option<&'static str>>> = OnceLock::new();
    TABLE.get_or_init(|| [Engine::Image, Engine::Svg, Engine::Ffmpeg].into_iter().map(|e| (e, check(e))).collect())
}

/// Vérifie tous les moteurs en arrière-plan, pour que le premier menu n'attende pas.
pub fn warm_up() {
    std::thread::spawn(|| {
        table();
    });
}

/// Raison pour laquelle `engine` est indisponible, ou `None` s'il fonctionne.
pub fn missing(engine: Engine) -> Option<&'static str> {
    table().get(&engine).copied().flatten()
}
