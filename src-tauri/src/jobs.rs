//! Conversions en arrière-plan (lancées depuis le menu radial) : progression par fichier,
//! annulation (tout ou un fichier), réessai, doublons à trancher.
//!
//! Chaque changement est diffusé par l'événement `job-update` (instantané complet). Le
//! menu suit sa conversion dans le moyeu ; si elle dure (ou se passe mal), elle est
//! « détachée » : le menu se ferme et la mini-fenêtre de progression prend le relais.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::convert::output::{self, Duplicates, Scope};
use crate::convert::{self, ErrorInfo};
use crate::{notify, progress};

/// Au-delà, la conversion passe dans la mini-fenêtre.
const DETACH_AFTER: Duration = Duration::from_secs(2);
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

pub fn list(app: &AppHandle) -> Vec<Job> {
    let mut jobs: Vec<Job> = app.state::<Jobs>().0.lock().unwrap().values().cloned().collect();
    jobs.sort_by_key(|j| j.id);
    jobs
}

/// Démarre la conversion de `paths` vers `target` ; renvoie son premier instantané.
pub fn start(app: &AppHandle, paths: Vec<String>, target: String, out_dir: Option<PathBuf>, duplicates: Duplicates) -> Job {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let job = Job {
        id,
        target,
        files: paths
            .into_iter()
            .map(|input| FileState {
                input,
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
            Some(Some((i, f.input.clone(), j.target.clone(), scope)))
        })
        .flatten();
        let Some(next) = next else { break };
        let Some((index, input, target, scope)) = next else { continue };
        emit(app, id);

        let mut last = Instant::now();
        let mut report = |p: f32| {
            let detach_now = with_job(app, id, |j| {
                j.files[index].progress = p;
                !j.detached && j.started.elapsed() > DETACH_AFTER
            });
            if detach_now == Some(true) {
                detach(app, id);
            }
            if last.elapsed() >= THROTTLE {
                last = Instant::now();
                emit(app, id);
            }
        };
        let result = output::scoped(scope, || convert::convert(Path::new(&input), &target, &mut report));
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
