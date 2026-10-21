//! Documents texte (MD, HTML, DOCX, ODT, RTF, EPUB, TXT) convertis par pandoc (sidecar).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::{ConvertError, no_window, ticking};

/// Chemin de pandoc : sidecar installé à côté de l'exécutable, sinon celui du PATH.
pub fn pandoc_path() -> PathBuf {
    let name = if cfg!(windows) { "pandoc.exe" } else { "pandoc" };
    std::env::current_exe()
        .ok()
        .map(|exe| exe.with_file_name(name))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("pandoc"))
}

pub(crate) fn command() -> Command {
    let mut cmd = Command::new(pandoc_path());
    no_window(&mut cmd);
    cmd
}

/// Lecteur pandoc d'une extension (un texte brut est lu comme du Markdown).
fn reader(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "md" | "txt" => "markdown",
        "html" => "html",
        "docx" => "docx",
        "odt" => "odt",
        "rtf" => "rtf",
        "epub" => "epub",
        _ => return None,
    })
}

/// Écrivain pandoc et options propres au format de sortie.
fn writer(ext: &str, stem: &str) -> Option<(&'static str, Vec<String>)> {
    let standalone = vec!["--standalone".to_string()];
    Some(match ext {
        // Markdown à la GitHub, le plus lu ; les images d'un DOCX/ODT/EPUB sont extraites
        // dans un dossier voisin (créé seulement s'il y en a).
        "md" => ("gfm", vec![format!("--extract-media={stem} (images)")]),
        // Page autonome : images et styles intégrés.
        "html" => ("html5", vec!["--standalone".into(), "--embed-resources".into()]),
        "txt" => ("plain", vec![]),
        "docx" => ("docx", vec![]),
        "odt" => ("odt", vec![]),
        "rtf" => ("rtf", standalone),
        "epub" => ("epub3", vec![]),
        _ => return None,
    })
}

/// Convertit `input` (format `from`) en `output` (format `to`). Le dossier de travail est
/// celui du document, pour retrouver les images qu'il cite par un chemin relatif.
pub fn convert(input: &Path, from: &str, output: &Path, to: &str, on_progress: &mut dyn FnMut(f32)) -> Result<(), ConvertError> {
    let unsupported = || ConvertError::NoRoute { from: from.into(), to: to.into() };
    let reader = reader(from).ok_or_else(unsupported)?;
    let stem = output.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
    let (writer, options) = writer(to, stem).ok_or_else(unsupported)?;

    let mut cmd = command();
    if let Some(dir) = input.parent().filter(|d| !d.as_os_str().is_empty()) {
        cmd.current_dir(dir);
    }
    // Le titre évite l'avertissement (et le titre vide) des formats qui en exigent un.
    cmd.args(["-f", reader, "-t", writer, "--metadata", &format!("pagetitle={stem}")])
        .args(&options)
        .arg("-o")
        .arg(output)
        .arg(input);
    let (result, _) = ticking(Duration::from_secs(2), 0.0, 0.95, on_progress, || cmd.output());
    let out = result.map_err(|e| ConvertError::Document(format!("impossible de lancer pandoc ({}) : {e}", pandoc_path().display())))?;
    if out.status.success() && output.exists() {
        on_progress(1.0);
        Ok(())
    } else {
        let _ = std::fs::remove_file(output);
        Err(ConvertError::Document(String::from_utf8_lossy(&out.stderr).trim().to_string()))
    }
}
