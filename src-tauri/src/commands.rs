use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager};

use crate::convert::output::{self, Duplicates, Scope};
use crate::convert::{ConvertError, ErrorInfo, catalog, formats};
use crate::edit::{self, compress, trim, video};
use crate::{jobs, progress, radial, settings, tools};

/// Options d'une opération qui écrit un fichier, transmises par l'interface.
#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RunOptions {
    /// Dossier choisi par l'utilisateur (réglage « demander à chaque fois »).
    out_dir: Option<String>,
    /// Choix fait pour un doublon, à la place du réglage.
    duplicates: Option<Duplicates>,
    /// Identifiant d'annulation (voir `cancel_task`).
    task: Option<String>,
}

fn scope(app: &AppHandle, run: &RunOptions) -> Scope {
    let s = settings::get(app);
    Scope {
        dir: s.output_dir(run.out_dir.as_deref()),
        duplicates: run.duplicates.unwrap_or(s.duplicates),
        cancel: run.task.iter().map(|t| jobs::task_flag(app, t)).collect(),
    }
}

/// Opération d'un outil, hors du fil de l'interface, dans le contexte de `run`.
async fn run_tool<T: Send + 'static>(
    app: &AppHandle,
    run: Option<RunOptions>,
    f: impl FnOnce() -> Result<T, ConvertError> + Send + 'static,
) -> Result<T, ConvertError> {
    let run = run.unwrap_or_default();
    let scope = scope(app, &run);
    let result = blocking(move || output::scoped(scope, f)).await;
    if let Some(task) = &run.task {
        jobs::end_task(app, task);
    }
    result
}

/// Table des formats, pour l'interface (type d'un fichier, couleur des pétales, outils).
#[tauri::command]
pub fn format_catalog() -> &'static [catalog::Format] {
    catalog::FORMATS
}

#[tauri::command]
pub fn get_targets(paths: Vec<String>) -> formats::TargetMenu {
    let paths: Vec<&Path> = paths.iter().map(Path::new).collect();
    formats::menu_for(&paths)
}

#[tauri::command]
pub fn open_radial(app: AppHandle, paths: Vec<String>) -> Result<(), String> {
    radial::show(&app, paths).map_err(|e| e.to_string())
}

// Async : appelée depuis la fenêtre d'outil, qui se ferme juste après.
#[tauri::command]
pub async fn open_actions(app: AppHandle, paths: Vec<String>) -> Result<(), String> {
    radial::show_with(&app, paths, radial::Ring::Saved).map_err(|e| e.to_string())
}

