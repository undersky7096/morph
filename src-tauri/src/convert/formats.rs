use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Image,
    Audio,
    Video,
}

/// Extensions reconnues en entrée, par type.
const IMAGE_IN: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "gif", "tif", "tiff", "ico", "tga"];
const AUDIO_IN: &[&str] = &["mp3", "wav", "ogg", "oga", "flac", "m4a", "aac", "opus", "wma"];
const VIDEO_IN: &[&str] = &[
    "mp4", "m4v", "webm", "mkv", "mov", "avi", "wmv", "flv", "mpeg", "mpg", "3gp", "ts",
];

/// Formats de sortie, par type de cible.
pub const IMAGE_OUT: &[&str] = &["png", "jpg", "webp", "bmp", "gif", "tiff", "ico", "tga"];
pub const AUDIO_OUT: &[&str] = &["mp3", "wav", "ogg", "flac", "m4a", "opus"];
pub const VIDEO_OUT: &[&str] = &["mp4", "webm", "mkv", "mov", "avi", "gif"];

/// Extension normalisée d'un chemin (minuscules, alias résolus : jpeg → jpg, tif → tiff…).
pub fn ext_of(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(normalize(&ext).to_string())
}

pub fn normalize(ext: &str) -> &str {
    match ext {
        "jpeg" => "jpg",
        "tif" => "tiff",
        "oga" => "ogg",
        "m4v" => "mp4",
        "mpg" => "mpeg",
        other => other,
    }
}

pub fn kind_of(path: &Path) -> Option<Kind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if IMAGE_IN.contains(&ext.as_str()) {
        Some(Kind::Image)
    } else if AUDIO_IN.contains(&ext.as_str()) {
        Some(Kind::Audio)
    } else if VIDEO_IN.contains(&ext.as_str()) {
        Some(Kind::Video)
    } else {
        None
    }
}

/// Formats cibles proposés pour un fichier, hors son propre format.
pub fn targets_for(path: &Path) -> Vec<&'static str> {
    let Some(kind) = kind_of(path) else {
        return Vec::new();
    };
    let src = ext_of(path).unwrap_or_default();
    let mut targets: Vec<&'static str> = match kind {
        Kind::Image => IMAGE_OUT.to_vec(),
        Kind::Audio => AUDIO_OUT.to_vec(),
        Kind::Video => VIDEO_OUT.iter().chain(AUDIO_OUT).copied().collect(),
    };
    // Un GIF peut être animé : on autorise sa conversion en vidéo.
    if src == "gif" {
        targets.extend(["mp4", "webm"]);
    }
    targets.retain(|t| *t != src);
    targets
}

#[derive(Debug, Serialize)]
pub struct TargetGroup {
    pub kind: Kind,
    pub targets: Vec<&'static str>,
}

/// Menu de formats pour un lot de fichiers.
/// `common` : formats applicables à tous les fichiers du lot.
/// `groups` : formats par type détecté (utile pour les lots mixtes → sous-menus).
#[derive(Debug, Serialize)]
pub struct TargetMenu {
    pub common: Vec<&'static str>,
    pub groups: Vec<TargetGroup>,
    pub unsupported: Vec<String>,
}

pub fn menu_for(paths: &[&Path]) -> TargetMenu {
    let mut common: Option<Vec<&'static str>> = None;
    let mut groups: Vec<TargetGroup> = Vec::new();
    let mut unsupported = Vec::new();

    for path in paths {
        let Some(kind) = kind_of(path) else {
            unsupported.push(path.display().to_string());
            continue;
        };
        let targets = targets_for(path);

        common = Some(match common {
            None => targets.clone(),
            Some(c) => c.into_iter().filter(|t| targets.contains(t)).collect(),
        });

        match groups.iter_mut().find(|g| g.kind == kind) {
            Some(group) => {
                for t in targets {
                    if !group.targets.contains(&t) {
                        group.targets.push(t);
                    }
                }
            }
            None => groups.push(TargetGroup { kind, targets }),
        }
    }

    TargetMenu { common: common.unwrap_or_default(), groups, unsupported }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_kinds_and_aliases() {
        assert_eq!(kind_of(Path::new("a/Photo.JPEG")), Some(Kind::Image));
        assert_eq!(kind_of(Path::new("clip.mov")), Some(Kind::Video));
        assert_eq!(kind_of(Path::new("notes.txt")), None);
        assert_eq!(ext_of(Path::new("x.TIF")).as_deref(), Some("tiff"));
    }

    #[test]
    fn excludes_source_format() {
        let t = targets_for(Path::new("a.png"));
        assert!(!t.contains(&"png"));
        assert!(t.contains(&"ico"));
        assert!(targets_for(Path::new("v.mp4")).contains(&"wav"));
        assert!(targets_for(Path::new("anim.gif")).contains(&"mp4"));
    }

    #[test]
    fn mixed_batch_has_groups() {
        let menu = menu_for(&[Path::new("a.png"), Path::new("b.mp3"), Path::new("c.txt")]);
        assert!(menu.common.is_empty());
        assert_eq!(menu.groups.len(), 2);
        assert_eq!(menu.unsupported, vec!["c.txt".to_string()]);

        let menu = menu_for(&[Path::new("a.mp4"), Path::new("b.wav")]);
        assert!(menu.common.contains(&"mp3"));
    }
}
