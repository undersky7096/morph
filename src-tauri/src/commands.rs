use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::convert::{self, formats};
use crate::radial;

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