/// Outil fermé sans enregistrer : retour au choix des outils pour les mêmes fichiers.
#[tauri::command]
pub async fn open_tools(app: AppHandle, paths: Vec<String>) -> Result<(), String> {
    radial::show_with(&app, paths, radial::Ring::Tools).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn take_radial_payload(app: AppHandle) -> Option<radial::RadialPayload> {
    radial::take_pending(&app)
}

#[tauri::command]
pub fn close_radial(app: AppHandle) -> Result<(), String> {
    radial::hide(&app).map_err(|e| e.to_string())
}

#[derive(Clone, Serialize)]
pub struct Progress {
    index: usize,
    input: String,
    progress: f32,
}

/// Lance une conversion en arrière-plan ; suivie par l'événement `job-update`.
#[tauri::command]
pub fn start_conversion(app: AppHandle, paths: Vec<String>, target: String, run: Option<RunOptions>) -> jobs::Job {
    let s = scope(&app, &run.unwrap_or_default());
    jobs::start(&app, paths, target, s.dir, s.duplicates)
}

#[tauri::command]
pub fn list_jobs(app: AppHandle) -> Vec<jobs::Job> {
    jobs::list(&app)
}

/// Annule une conversion entière, ou un seul de ses fichiers.
#[tauri::command]
pub fn cancel_job(app: AppHandle, id: u64, index: Option<usize>) {
    jobs::cancel(&app, id, index);
}

/// Réessaie un fichier (ou tous ceux en erreur) ; avec `duplicates`, tranche les doublons.
#[tauri::command]
pub fn retry_job(app: AppHandle, id: u64, index: Option<usize>, duplicates: Option<Duplicates>) {
    jobs::retry(&app, id, index, duplicates);
}

/// Doublons ignorés : le fichier existant est laissé tel quel.
#[tauri::command]
pub fn skip_conflicts(app: AppHandle, id: u64, index: Option<usize>) {
    jobs::skip(&app, id, index);
}

/// La conversion continue dans la mini-fenêtre (le menu a été fermé pendant qu'elle tournait).
// Async : peut créer la fenêtre (voir `open_tool`).
#[tauri::command]
pub async fn detach_job(app: AppHandle, id: u64) {
    jobs::detach(&app, id);
}

#[tauri::command]
pub fn clear_jobs(app: AppHandle) {
    jobs::clear_finished(&app);
}

// Async : crée la fenêtre si besoin (voir `open_tool`).
#[tauri::command]
pub async fn show_progress(app: AppHandle) -> Result<(), String> {
    progress::show(&app).map_err(|e| e.to_string())
}

/// Annule l'opération d'un outil (identifiant passé dans ses `RunOptions`).
#[tauri::command]
pub fn cancel_task(app: AppHandle, task: String) {
    jobs::cancel_task(&app, &task);
}

#[tauri::command]
pub fn update_status(app: AppHandle) -> crate::updates::Status {
    crate::updates::status(&app)
}

/// « Rechercher une mise à jour » (Réglages).
#[tauri::command]
pub async fn check_update(app: AppHandle) -> crate::updates::Status {
    crate::updates::check(&app).await
}

/// Installe la mise à jour trouvée ; Morph redémarre ensuite.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    crate::updates::install(&app).await
}

/// Interface des Réglages prête : la fenêtre s'affiche.
#[tauri::command]
pub fn reveal_settings(app: AppHandle) -> Result<(), String> {
    crate::tray::reveal_settings(&app).map_err(|e| e.to_string())
}

/// Liens de la page « À propos » : seules ces adresses peuvent être ouvertes.
const LINKS: &[&str] = &[
    "https://morph.underskydev.com",
    "https://morph.underskydev.com/reviews/#write",
    "https://morph.underskydev.com/changelog/",
    "https://morph.underskydev.com/roadmap/",
    "mailto:contact@underskydev.com",
    "https://ffmpeg.org",
    "https://tauri.app",
    "https://react.dev",
    "https://motion.dev",
    "https://konvajs.org",
    "https://github.com/image-rs/image",
    "https://github.com/shssoichiro/oxipng",
    "https://pngquant.org/lib/",
    "https://onnxruntime.ai",
    "https://github.com/xuebinqin/DIS",
    "https://github.com/ZhengPeng7/BiRefNet",
    "https://pandoc.org",
    "https://pdfium.googlesource.com/pdfium/",
    "https://github.com/J-F-Liu/lopdf",
    "https://github.com/linebender/resvg",
    "https://www.libreoffice.org",
    "https://github.com/zip-rs/zip2",
    crate::convert::engines::LIBREOFFICE_URL,
];

/// Ouvre un lien de la page « À propos » dans le navigateur (ou la messagerie).
#[tauri::command]
pub fn open_link(url: String) -> Result<(), String> {
    if !LINKS.contains(&url.as_str()) {
        return Err(format!("lien non autorisé : {url}"));
    }
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}

/// Montre un fichier dans l'explorateur.
#[tauri::command]
pub fn reveal(path: String) -> Result<(), String> {
    tauri_plugin_opener::reveal_item_in_dir(path).map_err(|e| e.to_string())
}

