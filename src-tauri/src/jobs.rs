//! Conversions en arrière-plan (lancées depuis le menu radial) : progression par fichier,
//! annulation (tout ou un fichier), réessai, doublons à trancher. Le détourage (cible
//! [`CUTOUT`]) passe par le même chemin.
//!
//! Chaque changement est diffusé par l'événement `job-update` (instantané complet). Le
//! menu suit sa conversion dans le moyeu ; si elle porte sur plusieurs fichiers et s'annonce
//! longue (ou se passe mal), elle est « détachée » : le menu se ferme et la mini-fenêtre de
//! progression prend le relais. Un fichier seul reste suivi dans le moyeu.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::convert::output::{self, Duplicates, Scope};
use crate::convert::{self, ErrorInfo};
use crate::edit::cutout;
use crate::{notify, progress, settings};

/// Cible spéciale : retirer le fond (PNG transparent, WebM pour une vidéo) au lieu de convertir.
pub const CUTOUT: &str = "cutout";
/// Cibles spéciales : fusionner des PDF, archiver la sélection, extraire une archive.
pub use crate::convert::archive::{ARCHIVE_7Z, ARCHIVE_ZIP, EXTRACT};
pub use crate::convert::pdf::MERGE;

/// Un lot qui mettra plus que ça à atteindre la moitié passe dans la mini-fenêtre.
const HALFWAY_LIMIT: f32 = 10.0;
/// Temps d'observation avant d'estimer la durée (les premières mesures sont trompeuses).
const ESTIMATE_AFTER: f32 = 1.0;
/// Intervalle minimal entre deux instantanés de progression.
const THROTTLE: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Pending,
    Running,
    Done,
    Error,
    /// Le fichier de sortie existe déjà (réglage « demander ») : en attente d'un choix.
    Conflict,
    Cancelled,
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileState {
    pub input: String,
    /// Nombre de fichiers traités ensemble (fusion, images → PDF) ; 1 sinon.
    pub count: usize,
    /// Tous les fichiers d'une opération sur le lot entier (le premier est `input`).
    #[serde(skip)]
    inputs: Vec<String>,
    pub status: Status,
    pub progress: f32,
    pub output: Option<String>,
    pub error: Option<ErrorInfo>,
    #[serde(skip)]
    cancel: Arc<AtomicBool>,
    /// Choix fait pour un doublon (remplacer / garder les deux), appliqué au réessai.
    #[serde(skip)]
    duplicates: Option<Duplicates>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: u64,
    pub target: String,
    pub files: Vec<FileState>,
    /// Précision affichée pendant le traitement (ex : premier téléchargement du modèle).
    pub note: Option<String>,
    /// Début, en millisecondes depuis l'époque Unix (pour le temps restant).
    pub started_ms: u64,
    pub finished: bool,
    /// Suivie dans la mini-fenêtre plutôt que dans le menu.
    pub detached: bool,
    #[serde(skip)]
    started: Instant,
    #[serde(skip)]
    cancel: Arc<AtomicBool>,
    #[serde(skip)]
    out_dir: Option<PathBuf>,
    #[serde(skip)]
    duplicates: Duplicates,
    #[serde(skip)]
    running: bool,
}

#[derive(Default)]
pub struct Jobs(Mutex<HashMap<u64, Job>>);

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn with_job<T>(app: &AppHandle, id: u64, f: impl FnOnce(&mut Job) -> T) -> Option<T> {
    app.state::<Jobs>().0.lock().unwrap().get_mut(&id).map(f)
}

fn emit(app: &AppHandle, id: u64) {
    if let Some(job) = with_job(app, id, |j| j.clone()) {
        let _ = app.emit("job-update", job);
    }
}

/// Un traitement est en cours (conversion, détourage, ou opération d'un outil).
pub fn any_running(app: &AppHandle) -> bool {
    app.state::<Jobs>().0.lock().unwrap().values().any(|j| !j.finished)
        || !app.state::<Tasks>().0.lock().unwrap().is_empty()
}

