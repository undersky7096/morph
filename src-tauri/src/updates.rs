//! Mises à jour (releases GitHub, signées) : vérification au démarrage puis toutes les 6 h,
//! notification une seule fois par version, installation seulement à la demande depuis les
//! Réglages, jamais pendant un traitement. L'état est diffusé par l'événement `update-status`.

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::{jobs, notify, tools};

/// Première vérification, après le démarrage (laisse la session s'ouvrir tranquillement).
const FIRST_CHECK: Duration = Duration::from_secs(30);
const EVERY: Duration = Duration::from_secs(6 * 3600);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Available {
    pub version: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub available: Option<Available>,
    pub checking: bool,
    /// Téléchargement de la mise à jour : avancement 0…1.
    pub installing: Option<f32>,
    pub error: Option<String>,
    /// Dernière vérification, en millisecondes depuis l'époque Unix.
    pub last_check_ms: Option<u64>,
}

#[derive(Default)]
pub struct State {
    status: Mutex<Status>,
    update: Mutex<Option<Update>>,
    /// Version déjà annoncée par une notification.
    notified: Mutex<Option<String>>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn change(app: &AppHandle, f: impl FnOnce(&mut Status)) -> Status {
    let state = app.state::<State>();
    let mut status = state.status.lock().unwrap();
    f(&mut status);
    let snapshot = status.clone();
    drop(status);
    let _ = app.emit("update-status", &snapshot);
    snapshot
}

pub fn status(app: &AppHandle) -> Status {
    app.state::<State>().status.lock().unwrap().clone()
}

/// Vérifications automatiques, sur un fil dédié pour toute la durée de l'application.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(FIRST_CHECK);
        loop {
            tauri::async_runtime::block_on(check(&app));
            std::thread::sleep(EVERY);
        }
    });
}

/// Cherche une nouvelle version ; la première fois qu'une version est trouvée, notification.
pub async fn check(app: &AppHandle) -> Status {
    if status(app).installing.is_some() {
        return status(app);
    }
    change(app, |s| {
        s.checking = true;
        s.error = None;
    });
    let result = match app.updater() {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };
    let state = app.state::<State>();
    let found = match result {
        Ok(Some(update)) => {
            let available = Available { version: update.version.clone(), notes: update.body.clone() };
            *state.update.lock().unwrap() = Some(update);
            Ok(Some(available))
        }
        Ok(None) => {
            *state.update.lock().unwrap() = None;
            Ok(None)
        }
        Err(e) => Err(e.to_string()),
    };
    if let Ok(Some(available)) = &found {
        let mut notified = state.notified.lock().unwrap();
        if notified.as_deref() != Some(available.version.as_str()) {
            *notified = Some(available.version.clone());
            notify::update_available(app, &available.version);
        }
    }
    change(app, |s| {
        s.checking = false;
        s.last_check_ms = Some(now_ms());
        match found {
            Ok(available) => s.available = available,
            Err(e) => {
                eprintln!("[morph] mise à jour : {e}");
                s.error = Some("Impossible de vérifier les mises à jour (connexion ?).".into());
            }
        }
    })
}

/// Raison d'attendre avant d'installer (l'application redémarre).
fn busy(app: &AppHandle) -> Option<&'static str> {
    if jobs::any_running(app) {
        return Some("Un traitement est en cours : la mise à jour pourra s'installer à la fin.");
    }
    if app.get_webview_window(tools::LABEL).is_some() {
        return Some("Un outil est ouvert : ferme-le avant d'installer la mise à jour.");
    }
    None
}

/// Télécharge et installe la version trouvée, puis redémarre Morph.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    if let Some(reason) = busy(app) {
        return Err(reason.into());
    }
    let Some(update) = app.state::<State>().update.lock().unwrap().clone() else {
        return Err("Aucune mise à jour à installer.".into());
    };
    change(app, |s| {
        s.installing = Some(0.0);
        s.error = None;
    });
    let mut received = 0u64;
    let handle = app.clone();
    let result = update
        .download_and_install(
            move |chunk, total| {
                received += chunk as u64;
                if let Some(total) = total.filter(|t| *t > 0) {
                    let p = (received as f32 / total as f32).min(1.0);
                    change(&handle, |s| s.installing = Some(p));
                }
            },
            || {},
        )
        .await;
    match result {
        Ok(()) => app.restart(),
        Err(e) => {
            eprintln!("[morph] installation de la mise à jour : {e}");
            change(app, |s| {
                s.installing = None;
                s.error = Some("Le téléchargement de la mise à jour a échoué : réessaie plus tard.".into());
            });
            Err(e.to_string())
        }
    }
}
