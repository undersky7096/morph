//! Moteur de conversion : images via la crate `image` (SVG via `resvg`, AVIF et HEIC via
//! ffmpeg), audio/vidéo via ffmpeg (sidecar). Formats et routes : [`catalog`].

pub mod catalog;
pub mod engines;
pub mod ffmpeg;
pub mod formats;
pub mod output;
pub(crate) mod raster;
mod svg;

use std::path::{Path, PathBuf};

use serde::Serialize;

use formats::Kind;

#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("fichier introuvable : {0}")]
    NotFound(String),
    #[error("format non supporté : {0}")]
    Unsupported(String),
    #[error("conversion impossible de {from} vers {to}")]
    NoRoute { from: String, to: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("erreur image : {0}")]
    Image(#[from] image::ImageError),
    #[error("ffmpeg a échoué :\n{0}")]
    Ffmpeg(String),
    #[error("compression impossible : {0}")]
    Compress(String),
    #[error("édition impossible : {0}")]
    Edit(String),
    #[error("téléchargement impossible : {0}")]
    Download(String),
    #[error("{0}")]
    EngineMissing(&'static str),
    #[error("annulé")]
    Cancelled,
    #[error("le fichier existe déjà : {}", .0.display())]
    Exists(PathBuf),
}

/// Erreur telle que présentée à l'utilisateur : un code (pour l'interface), un message
/// clair, et le détail technique (à copier pour un rapport de bug).
#[derive(Debug, Clone, Serialize)]
pub struct ErrorInfo {
    pub code: &'static str,
    pub message: String,
    pub detail: String,
    /// Fichier concerné (doublon).
    pub path: Option<String>,
}

const DENIED: &str = "Accès refusé : le fichier est peut-être ouvert ailleurs, ou le dossier est protégé.";
const DISK_FULL: &str = "Disque plein : libère de la place puis réessaie.";

impl ConvertError {
    pub fn info(&self) -> ErrorInfo {
        let detail = self.to_string();
        let (code, message) = match self {
            ConvertError::NotFound(_) => ("not-found", "Fichier introuvable : il a peut-être été déplacé ou supprimé.".into()),
            ConvertError::Unsupported(_) => ("unsupported", "Format non pris en charge.".into()),
            ConvertError::NoRoute { from, to } => (
                "unsupported",
                format!("Impossible de convertir du {} en {}.", from.to_uppercase(), to.to_uppercase()),
            ),
            ConvertError::Io(e) => io_message(e),
            ConvertError::Image(_) => ("unreadable", "Image illisible ou endommagée.".into()),
            ConvertError::Ffmpeg(log) => ffmpeg_message(log),
            ConvertError::Compress(m) | ConvertError::Edit(m) => ("failed", sentence(m)),
            ConvertError::Download(_) => (
                "download",
                "Téléchargement du modèle de détourage impossible : vérifie la connexion à internet, puis réessaie.".into(),
            ),
            ConvertError::EngineMissing(reason) => ("engine-missing", (*reason).into()),
            ConvertError::Cancelled => ("cancelled", "Annulé.".into()),
            ConvertError::Exists(p) => (
                "exists",
                format!("« {} » existe déjà.", p.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()),
            ),
        };
        let path = match self {
            ConvertError::Exists(p) => Some(p.display().to_string()),
            _ => None,
        };
        ErrorInfo { code, message, detail, path }
    }
}

/// « aucun passage à garder » → « Aucun passage à garder. »
fn sentence(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() + "."
}

fn io_message(e: &std::io::Error) -> (&'static str, String) {
    use std::io::ErrorKind;
    match e.kind() {
        ErrorKind::PermissionDenied => ("denied", DENIED.into()),
        ErrorKind::StorageFull => ("disk-full", DISK_FULL.into()),
        ErrorKind::NotFound => ("not-found", "Fichier ou dossier introuvable.".into()),
        _ => ("io", "Erreur de lecture ou d'écriture du fichier.".into()),
    }
}

/// Cause probable d'un échec de ffmpeg, d'après la fin de son journal.
fn ffmpeg_message(log: &str) -> (&'static str, String) {
    let l = log.to_lowercase();
    let any = |words: &[&str]| words.iter().any(|w| l.contains(w));
    if any(&["no space left"]) {
        ("disk-full", DISK_FULL.into())
    } else if any(&["permission denied"]) {
        ("denied", DENIED.into())
    } else if any(&["impossible de lancer ffmpeg"]) {
        ("ffmpeg-missing", "Le moteur de conversion (ffmpeg) est introuvable : réinstalle Morph.".into())
    } else if any(&["does not contain any stream", "matches no streams"]) {
        ("unsupported", "Ce fichier ne contient rien à convertir (ni image ni son).".into())
    } else if any(&["invalid data", "moov atom not found", "could not find codec", "error while decoding"]) {
        ("unreadable", "Fichier illisible ou endommagé.".into())
    } else {
        ("failed", "La conversion a échoué.".into())
    }
}

// Les erreurs remontent au webview sous forme structurée (voir `src/errors.ts`).
impl Serialize for ConvertError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.info().serialize(s)
    }
}