pub fn list(app: &AppHandle) -> Vec<Job> {
    let mut jobs: Vec<Job> = app.state::<Jobs>().0.lock().unwrap().values().cloned().collect();
    jobs.sort_by_key(|j| j.id);
    jobs
}

/// Démarre la conversion de `paths` vers `target` ; renvoie son premier instantané.
pub fn start(app: &AppHandle, paths: Vec<String>, target: String, out_dir: Option<PathBuf>, duplicates: Duplicates) -> Job {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let quality = settings::get(app).cutout;
    let note = (target == CUTOUT && !cutout::model_ready(quality)).then(|| {
        crate::i18n::tr(
            format!("Premier détourage : téléchargement du modèle ({}, une seule fois).", quality.download_size()),
            format!("First background removal: downloading the model ({}, only once).", quality.download_size()),
        )
    });
    // Fusion, images → PDF : un seul fichier produit pour tout le lot.
    let groups: Vec<Vec<String>> = if convert::is_batch(&target, &paths) {
        vec![paths]
    } else {
        paths.into_iter().map(|p| vec![p]).collect()
    };
    let job = Job {
        id,
        target,
        note,
        files: groups
            .into_iter()
            .map(|inputs| FileState {
                input: inputs[0].clone(),
                count: inputs.len(),
                inputs,
                status: Status::Pending,
                progress: 0.0,
                output: None,
                error: None,
                cancel: Arc::default(),
                duplicates: None,
            })
            .collect(),
        started_ms: now_ms(),
        finished: false,
        detached: false,
        started: Instant::now(),
        cancel: Arc::default(),
        out_dir,
        duplicates,
        running: false,
    };
    app.state::<Jobs>().0.lock().unwrap().insert(id, job.clone());
    spawn(app, id);
    job
}

/// Lance le traitement des fichiers en attente, s'il ne tourne pas déjà.
fn spawn(app: &AppHandle, id: u64) {
    let launch = with_job(app, id, |j| {
        let launch = !j.running;
        j.running = true;
        j.finished = false;
        launch
    });
    if launch == Some(true) {
        let app = app.clone();
        std::thread::spawn(move || work(&app, id));
    }
}

/// Lot de plusieurs fichiers qui mettra plus de [`HALFWAY_LIMIT`] secondes à atteindre 50 %,
/// d'après le rythme depuis le début. Un fichier seul n'est jamais détaché (sauf erreur).
fn long_batch(job: &Job) -> bool {
    let inputs: usize = job.files.iter().map(|f| f.count).sum();
    if inputs < 2 {
        return false;
    }
    let kept: Vec<&FileState> = job.files.iter().filter(|f| !matches!(f.status, Status::Cancelled | Status::Skipped)).collect();
    let done: f32 = kept
        .iter()
        .map(|f| match f.status {
            Status::Running => f.progress,
            Status::Pending => 0.0,
            _ => 1.0,
        })
        .sum();
    let progress = done / kept.len().max(1) as f32;
    let elapsed = job.started.elapsed().as_secs_f32();
    if progress >= 0.5 {
        return false;
    }
    elapsed >= HALFWAY_LIMIT || (elapsed >= ESTIMATE_AFTER && progress > 0.0 && elapsed * 0.5 / progress > HALFWAY_LIMIT)
}

/// Passe la conversion dans la mini-fenêtre (une seule fois).
pub fn detach(app: &AppHandle, id: u64) {
    if with_job(app, id, |j| std::mem::replace(&mut j.detached, true)) == Some(false) {
        emit(app, id);
        if let Err(e) = progress::show(app) {
            eprintln!("[morph] fenêtre de progression : {e}");
        }
    }
}

