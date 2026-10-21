//! Questions posées à la table des formats ([`super::catalog`]) : type d'un fichier,
//! formats proposés, menu d'un lot.

use std::path::Path;

use serde::Serialize;

pub use super::catalog::Kind;
use super::catalog::{self, Engine, Family, Format};
use super::engines::{self, Missing};

/// Format reconnu (et lisible) d'un fichier, d'après son extension.
pub fn format_of(path: &Path) -> Option<&'static Format> {
    catalog::find(path.extension()?.to_str()?).filter(|f| f.read.is_some())
}

/// Extension normalisée d'un chemin (minuscules, alias résolus : jpeg → jpg, tif → tiff…).
pub fn ext_of(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(normalize(&ext).to_string())
}

pub fn normalize(ext: &str) -> &str {
    catalog::find(ext).map_or(ext, |f| f.ext)
}

pub fn kind_of(path: &Path) -> Option<Kind> {
    format_of(path).map(|f| f.kind)
}

/// Rang où s'insèrent les sorties particulières (vidéo → GIF, GIF → vidéo) parmi les
/// formats du même type : juste après les plus courants, pour rester au premier niveau.
const EXTRA_RANK: usize = 4;

/// Formats cibles proposés pour un fichier, hors son propre format : ceux de son type (ses
/// sorties particulières après les [`EXTRA_RANK`] premiers), puis les autres types atteignables.
pub fn targets_for(path: &Path) -> Vec<&'static str> {
    format_of(path).map(targets_from).unwrap_or_default()
}

fn targets_from(src: &Format) -> Vec<&'static str> {
    // Un document reste dans sa famille (texte, tableur, présentation), plus le PDF.
    let related = |f: &Format| src.family == Family::Media || f.family == src.family || f.ext == "pdf";
    let writable = |f: &&Format| f.write.is_some() && f.ext != src.ext && related(f);
    let mut targets: Vec<&'static str> = Vec::new();
    for (i, kind) in src.kind.reaches().iter().enumerate() {
        let same = catalog::FORMATS.iter().filter(|f| f.kind == *kind).filter(writable).map(|f| f.ext);
        let start = targets.len();
        targets.extend(same);
        if i == 0 {
            let at = (start + EXTRA_RANK).min(targets.len());
            let extra = src.extra.iter().filter_map(|e| catalog::find(e)).filter(|f| f.write.is_some()).map(|f| f.ext);
            targets.splice(at..at, extra);
        }
    }
    targets
}

/// Moteurs qui font passer de `src` à `to`, dans l'ordre.
pub fn route(src: &Format, to: &Format) -> Vec<Engine> {
    let read = src.read.into_iter();
    if src.ext == "pdf" {
        // Pages → images.
        return vec![Engine::Pdf];
    }
    if to.ext == "pdf" {
        return match src.kind {
            // Images assemblées par Morph lui-même (lopdf).
            Kind::Image => read.collect(),
            // Un texte que LibreOffice ne lit pas passe d'abord par pandoc (en ODT).
            _ if src.office => vec![Engine::Office],
            _ => vec![Engine::Pandoc, Engine::Office],
        };
    }
    if src.kind == Kind::Document {
        // Entre deux formats bureautiques, LibreOffice garde mieux la mise en page ;
        // pandoc prend le relais s'il manque et sait faire.
        let by_pandoc = src.read == Some(Engine::Pandoc) && to.write == Some(Engine::Pandoc);
        let office_ok = engines::missing(Engine::Office).is_none();
        return if src.office && to.office && (office_ok || !by_pandoc) { vec![Engine::Office] } else { vec![Engine::Pandoc] };
    }
    read.chain(to.write).collect()
}

/// Raison pour laquelle la conversion de `src` vers `target` est impossible sur cette
/// machine (moteur absent), ou `None`.
pub fn blocked_reason(src: &Format, target: &str) -> Option<Missing> {
    let to = catalog::find(target)?;
    route(src, to).into_iter().find_map(engines::missing)
}

/// Format cible indisponible, affiché grisé avec la raison (et la page pour obtenir le
/// moteur qui manque, ouverte au clic).
#[derive(Debug, Clone, Serialize)]
pub struct Blocked {
    pub format: &'static str,
    pub reason: &'static str,
    pub link: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TargetGroup {
    pub kind: Kind,
    pub targets: Vec<&'static str>,
    pub blocked: Vec<Blocked>,
    pub paths: Vec<String>,
}

/// Menu de formats pour un lot de fichiers.
/// `common` : formats applicables à tous les fichiers du lot.
/// `groups` : formats par type détecté (utile pour les lots mixtes → sous-menus).
#[derive(Debug, Clone, Serialize)]
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
        let Some(src) = format_of(path) else {
            unsupported.push(path.display().to_string());
            continue;
        };
        let (targets, blocked): (Vec<_>, Vec<_>) =
            targets_from(src).into_iter().partition(|t| blocked_reason(src, t).is_none());
        let blocked: Vec<Blocked> = blocked
            .into_iter()
            .filter_map(|format| {
                let missing = blocked_reason(src, format)?;
                Some(Blocked { format, reason: missing.reason, link: missing.link })
            })
            .collect();

