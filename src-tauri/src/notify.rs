//! Notification système à la fin d'une conversion longue ; un clic ouvre le dossier.

use std::path::PathBuf;

use tauri::AppHandle;

use crate::jobs::{Job, Status};

/// Message de fin : « 5 fichiers convertis en WEBP », « 3 sur 4 réussis, 1 erreur »…
fn summary(job: &Job) -> (String, String) {
    let count = |s: Status| job.files.iter().filter(|f| f.status == s).count();
    let (done, failed) = (count(Status::Done), count(Status::Error) + count(Status::Conflict));
    let s = |n: usize| if n > 1 { "s" } else { "" };
    if failed == 0 {
        let title = match (done, job.target == crate::jobs::CUTOUT) {
            (1, true) => "Fond retiré".to_string(),
            (n, true) => format!("Fond retiré de {n} images"),
            (1, false) => format!("Fichier converti en {}", job.target.to_uppercase()),
            (n, false) => format!("{n} fichiers convertis en {}", job.target.to_uppercase()),
        };
        (title, "Clique pour ouvrir le dossier.".into())
    } else {
        let body = format!("{done} sur {} réussi{}, {failed} erreur{} : détails dans Morph.", job.files.len(), s(done), s(failed));
        let title = if job.target == crate::jobs::CUTOUT { "Détourage terminé avec des erreurs" } else { "Conversion terminée avec des erreurs" };
        (title.into(), body)
    }
}

pub fn job_finished(app: &AppHandle, job: &Job) {
    // Tout annulé : rien à annoncer.
    if !job.files.iter().any(|f| matches!(f.status, Status::Done | Status::Error | Status::Conflict)) {
        return;
    }
    let first: Option<PathBuf> = job.files.iter().find_map(|f| f.output.as_ref().map(PathBuf::from));
    let (title, body) = summary(job);
    show(app, &title, &body, first);
}

#[cfg(windows)]
fn show(app: &AppHandle, title: &str, body: &str, reveal: Option<PathBuf>) {
    use tauri_winrt_notification::Toast;
    // Installé : identifiant posé sur le raccourci par l'installeur ; sinon (développement),
    // celui de PowerShell, toujours enregistré.
    let id = if cfg!(debug_assertions) { Toast::POWERSHELL_APP_ID.to_string() } else { app.config().identifier.clone() };
    let result = Toast::new(&id)
        .title(title)
        .text1(body)
        .on_activated(move |_| {
            if let Some(path) = &reveal {
                let _ = tauri_plugin_opener::reveal_item_in_dir(path);
            }
            Ok(())
        })
        .show();
    if let Err(e) = result {
        eprintln!("[morph] notification : {e}");
    }
}

#[cfg(target_os = "linux")]
fn show(_app: &AppHandle, title: &str, body: &str, reveal: Option<PathBuf>) {
    let (title, body) = (title.to_string(), body.to_string());
    std::thread::spawn(move || {
        let mut n = notify_rust::Notification::new();
        n.summary(&title).body(&body).appname("Morph");
        if reveal.is_some() {
            n.action("default", "Ouvrir le dossier");
        }
        match n.show() {
            Ok(handle) => handle.wait_for_action(|action| {
                if action == "default" {
                    if let Some(path) = &reveal {
                        let _ = tauri_plugin_opener::reveal_item_in_dir(path);
                    }
                }
            }),
            Err(e) => eprintln!("[morph] notification : {e}"),
        }
    });
}

#[cfg(not(any(windows, target_os = "linux")))]
fn show(_app: &AppHandle, _title: &str, _body: &str, _reveal: Option<PathBuf>) {}