fn work(app: &AppHandle, id: u64) {
    loop {
        // Fichier suivant en attente (ou fin).
        let next = with_job(app, id, |j| {
            let all = j.cancel.load(Ordering::Relaxed);
            let i = j.files.iter().position(|f| f.status == Status::Pending)?;
            let f = &mut j.files[i];
            if all {
                f.status = Status::Cancelled;
                return Some(None);
            }
            f.status = Status::Running;
            f.progress = 0.0;
            f.error = None;
            let scope = Scope {
                dir: j.out_dir.clone(),
                duplicates: f.duplicates.unwrap_or(j.duplicates),
                cancel: vec![j.cancel.clone(), f.cancel.clone()],
            };
            Some(Some((i, f.inputs.clone(), j.target.clone(), scope)))
        })
        .flatten();
        let Some(next) = next else { break };
        let Some((index, inputs, target, scope)) = next else { continue };
        let input = inputs[0].clone();
        emit(app, id);

        let mut last = Instant::now();
        let mut report = |p: f32| {
            let detach_now = with_job(app, id, |j| {
                j.files[index].progress = p;
                !j.detached && long_batch(j)
            });
            if detach_now == Some(true) {
                detach(app, id);
            }
            if last.elapsed() >= THROTTLE {
                last = Instant::now();
                emit(app, id);
            }
        };
        let result = output::scoped(scope, || {
            if target == CUTOUT {
                cutout::remove_background(Path::new(&input), settings::get(app).cutout, &mut report)
            } else if target == EXTRACT {
                convert::archive::extract(Path::new(&input), &mut report)
            } else if inputs.len() > 1 || convert::is_batch(&target, &inputs) {
                let inputs: Vec<PathBuf> = inputs.iter().map(PathBuf::from).collect();
                convert::run_batch(&inputs, &target, &mut report)
            } else {
                convert::convert(Path::new(&input), &target, &mut report)
            }
        });
        with_job(app, id, |j| {
            let f = &mut j.files[index];
            f.progress = 1.0;
            match result {
                Ok(out) => {
                    f.status = Status::Done;
                    f.output = Some(out.display().to_string());
                }
                Err(e) => {
                    let info = e.info();
                    f.status = match info.code {
                        "cancelled" => Status::Cancelled,
                        "exists" => Status::Conflict,
                        _ => Status::Error,
                    };
                    f.error = Some(info);
                }
            }
            f.cancel.store(false, Ordering::Relaxed);
        });
        emit(app, id);
    }

    // Fin : erreur ou doublon à trancher → mini-fenêtre ; longue conversion → notification.
    let Some(job) = with_job(app, id, |j| {
        j.running = false;
        j.finished = true;
        j.clone()
    }) else {
        return;
    };
    let needs_attention = job.files.iter().any(|f| matches!(f.status, Status::Error | Status::Conflict));
    if needs_attention {
        detach(app, id);
    }
    emit(app, id);
    if job.detached {
        notify::job_finished(app, &job);
    }
    let done = job.files.iter().filter(|f| f.status == Status::Done).count();
    notify::count_success(app, done, job.detached);
}

pub fn cancel(app: &AppHandle, id: u64, index: Option<usize>) {
    with_job(app, id, |j| match index {
        None => {
            j.cancel.store(true, Ordering::Relaxed);
            for f in &mut j.files {
                if matches!(f.status, Status::Pending | Status::Conflict) {
                    f.status = Status::Cancelled;
                }
            }
        }
        Some(i) => {
            if let Some(f) = j.files.get_mut(i) {
                match f.status {
                    Status::Running => f.cancel.store(true, Ordering::Relaxed),
                    Status::Pending | Status::Conflict => f.status = Status::Cancelled,
                    _ => {}
                }
            }
        }
    });
    emit(app, id);
}

/// Remet des fichiers en attente (réessai, ou doublon tranché) et relance le traitement.
/// `index` : un fichier, ou tous ceux qui sont dans l'état concerné.
pub fn retry(app: &AppHandle, id: u64, index: Option<usize>, duplicates: Option<Duplicates>) {
    with_job(app, id, |j| {
        j.cancel.store(false, Ordering::Relaxed);
        for (i, f) in j.files.iter_mut().enumerate() {
            let wanted = match (index, duplicates) {
                (Some(k), _) => i == k,
                (None, Some(_)) => f.status == Status::Conflict,
                (None, None) => matches!(f.status, Status::Error | Status::Cancelled),
            };
            if wanted && f.status != Status::Running && f.status != Status::Done {
                f.status = Status::Pending;
                f.error = None;
                f.progress = 0.0;
                if duplicates.is_some() {
                    f.duplicates = duplicates;
                }
            }
        }
    });
    spawn(app, id);
    emit(app, id);
}

