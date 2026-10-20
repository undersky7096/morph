//! Moteur de conversion : images via la crate `image`, audio/vidéo via ffmpeg (sidecar).

pub mod ffmpeg;
pub mod formats;
pub(crate) mod raster;

use std::path::{Path, PathBuf};

use serde::Serialize;

use formats::{IMAGE_OUT, Kind};

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
}

// Les erreurs remontent au webview sous forme de texte.
impl Serialize for ConvertError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
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
    let kind = formats::kind_of(input)
        .ok_or_else(|| ConvertError::Unsupported(input.display().to_string()))?;
    if !formats::targets_for(input).contains(&target.as_str()) {
        return Err(ConvertError::NoRoute {
            from: formats::ext_of(input).unwrap_or_default(),
            to: target,
        });
    }

    let output = unique_output(input, &target);
    on_progress(0.0);
    if kind == Kind::Image && IMAGE_OUT.contains(&target.as_str()) {
        raster::convert(input, &output, &target)?;
        on_progress(1.0);
    } else {
        ffmpeg::convert(input, &output, &target, on_progress)?;
    }
    Ok(output)
}

/// `dossier/nom.ext`, ou `dossier/nom (2).ext`, `(3)`… si le fichier existe déjà.
pub fn unique_output(input: &Path, ext: &str) -> PathBuf {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("fichier");
    unique_in_dir(input, stem, ext)
}

/// Résultat d'un outil d'édition : `dossier/nom Suffixe.ext` (ex : `Page 001 Cropped.jpg`).
pub fn suffixed_output(input: &Path, suffix: &str) -> PathBuf {
    let ext = input.extension().and_then(|s| s.to_str()).unwrap_or("png");
    suffixed_output_as(input, suffix, ext)
}

/// Comme [`suffixed_output`], avec une autre extension (ex : `.flv` compressé en `.mp4`).
pub fn suffixed_output_as(input: &Path, suffix: &str, ext: &str) -> PathBuf {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("fichier");
    unique_in_dir(input, &format!("{stem} {suffix}"), ext)
}

thread_local! {
    /// Dossier de sortie de l'opération en cours sur ce thread (réglage « dossier fixe »
    /// ou « demander ») ; aucun : à côté de l'original.
    static OUTPUT_DIR: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Exécute `f` en écrivant ses résultats dans `dir` (créé au besoin) plutôt qu'à côté
/// des originaux. Les traitements tournent sur un seul thread : la valeur suit l'opération.
pub fn with_output_dir<T>(dir: Option<PathBuf>, f: impl FnOnce() -> T) -> T {
    if let Some(d) = &dir {
        let _ = std::fs::create_dir_all(d);
    }
    let previous = OUTPUT_DIR.with(|cell| cell.replace(dir));
    let result = f();
    OUTPUT_DIR.with(|cell| cell.replace(previous));
    result
}

fn unique_in_dir(input: &Path, stem: &str, ext: &str) -> PathBuf {
    let dir = OUTPUT_DIR
        .with(|cell| cell.borrow().clone())
        .unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")).to_path_buf());
    let mut candidate = dir.join(format!("{stem}.{ext}"));
    let mut n = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{stem} ({n}).{ext}"));
        n += 1;
    }
    candidate
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
        assert_eq!(unique_output(&input, "jpg"), dir.join("photo.jpg"));
        std::fs::write(dir.join("photo.jpg"), b"").unwrap();
        assert_eq!(unique_output(&input, "jpg"), dir.join("photo (2).jpg"));
        assert_eq!(suffixed_output(&input, "Cropped"), dir.join("photo Cropped.png"));
        let elsewhere = dir.join("sorties");
        let out = with_output_dir(Some(elsewhere.clone()), || unique_output(&input, "jpg"));
        assert_eq!(out, elsewhere.join("photo.jpg"));
        assert!(elsewhere.is_dir());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
