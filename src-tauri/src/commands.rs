use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter};

use crate::convert::{self, ConvertError, formats};
use crate::edit::{self, compress};
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
