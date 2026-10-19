//! Moteur de conversion : images via la crate `image`, audio/vidéo via ffmpeg (sidecar).

pub mod ffmpeg;
pub mod formats;
mod raster;

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
        raster::convert(input, &output, &target).inspect_err(|_| {
            let _ = std::fs::remove_file(&output);
        })?;
        on_progress(1.0);
    } else {
        ffmpeg::convert(input, &output, &target, on_progress)?;
    }
    Ok(output)
}

/// `dossier/nom.ext`, ou `dossier/nom (2).ext`, `(3)`… si le fichier existe déjà.
pub fn unique_output(input: &Path, ext: &str) -> PathBuf {
    let dir = input.parent().unwrap_or(Path::new("."));
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("fichier");
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
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