/// Convertit `input` vers le format `target` (extension, ex : "webp").
/// Le résultat est écrit à côté de l'original, sans jamais écraser un fichier existant.
pub fn convert(
    input: &Path,
    target: &str,
    on_progress: &mut dyn FnMut(f32),
) -> Result<PathBuf, ConvertError> {
    if !input.is_file() {
        return Err(ConvertError::NotFound(input.display().to_string()));
    }
    let target = formats::normalize(&target.to_ascii_lowercase()).to_string();
    let src = formats::format_of(input)
        .ok_or_else(|| ConvertError::Unsupported(input.display().to_string()))?;
    if !formats::targets_for(input).contains(&target.as_str()) {
        return Err(ConvertError::NoRoute {
            from: formats::ext_of(input).unwrap_or_default(),
            to: target,
        });
    }
    if let Some(reason) = formats::blocked_reason(src, &target) {
        return Err(ConvertError::EngineMissing(reason));
    }

    let output = unique_output(input, &target)?;
    on_progress(0.0);
    let to_image = catalog::find(&target).is_some_and(|f| f.kind == Kind::Image);
    if src.kind == Kind::Image && to_image {
        raster::convert(input, &output, &target)?;
        on_progress(1.0);
    } else {
        ffmpeg::convert(input, &output, &target, on_progress)?;
    }
    Ok(output)
}

/// `dossier/nom.ext` ; si le fichier existe déjà : `nom (2).ext`, remplacement ou
/// erreur, selon le contexte de l'opération (voir [`output`]).
pub fn unique_output(input: &Path, ext: &str) -> Result<PathBuf, ConvertError> {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("fichier");
    output::unique_in_dir(input, stem, ext)
}

/// Résultat d'un outil d'édition : `dossier/nom Suffixe.ext` (ex : `Page 001 Cropped.jpg`).
pub fn suffixed_output(input: &Path, suffix: &str) -> Result<PathBuf, ConvertError> {
    let ext = input.extension().and_then(|s| s.to_str()).unwrap_or("png");
    suffixed_output_as(input, suffix, ext)
}

/// Comme [`suffixed_output`], avec une autre extension (ex : `.flv` compressé en `.mp4`).
pub fn suffixed_output_as(input: &Path, suffix: &str, ext: &str) -> Result<PathBuf, ConvertError> {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("fichier");
    output::unique_in_dir(input, &format!("{stem} {suffix}"), ext)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_output_adds_counter() {
        let dir = std::env::temp_dir().join("morph-test-unique");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("photo.png");
        assert_eq!(unique_output(&input, "jpg").unwrap(), dir.join("photo.jpg"));
        std::fs::write(dir.join("photo.jpg"), b"").unwrap();
        assert_eq!(unique_output(&input, "jpg").unwrap(), dir.join("photo (2).jpg"));
        assert_eq!(suffixed_output(&input, "Cropped").unwrap(), dir.join("photo Cropped.png"));
        let elsewhere = dir.join("sorties");
        let scope = output::Scope { dir: Some(elsewhere.clone()), ..output::Scope::default() };
        let out = output::scoped(scope, || unique_output(&input, "jpg")).unwrap();
        assert_eq!(out, elsewhere.join("photo.jpg"));
        assert!(elsewhere.is_dir());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
