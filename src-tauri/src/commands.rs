use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter};

use crate::convert::{self, ConvertError, formats};
use crate::edit::{self, compress, trim};
use crate::{radial, tools};

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
    radial::show_actions(&app, paths).map_err(|e| e.to_string())
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

#[derive(Serialize)]
pub struct ConvertResult {
    input: String,
    output: Option<String>,
    error: Option<String>,
}

/// Convertit un lot vers un même format. Progression émise via l'événement `convert-progress`.
#[tauri::command]
pub async fn convert_files(app: AppHandle, paths: Vec<String>, target: String) -> Vec<ConvertResult> {
    tauri::async_runtime::spawn_blocking(move || {
        paths
            .into_iter()
            .enumerate()
            .map(|(index, input)| {
                let mut report = |progress: f32| {
                    let _ = app.emit("convert-progress", Progress { index, input: input.clone(), progress });
                };
                let result = convert::convert(&PathBuf::from(&input), &target, &mut report);
                match result {
                    Ok(out) => ConvertResult { input, output: Some(out.display().to_string()), error: None },
                    Err(e) => ConvertResult { input, output: None, error: Some(e.to_string()) },
                }
            })
            .collect()
    })
    .await
    .unwrap_or_default()
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
pub async fn crop_image(path: String, rect: edit::Rect) -> Result<String, ConvertError> {
    tauri::async_runtime::spawn_blocking(move || edit::crop(Path::new(&path), rect))
        .await
        .map_err(|e| ConvertError::Io(std::io::Error::other(e)))?
        .map(|out| out.display().to_string())
}

/// Enregistre le rendu de l'éditeur image (PNG brut dans le corps de la requête,
/// chemin d'origine dans l'en-tête `x-path`) au format d'origine, suffixe « Edited ».
#[tauri::command]
pub async fn save_edited(request: tauri::ipc::Request<'_>) -> Result<String, ConvertError> {
    let tauri::ipc::InvokeBody::Raw(png) = request.body() else {
        return Err(ConvertError::Unsupported("corps de requête attendu : PNG brut".into()));
    };
    let path = request
        .headers()
        .get("x-path")
        .and_then(|v| v.to_str().ok())
        .map(|v| percent_decode(v))
        .ok_or_else(|| ConvertError::Unsupported("en-tête x-path manquant".into()))?;
    let png = png.clone();
    tauri::async_runtime::spawn_blocking(move || edit::save_edited(Path::new(&path), &png))
        .await
        .map_err(|e| ConvertError::Io(std::io::Error::other(e)))?
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

/// Garde les segments donnés d'un fichier audio. Progression via l'événement `trim-progress`.
#[tauri::command]
pub async fn trim_audio(app: AppHandle, path: String, segments: Vec<trim::Segment>) -> Result<String, ConvertError> {
    blocking(move || {
        trim::trim(Path::new(&path), &segments, &mut |p| {
            let _ = app.emit("trim-progress", p);
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
    error: Option<String>,
}

/// Compresse un lot. Progression émise via l'événement `compress-progress`.
#[tauri::command]
pub async fn compress_files(app: AppHandle, paths: Vec<String>, options: compress::Options) -> Vec<CompressResult> {
    tauri::async_runtime::spawn_blocking(move || {
        paths
            .into_iter()
            .enumerate()
            .map(|(index, input)| {
                let mut report = |progress: f32| {
                    let _ = app.emit("compress-progress", Progress { index, input: input.clone(), progress });
                };
                match compress::compress(Path::new(&input), &options, &mut report) {
                    Ok(outcome) => CompressResult { input, outcome: Some(outcome), error: None },
                    Err(e) => CompressResult { input, outcome: None, error: Some(e.to_string()) },
                }
            })
            .collect()
    })
    .await
    .unwrap_or_default()
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
