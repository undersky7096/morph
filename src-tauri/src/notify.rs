//! Notification système à la fin d'une conversion longue ; un clic ouvre le dossier.
//! Après quelques dizaines de conversions réussies, une seule invitation à laisser un avis.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::i18n::tr;
use crate::jobs::{Job, Status};

/// Message de fin : « 5 fichiers convertis en WEBP », « 3 sur 4 réussis, 1 erreur »…
fn summary(job: &Job) -> (String, String) {
    let count = |s: Status| job.files.iter().filter(|f| f.status == s).count();
    let (done, failed) = (count(Status::Done), count(Status::Error) + count(Status::Conflict));
    let s = |n: usize| if n > 1 { "s" } else { "" };
    let open = || tr("Clique pour ouvrir le dossier.", "Click to open the folder.").to_string();
    let target = job.target.to_uppercase();
    use crate::jobs::{ARCHIVE_7Z, ARCHIVE_ZIP, EXTRACT, MERGE};
    if failed == 0 && job.target == MERGE {
        (tr("PDF fusionnés", "PDFs merged").into(), open())
    } else if failed == 0 && (job.target == ARCHIVE_ZIP || job.target == ARCHIVE_7Z) {
        (tr("Archive créée", "Archive created").into(), open())
    } else if failed == 0 && job.target == EXTRACT {
        let title = if done > 1 {
            tr(format!("{done} archives extraites"), format!("{done} archives extracted"))
        } else {
            tr("Archive extraite", "Archive extracted").into()
        };
        (title, open())
    } else if failed == 0 {
        let title = match (done, job.target == crate::jobs::CUTOUT) {
            (1, true) => tr("Fond retiré", "Background removed").to_string(),
            (n, true) => tr(format!("Fond retiré de {n} fichiers"), format!("Background removed from {n} files")),
            (1, false) => tr(format!("Fichier converti en {target}"), format!("File converted to {target}")),
            (n, false) => tr(format!("{n} fichiers convertis en {target}"), format!("{n} files converted to {target}")),
        };
        (title, open())
    } else {
        let total = job.files.len();
        let body = tr(
            format!("{done} sur {total} réussi{}, {failed} erreur{} : détails dans Morph.", s(done), s(failed)),
            format!("{done} of {total} succeeded, {failed} error{}: details in Morph.", s(failed)),
        );
        let title = if job.target == crate::jobs::CUTOUT {
            tr("Détourage terminé avec des erreurs", "Background removal finished with errors")
        } else {
            tr("Conversion terminée avec des erreurs", "Conversion finished with errors")
        };
        (title.into(), body)
    }
}

/// Clic sur une notification.
enum OnClick {
    /// Montre le fichier dans l'explorateur.
    Reveal(PathBuf),
    /// Ouvre les Réglages sur « À propos » (mise à jour).
    Updates,
    /// Ouvre une page du site dans le navigateur.
    Url(&'static str),
}

fn activate(app: &AppHandle, click: &OnClick) {
    match click {
        OnClick::Reveal(path) => {
            let _ = tauri_plugin_opener::reveal_item_in_dir(path);
        }
        OnClick::Updates => {
            let _ = crate::tray::show_settings_at(app, Some("about"));
        }
        OnClick::Url(url) => {
            let _ = tauri_plugin_opener::open_url(*url, None::<&str>);
        }
    }
}

pub fn update_available(app: &AppHandle, version: &str) {
    show(
        app,
        &tr(format!("Morph {version} est disponible"), format!("Morph {version} is available")),
        tr("Clique pour voir les nouveautés et installer la mise à jour.", "Click to see what’s new and install the update."),
        Some(OnClick::Updates),
    );
}

/// Nombre de fichiers convertis avec succès avant d'inviter à laisser un avis.
const REVIEW_AFTER: u64 = 20;
pub const REVIEW_URL: &str = "https://morph.underskydev.com/reviews/#write";

/// Compteur gardé à part des réglages (`review.json`), pour ne jamais les toucher.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct ReviewState {
    converted: u64,
    asked: bool,
}

/// Compte les fichiers convertis ; au seuil atteint, une seule notification « laisse un avis ».
pub fn count_success(app: &AppHandle, done: usize, after_notification: bool) {
    if done == 0 {
        return;
    }
    let Ok(dir) = app.path().app_config_dir() else { return };
    let path = dir.join("review.json");
    let mut state: ReviewState = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
    if state.asked {
        return;
    }
    state.converted += done as u64;
    let ask = state.converted >= REVIEW_AFTER;
    state.asked = ask;
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(&path, serde_json::to_string(&state).unwrap_or_default());
    if ask {
        let app = app.clone();
        std::thread::spawn(move || {
            // Laisse d'abord passer la notification de fin de conversion.
            if after_notification {
                std::thread::sleep(std::time::Duration::from_secs(8));
            }
            show(
                &app,
                tr("Morph te rend service ?", "Is Morph useful to you?"),
                tr(
                    "Clique pour laisser un avis en 30 secondes : ça aide beaucoup Morph à se faire connaître.",
                    "Click to leave a review in 30 seconds: it really helps Morph get known.",
                ),
                Some(OnClick::Url(REVIEW_URL)),
            );
        });
    }
}

pub fn job_finished(app: &AppHandle, job: &Job) {
    // Tout annulé : rien à annoncer.
    if !job.files.iter().any(|f| matches!(f.status, Status::Done | Status::Error | Status::Conflict)) {
        return;
    }
    let first = job.files.iter().find_map(|f| f.output.as_ref().map(PathBuf::from));
    let (title, body) = summary(job);
    show(app, &title, &body, first.map(OnClick::Reveal));
}

#[cfg(windows)]
fn show(app: &AppHandle, title: &str, body: &str, click: Option<OnClick>) {
    use tauri_winrt_notification::Toast;
    // Installé : identifiant posé sur le raccourci par l'installeur ; sinon (développement),
    // celui de PowerShell, toujours enregistré.
    let id = if cfg!(debug_assertions) { Toast::POWERSHELL_APP_ID.to_string() } else { app.config().identifier.clone() };
    let handle = app.clone();
    let result = Toast::new(&id)
        .title(title)
        .text1(body)
        .on_activated(move |_| {
            if let Some(click) = &click {
                activate(&handle, click);
            }
            Ok(())
        })
        .show();
    if let Err(e) = result {
        eprintln!("[morph] notification : {e}");
    }
}

#[cfg(target_os = "linux")]
fn show(app: &AppHandle, title: &str, body: &str, click: Option<OnClick>) {
    let (title, body, app) = (title.to_string(), body.to_string(), app.clone());
    std::thread::spawn(move || {
        let mut n = notify_rust::Notification::new();
        n.summary(&title).body(&body).appname("Morph");
        match &click {
            Some(OnClick::Reveal(_)) => {
                n.action("default", tr("Ouvrir le dossier", "Open folder"));
            }
            Some(OnClick::Updates) => {
                n.action("default", tr("Voir la mise à jour", "See the update"));
            }
            Some(OnClick::Url(_)) => {
                n.action("default", tr("Ouvrir", "Open"));
            }
            None => {}
        }
        match n.show() {
            Ok(handle) => handle.wait_for_action(|action| {
                if action == "default" {
                    if let Some(click) = &click {
                        activate(&app, click);
                    }
                }
            }),
            Err(e) => eprintln!("[morph] notification : {e}"),
        }
    });
}

#[cfg(not(any(windows, target_os = "linux")))]
fn show(_app: &AppHandle, _title: &str, _body: &str, _click: Option<OnClick>) {}