/// Doublon(s) laissé(s) tel(s) quel(s).
pub fn skip(app: &AppHandle, id: u64, index: Option<usize>) {
    with_job(app, id, |j| {
        for (i, f) in j.files.iter_mut().enumerate() {
            if f.status == Status::Conflict && index.is_none_or(|k| k == i) {
                f.status = Status::Skipped;
            }
        }
    });
    emit(app, id);
}

/// Oublie les conversions terminées (la mini-fenêtre se vide).
pub fn clear_finished(app: &AppHandle) {
    app.state::<Jobs>().0.lock().unwrap().retain(|_, j| !j.finished);
}

/// Tâches d'outils annulables (éditeurs, compression), désignées par un identifiant
/// choisi par l'interface.
#[derive(Default)]
pub struct Tasks(Mutex<HashMap<String, Arc<AtomicBool>>>);

pub fn task_flag(app: &AppHandle, id: &str) -> Arc<AtomicBool> {
    app.state::<Tasks>().0.lock().unwrap().entry(id.to_string()).or_default().clone()
}

pub fn cancel_task(app: &AppHandle, id: &str) {
    if let Some(flag) = app.state::<Tasks>().0.lock().unwrap().get(id) {
        flag.store(true, Ordering::Relaxed);
    }
}

pub fn end_task(app: &AppHandle, id: &str) {
    app.state::<Tasks>().0.lock().unwrap().remove(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(files: &[(Status, f32, usize)], elapsed_secs: f32) -> Job {
        Job {
            id: 1,
            target: "webp".into(),
            files: files
                .iter()
                .map(|&(status, progress, count)| FileState {
                    input: "a.png".into(),
                    count,
                    inputs: vec!["a.png".into(); count],
                    status,
                    progress,
                    output: None,
                    error: None,
                    cancel: Arc::default(),
                    duplicates: None,
                })
                .collect(),
            note: None,
            started_ms: 0,
            finished: false,
            detached: false,
            started: Instant::now() - Duration::from_secs_f32(elapsed_secs),
            cancel: Arc::default(),
            out_dir: None,
            duplicates: Duplicates::Keep,
            running: true,
        }
    }

    #[test]
    fn only_long_batches_are_detached() {
        use Status::*;
        // Un fichier seul, même long : reste dans le moyeu.
        assert!(!long_batch(&job(&[(Running, 0.1, 1)], 30.0)));
        // Lot rapide : 40 % en 2 s, la moitié vers 2,5 s.
        assert!(!long_batch(&job(&[(Done, 0.0, 1), (Running, 0.6, 1), (Pending, 0.0, 1)], 4.0)));
        // Lot lent : 10 % en 3 s, la moitié vers 15 s.
        assert!(long_batch(&job(&[(Running, 0.2, 1), (Pending, 0.0, 1)], 3.0)));
        // Trop tôt pour estimer.
        assert!(!long_batch(&job(&[(Running, 0.01, 1), (Pending, 0.0, 1)], 0.5)));
        // Plus de 10 s sans atteindre la moitié.
        assert!(long_batch(&job(&[(Running, 0.0, 1), (Pending, 0.0, 1)], 11.0)));
        // Moitié passée : plus de détachement.
        assert!(!long_batch(&job(&[(Done, 0.0, 1), (Running, 0.1, 1)], 30.0)));
        // Fusion de plusieurs PDF : un seul résultat, mais plusieurs fichiers.
        assert!(long_batch(&job(&[(Running, 0.05, 3)], 2.0)));
    }
}