// Async : sous Windows, créer une fenêtre depuis une commande synchrone bloque l'application.
#[tauri::command]
pub async fn open_tool(app: AppHandle, tool: String, paths: Vec<String>) -> Result<(), String> {
    tools::open(&app, tool, paths).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn take_tool_payload(app: AppHandle) -> Option<tools::ToolPayload> {
    tools::take_pending(&app)
}

/// Contenu brut d'une image, pour l'aperçu dans les outils (limité aux images reconnues).
#[tauri::command]
pub async fn read_image(path: String) -> Result<Response, ConvertError> {
    let path = PathBuf::from(path);
    if formats::kind_of(&path) != Some(formats::Kind::Image) {
        return Err(ConvertError::Unsupported(path.display().to_string()));
    }
    Ok(Response::new(read_file(path).await?))
}

/// Contenu brut d'un fichier audio ou vidéo, lu par le lecteur des éditeurs.
#[tauri::command]
pub async fn read_media(path: String) -> Result<Response, ConvertError> {
    let path = PathBuf::from(path);
    if !matches!(formats::kind_of(&path), Some(formats::Kind::Audio | formats::Kind::Video)) {
        return Err(ConvertError::Unsupported(path.display().to_string()));
    }
    Ok(Response::new(read_file(path).await?))
}

async fn read_file(path: PathBuf) -> std::io::Result<Vec<u8>> {
    tauri::async_runtime::spawn_blocking(move || std::fs::read(path))
        .await
        .map_err(std::io::Error::other)?
}

#[tauri::command]
pub async fn crop_image(
    app: AppHandle,
    path: String,
    rect: edit::Rect,
    run: Option<RunOptions>,
) -> Result<String, ConvertError> {
    run_tool(&app, run, move || edit::crop(Path::new(&path), rect))
        .await
        .map(|out| out.display().to_string())
}

/// Image détourée pour l'éditeur d'image (PNG). Progression via l'événement `cutout-progress`
/// (le premier détourage télécharge le modèle).
#[tauri::command]
pub async fn cutout_image(app: AppHandle, path: String, run: Option<RunOptions>) -> Result<Response, ConvertError> {
    let handle = app.clone();
    let quality = settings::get(&app).cutout;
    run_tool(&app, run, move || {
        edit::cutout::cutout_png(Path::new(&path), quality, &mut |p| {
            let _ = handle.emit("cutout-progress", p);
        })
    })
    .await
    .map(Response::new)
}

/// Poids du modèle de détourage choisi s'il reste à télécharger (« 970 Mo »), sinon rien.
#[tauri::command]
pub fn cutout_download(app: AppHandle) -> Option<&'static str> {
    let quality = settings::get(&app).cutout;
    (!edit::cutout::model_ready(quality)).then(|| quality.download_size())
}

/// Enregistre le rendu de l'éditeur image (PNG brut dans le corps de la requête,
/// chemin d'origine dans l'en-tête `x-path`, `RunOptions` en JSON dans `x-run`) au
/// format d'origine, suffixe « Edited ». En-tête `x-transparent` : le rendu a de la
/// transparence (fond retiré), gardée en PNG si le format d'origine n'en a pas.
#[tauri::command]
pub async fn save_edited(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<String, ConvertError> {
    let tauri::ipc::InvokeBody::Raw(png) = request.body() else {
        return Err(ConvertError::Unsupported("corps de requête attendu : PNG brut".into()));
    };
    let path = request
        .headers()
        .get("x-path")
        .and_then(|v| v.to_str().ok())
        .map(percent_decode)
        .ok_or_else(|| ConvertError::Unsupported("en-tête x-path manquant".into()))?;
    let run = request
        .headers()
        .get("x-run")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| serde_json::from_str(&percent_decode(v)).ok());
    let transparent = request.headers().contains_key("x-transparent");
    let png = png.clone();
    run_tool(&app, run, move || edit::save_edited(Path::new(&path), &png, transparent))
        .await
        .map(|out| out.display().to_string())
}

