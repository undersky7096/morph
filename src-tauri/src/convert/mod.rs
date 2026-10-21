//! Moteur de conversion : images via la crate `image` (SVG via `resvg`, AVIF et HEIC via
//! ffmpeg), audio/vidéo via ffmpeg (sidecar), documents via pandoc (sidecar) et LibreOffice
//! (s'il est installé), PDF via pdfium et lopdf. Formats et routes : [`catalog`], [`formats`].

pub mod archive;
pub mod catalog;
pub mod engines;
pub mod ffmpeg;
pub mod formats;
pub mod office;
pub mod output;
pub mod pandoc;
pub mod pdf;
pub(crate) mod raster;
mod svg;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

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
    #[error("document : {0}")]
    Document(String),
    #[error("archive : {0}")]
    Archive(String),
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
            ConvertError::Document(log) => document_message(log),
            ConvertError::Archive(log) => archive_message(log),
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

/// Cause probable d'un échec d'archive.
fn archive_message(log: &str) -> (&'static str, String) {
    let l = log.to_lowercase();
    if ["password", "encrypted", "mot de passe", "chiffr"].iter().any(|w| l.contains(w)) {
        ("protected", "Archive protégée par un mot de passe : non prise en charge.".into())
    } else if l.contains("archive vide") {
        ("unsupported", "Archive vide : rien à extraire.".into())
    } else {
        ("unreadable", "Archive illisible ou endommagée.".into())
    }
}

/// Cause probable d'un échec de conversion de document (pandoc, LibreOffice, PDF).
fn document_message(log: &str) -> (&'static str, String) {
    if log.contains("mot de passe") {
        ("protected", "PDF protégé par un mot de passe : retire la protection d'abord.".into())
    } else if log.contains("délai dépassé") {
        ("failed", "LibreOffice ne répond pas : ferme ses fenêtres ouvertes, puis réessaie.".into())
    } else if log.contains("impossible de lancer") {
        ("engine-missing", "Le moteur de conversion des documents est introuvable : réinstalle Morph.".into())
    } else {
        ("unreadable", "Conversion du document impossible : fichier illisible ou endommagé ?".into())
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
    if let Some(missing) = formats::blocked_reason(src, &target) {
        return Err(ConvertError::EngineMissing(missing.reason));
    }
    let to = catalog::find(&target).expect("cible de la table");
    on_progress(0.0);

    // Pages d'un PDF : un fichier, ou un dossier s'il y a plusieurs pages.
    if src.ext == "pdf" {
        return pdf::render_pages(input, &target, on_progress);
    }
    let output = if src.kind == Kind::Archive {
        // `photos.tar.gz` → `photos.zip`, pas `photos.tar.zip`.
        output::unique_in_dir(input, &archive::base_name(input), &target)?
    } else {
        unique_output(input, &target)?
    };
    match formats::route(src, to).as_slice() {
        _ if src.kind == Kind::Image && to.ext == "pdf" => {
            pdf::images_to_pdf(&[input.to_path_buf()], &output, on_progress)?;
        }
        _ if src.kind == Kind::Image && to.kind == Kind::Image => {
            raster::convert(input, &output, &target)?;
            on_progress(1.0);
        }
        [catalog::Engine::Office] => office::convert(input, &output, &target, on_progress)?,
        // Texte → PDF : pandoc vers un ODT provisoire, que LibreOffice exporte.
        [catalog::Engine::Pandoc, catalog::Engine::Office] => {
            let odt = std::env::temp_dir().join(format!("morph-{}-{}.odt", std::process::id(), output::unique_suffix()));
            let mut first = |p: f32| on_progress(p * 0.3);
            let result = pandoc::convert(input, src.ext, &odt, "odt", &mut first)
                .and_then(|()| office::convert(&odt, &output, &target, &mut |p| on_progress(0.3 + p * 0.7)));
            let _ = std::fs::remove_file(&odt);
            result?;
        }
        [catalog::Engine::Pandoc] => pandoc::convert(input, src.ext, &output, &target, on_progress)?,
        _ if src.kind == Kind::Archive => archive::repack(input, &output, &target, on_progress)?,
        _ => ffmpeg::convert(input, &output, &target, on_progress)?,
    }
    Ok(output)
}

/// Le lot `paths` vers `target` produit un seul fichier : fusion de PDF, images → PDF,
/// archive de la sélection.
pub fn is_batch(target: &str, paths: &[String]) -> bool {
    target == archive::ARCHIVE_ZIP || target == archive::ARCHIVE_7Z || pdf::is_batch(target, paths)
}

/// Opération sur tout un lot (voir [`is_batch`]) ; renvoie le fichier produit.
pub fn run_batch(inputs: &[PathBuf], target: &str, on_progress: &mut dyn FnMut(f32)) -> Result<PathBuf, ConvertError> {
    match target {
        archive::ARCHIVE_ZIP => archive::create(inputs, "zip", on_progress),
        archive::ARCHIVE_7Z => archive::create(inputs, "7z", on_progress),
        _ => pdf::batch(inputs, target, on_progress),
    }
}

/// Lance un programme externe sans ouvrir de fenêtre de console (Windows).
pub(crate) fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Exécute `work` dans un fil à part. Pendant ce temps, la progression avance de `from`
/// vers `to` au rythme de la durée `expected` (90 % de l'étape au temps prévu, puis de plus
/// en plus lentement, sans jamais l'atteindre) : une étape longue et muette (chargement d'un
/// modèle, LibreOffice…) ne fige pas l'affichage.
pub(crate) fn ticking<T: Send>(
    expected: Duration,
    from: f32,
    to: f32,
    on_progress: &mut dyn FnMut(f32),
    work: impl FnOnce() -> T + Send,
) -> (T, Duration) {
    let started = Instant::now();
    std::thread::scope(|scope| {
        let handle = scope.spawn(work);
        while !handle.is_finished() {
            let x = started.elapsed().as_secs_f32() / expected.as_secs_f32().max(0.1);
            let done = if x < 1.0 { 0.9 * x } else { 1.0 - 0.1 * (1.0 - x).exp() };
            on_progress(from + (to - from) * done);
            std::thread::sleep(Duration::from_millis(100));
        }
        let result = handle.join().unwrap_or_else(|e| std::panic::resume_unwind(e));
        (result, started.elapsed())
    })
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