        common = Some(match common {
            None => targets.clone(),
            Some(c) => c.into_iter().filter(|t| targets.contains(t)).collect(),
        });

        let path_str = path.display().to_string();
        match groups.iter_mut().find(|g| g.kind == src.kind) {
            Some(group) => {
                group.paths.push(path_str);
                for t in targets {
                    if !group.targets.contains(&t) {
                        group.targets.push(t);
                    }
                }
                for b in blocked {
                    if !group.blocked.iter().any(|g| g.format == b.format) {
                        group.blocked.push(b);
                    }
                }
            }
            None => groups.push(TargetGroup { kind: src.kind, targets, blocked, paths: vec![path_str] }),
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
        assert_eq!(kind_of(Path::new("IMG_0001.HEIC")), Some(Kind::Image));
        assert_eq!(kind_of(Path::new("son.aif")), Some(Kind::Audio));
        assert_eq!(kind_of(Path::new("archive.xyz")), None);
        assert_eq!(ext_of(Path::new("x.TIF")).as_deref(), Some("tiff"));
    }

    #[test]
    fn excludes_source_format() {
        let t = targets_for(Path::new("a.png"));
        assert!(!t.contains(&"png"));
        assert!(t.contains(&"ico") && t.contains(&"avif"));
        assert!(targets_for(Path::new("v.mp4")).contains(&"wav"));
        assert!(targets_for(Path::new("anim.gif")).contains(&"mp4"));
    }

    #[test]
    fn read_only_formats_are_never_targets() {
        let t = targets_for(Path::new("photo.heic"));
        assert_eq!(t.first(), Some(&"png"));
        assert!(!t.contains(&"heic") && !t.contains(&"svg"));
        assert!(!targets_for(Path::new("v.mts")).contains(&"ts"));
    }

    #[test]
    fn video_targets_order() {
        let t = targets_for(Path::new("v.mkv"));
        let pos = |f| t.iter().position(|x| *x == f).unwrap();
        assert!(pos("mp4") < pos("gif") && pos("gif") < pos("wmv") && pos("3gp") < pos("mp3"));
    }

    #[test]
    fn documents_stay_in_their_family() {
        let t = targets_for(Path::new("rapport.docx"));
        assert!(t.contains(&"md") && t.contains(&"odt") && t.contains(&"pdf"));
        assert!(!t.contains(&"xlsx") && !t.contains(&"doc"));
        let t = targets_for(Path::new("compta.xls"));
        assert!(t.contains(&"xlsx") && t.contains(&"csv") && t.contains(&"pdf") && !t.contains(&"docx"));
        assert_eq!(targets_for(Path::new("scan.pdf")), vec!["png", "jpg"]);
        assert!(targets_for(Path::new("photo.jpg")).contains(&"pdf"));
    }

    #[test]
    fn routes_pick_the_right_engines() {
        let f = |e| catalog::find(e).unwrap();
        assert_eq!(route(f("md"), f("pdf")), vec![Engine::Pandoc, Engine::Office]);
        assert_eq!(route(f("docx"), f("pdf")), vec![Engine::Office]);
        assert_eq!(route(f("md"), f("docx")), vec![Engine::Pandoc]);
        assert_eq!(route(f("pdf"), f("png")), vec![Engine::Pdf]);
        assert_eq!(route(f("heic"), f("pdf")), vec![Engine::Ffmpeg]);
        assert_eq!(route(f("doc"), f("docx")), vec![Engine::Office]);
    }

    #[test]
    fn mixed_batch_has_groups() {
        let menu = menu_for(&[Path::new("a.png"), Path::new("b.mp3"), Path::new("c.xyz")]);
        assert!(menu.common.is_empty(), "{:?}", menu.common);
        assert!(!targets_for(Path::new("son.mp3")).contains(&"pdf"));
        assert_eq!(menu.groups.len(), 2);
        assert_eq!(menu.groups[1].paths, vec!["b.mp3".to_string()]);
        assert_eq!(menu.unsupported, vec!["c.xyz".to_string()]);
    }
}