/// Les en-têtes HTTP sont en ASCII : le chemin y est encodé par `encodeURIComponent`.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Forme d'onde : durée en secondes (f64, petit-boutiste) suivie d'un pic (0-255) par 10 ms.
#[tauri::command]
pub async fn audio_peaks(path: String) -> Result<Response, ConvertError> {
    let (duration, peaks) = blocking(move || trim::peaks(Path::new(&path))).await?;
    let mut bytes = duration.to_le_bytes().to_vec();
    bytes.extend(peaks);
    Ok(Response::new(bytes))
}

/// Version WAV d'un fichier audio, pour l'écoute quand le webview ne lit pas son format.
#[tauri::command]
pub async fn audio_preview(path: String) -> Result<Response, ConvertError> {
    blocking(move || trim::preview_wav(Path::new(&path))).await.map(Response::new)
}

/// Garde les segments donnés d'un fichier audio, au volume donné (0…2). Progression via l'événement `trim-progress`.
#[tauri::command]
pub async fn trim_audio(
    app: AppHandle,
    path: String,
    segments: Vec<trim::Segment>,
    volume: Option<f64>,
    run: Option<RunOptions>,
) -> Result<String, ConvertError> {
    let handle = app.clone();
    run_tool(&app, run, move || {
        trim::trim(Path::new(&path), &segments, volume.unwrap_or(1.0), &mut |p| {
            let _ = handle.emit("trim-progress", p);
        })
    })
    .await
    .map(|out| out.display().to_string())
}

/// Ouvre l'accès d'un fichier vidéo au protocole `asset` (lecture en continu par le lecteur,
/// sans tout charger en mémoire) et le renvoie. Seuls les fichiers demandés sont autorisés.
#[tauri::command]
pub fn allow_media(app: AppHandle, path: String) -> Result<String, ConvertError> {
    let p = Path::new(&path);
    if !matches!(formats::kind_of(p), Some(formats::Kind::Audio | formats::Kind::Video)) {
        return Err(ConvertError::Unsupported(path));
    }
    app.asset_protocol_scope()
        .allow_file(p)
        .map_err(|e| ConvertError::Io(std::io::Error::other(e)))?;
    Ok(path)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    duration: f64,
    width: u32,
    height: u32,
    has_audio: bool,
}

/// Durée, dimensions affichées et présence de son d'une vidéo.
#[tauri::command]
pub async fn video_info(path: String) -> Result<VideoInfo, ConvertError> {
    let i = blocking(move || video::info(Path::new(&path))).await?;
    Ok(VideoInfo { duration: i.duration, width: i.width, height: i.height, has_audio: i.has_audio })
}

/// Copie lisible par le webview d'une vidéo dans un conteneur qu'il ne lit pas (AVI, WMV…).
#[tauri::command]
pub async fn video_preview(app: AppHandle, path: String) -> Result<String, ConvertError> {
    let out = blocking(move || video::preview(Path::new(&path))).await?;
    allow_media(app, out.display().to_string())
}

/// Enregistre une vidéo éditée. Progression via l'événement `edit-progress`.
#[tauri::command]
pub async fn edit_video(
    app: AppHandle,
    path: String,
    edit: video::VideoEdit,
    run: Option<RunOptions>,
) -> Result<String, ConvertError> {
    let handle = app.clone();
    run_tool(&app, run, move || {
        video::edit(Path::new(&path), &edit, &mut |p| {
            let _ = handle.emit("edit-progress", p);
        })
    })
    .await
    .map(|out| out.display().to_string())
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, ConvertError> + Send + 'static,
) -> Result<T, ConvertError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| ConvertError::Io(std::io::Error::other(e)))?
}

#[derive(Serialize)]
pub struct CompressResult {
    input: String,
    outcome: Option<compress::Outcome>,
    error: Option<ErrorInfo>,
}

/// Compresse un lot. Progression émise via l'événement `compress-progress`.
#[tauri::command]
pub async fn compress_files(
    app: AppHandle,
    paths: Vec<String>,
    options: compress::Options,
    run: Option<RunOptions>,
) -> Vec<CompressResult> {
    let run = run.unwrap_or_default();
    let scope = scope(&app, &run);
    let handle = app.clone();
    let results = tauri::async_runtime::spawn_blocking(move || {
        let app = handle;
        paths
            .into_iter()
            .enumerate()
            .map(|(index, input)| {
                let mut report = |progress: f32| {
                    let _ = app.emit("compress-progress", Progress { index, input: input.clone(), progress });
                };
                // Un contexte par fichier : un fichier remplacé est restauré si le sien échoue.
                let result = output::scoped(scope.clone(), || compress::compress(Path::new(&input), &options, &mut report));
                match result {
                    Ok(outcome) => CompressResult { input, outcome: Some(outcome), error: None },
                    Err(e) => CompressResult { input, outcome: None, error: Some(e.info()) },
                }
            })
            .collect()
    })
    .await
    .unwrap_or_default();
    if let Some(task) = &run.task {
        jobs::end_task(&app, task);
    }
    results
}

/// Image compressée sans l'enregistrer, pour l'aperçu (poids et rendu).
#[tauri::command]
pub async fn compress_preview(path: String, options: compress::Options) -> Result<Response, ConvertError> {
    tauri::async_runtime::spawn_blocking(move || compress::compress_image(Path::new(&path), &options))
        .await
        .map_err(|e| ConvertError::Io(std::io::Error::other(e)))?
        .map(Response::new)
}

/// Poids des fichiers en octets (0 si illisible).
#[tauri::command]
pub fn file_sizes(paths: Vec<String>) -> Vec<u64> {
    paths.iter().map(|p| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)).collect()
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> settings::Settings {
    settings::get(&app)
}

/// Enregistre et applique les réglages ; renvoie la version retenue (valeurs bornées).
#[tauri::command]
pub fn set_settings(app: AppHandle, settings: settings::Settings) -> Result<settings::Settings, String> {
    settings::set(&app, settings)
}

/// Système d'exploitation (« windows », « linux »…), pour adapter l'écran d'accueil.
#[tauri::command]
pub fn platform() -> &'static str {
    std::env::consts::OS
}

/// Linux : Morph est-il intégré au bureau (menu, « Ouvrir avec », clic droit) ?
#[tauri::command]
pub fn system_integrated() -> bool {
    cfg!(target_os = "linux") && crate::linux::share_dir().is_some_and(|d| crate::linux::is_integrated(&d))
}

/// Linux : ajoute (ou retire) Morph du menu des applications et du clic droit.
#[tauri::command]
pub fn set_system_integration(enable: bool) -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("intégration réservée à Linux".into());
    }
    let share = crate::linux::share_dir().ok_or("dossier personnel introuvable")?;
    let result = if enable {
        crate::linux::executable().and_then(|exe| crate::linux::integrate(&exe, &share))
    } else {
        crate::linux::remove(&share)
    };
    result.map_err(|e| e.to_string())
}

/// Quitte Morph (menu de l'icône, bouton des Réglages).
#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}

/// Erreurs non gérées de l'interface, recopiées dans la console de l'application.
#[tauri::command]
pub fn log_frontend_error(window: tauri::WebviewWindow, message: String) {
    eprintln!("[ui:{}] {message}", window.label());
}

#[cfg(test)]
mod tests {
    #[test]
    fn decodes_percent_encoded_paths() {
        assert_eq!(super::percent_decode("C%3A%5CPhotos%5Cd%C3%A9j%C3%A0%20vu.png"), r"C:\Photos\déjà vu.png");
        assert_eq!(super::percent_decode("50%"), "50%");
    }
}
