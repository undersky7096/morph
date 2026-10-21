//! Contexte d'une opération : où écrire les résultats, que faire d'un fichier qui existe
//! déjà, et si l'opération a été annulée.
//!
//! Les traitements tournent sur un seul thread du début à la fin : le contexte est posé
//! sur ce thread par [`scoped`] et lu par les fonctions de nommage et par ffmpeg, sans
//! avoir à le faire passer par chaque fonction de conversion.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use serde::{Deserialize, Serialize};

use super::ConvertError;

/// Fichier de sortie déjà présent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Duplicates {
    /// Garder les deux : « nom (2).ext ».
    #[default]
    Keep,
    /// Remplacer l'ancien (restauré si l'opération échoue).
    Replace,
    /// Laisser l'utilisateur trancher : l'opération échoue avec [`ConvertError::Exists`].
    Ask,
}

/// Réglages d'une opération.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    /// Dossier des résultats ; aucun : à côté de l'original.
    pub dir: Option<PathBuf>,
    pub duplicates: Duplicates,
    /// L'opération s'arrête dès que l'un de ces drapeaux est levé (fichier, lot…).
    pub cancel: Vec<Arc<AtomicBool>>,
}

#[derive(Default)]
struct Context {
    scope: Scope,
    /// Fichiers remplacés, mis de côté : (copie de sauvegarde, emplacement d'origine).
    backups: Vec<(PathBuf, PathBuf)>,
}

thread_local! {
    static CONTEXT: RefCell<Option<Context>> = const { RefCell::new(None) };
}

/// Exécute `f` dans le contexte `scope`. À la fin, les fichiers remplacés sont supprimés
/// si l'opération a réussi (et a bien écrit le nouveau fichier), restaurés sinon.
pub fn scoped<T>(scope: Scope, f: impl FnOnce() -> Result<T, ConvertError>) -> Result<T, ConvertError> {
    if let Some(dir) = &scope.dir {
        let _ = std::fs::create_dir_all(dir);
    }
    let previous = CONTEXT.with(|c| c.replace(Some(Context { scope, backups: Vec::new() })));
    let result = check_cancel().and_then(|_| f());
    let ctx = CONTEXT.with(|c| c.replace(previous)).unwrap_or_default();
    let cancelled = ctx.scope.cancel.iter().any(|f| f.load(Ordering::Relaxed));
    for (backup, target) in ctx.backups {
        if result.is_ok() && target.exists() {
            let _ = std::fs::remove_file(&backup);
        } else {
            let _ = std::fs::remove_file(&target);
            let _ = std::fs::rename(&backup, &target);
        }
    }
    match result {
        Err(_) if cancelled => Err(ConvertError::Cancelled),
        other => other,
    }
}

/// L'opération en cours a été annulée.
pub fn cancelled() -> bool {
    CONTEXT.with(|c| {
        c.borrow()
            .as_ref()
            .is_some_and(|ctx| ctx.scope.cancel.iter().any(|f| f.load(Ordering::Relaxed)))
    })
}

pub fn check_cancel() -> Result<(), ConvertError> {
    if cancelled() { Err(ConvertError::Cancelled) } else { Ok(()) }
}

/// Nombre différent à chaque appel (noms de fichiers provisoires).
pub(super) fn unique_suffix() -> u32 {
    static N: AtomicU32 = AtomicU32::new(0);
    N.fetch_add(1, Ordering::Relaxed)
}

/// Nouveau dossier `stem` pour des résultats issus de `input` (pages d'un PDF…), à côté de
/// l'original ou dans le dossier de sortie ; « (2) » s'il existe déjà (jamais remplacé).
pub(super) fn unique_dir(input: &Path, stem: &str) -> Result<PathBuf, ConvertError> {
    let dir = CONTEXT.with(|c| c.borrow().as_ref().and_then(|ctx| ctx.scope.dir.clone()));
    let parent = dir.unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")).to_path_buf());
    let mut target = parent.join(stem);
    let mut n = 2;
    while target.exists() {
        target = parent.join(format!("{stem} ({n})"));
        n += 1;
    }
    std::fs::create_dir_all(&target)?;
    Ok(target)
}

/// `dossier/stem.ext` pour un résultat issu de `input`, selon le dossier et la politique
/// de doublons du contexte (sans contexte : à côté de l'original, « (2) » si besoin).
pub(super) fn unique_in_dir(input: &Path, stem: &str, ext: &str) -> Result<PathBuf, ConvertError> {
    let (dir, duplicates) = CONTEXT.with(|c| {
        let c = c.borrow();
        let scope = c.as_ref().map(|ctx| &ctx.scope);
        (scope.and_then(|s| s.dir.clone()), scope.map_or(Duplicates::Keep, |s| s.duplicates))
    });
    let dir = dir.unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")).to_path_buf());
    let target = dir.join(format!("{stem}.{ext}"));
    if !target.exists() {
        return Ok(target);
    }
    match duplicates {
        Duplicates::Keep => {
            let mut n = 2;
            loop {
                let candidate = dir.join(format!("{stem} ({n}).{ext}"));
                if !candidate.exists() {
                    return Ok(candidate);
                }
                n += 1;
            }
        }
        Duplicates::Ask => Err(ConvertError::Exists(target)),
        Duplicates::Replace => {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let backup = dir.join(format!(".{stem}.{ext}.morph-{}-{n}", std::process::id()));
            std::fs::rename(&target, &backup)?;
            CONTEXT.with(|c| {
                if let Some(ctx) = c.borrow_mut().as_mut() {
                    ctx.backups.push((backup.clone(), target.clone()));
                }
            });
            Ok(target)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn replace_keeps_old_file_when_operation_fails() {
        let d = dir("morph-test-replace");
        let input = d.join("a.png");
        let target = d.join("a.webp");
        std::fs::write(&target, b"ancien").unwrap();
        let scope = || Scope { duplicates: Duplicates::Replace, ..Scope::default() };

        // Échec : l'ancien fichier revient.
        let r: Result<(), _> = scoped(scope(), || {
            let out = unique_in_dir(&input, "a", "webp")?;
            std::fs::write(&out, b"partiel")?;
            Err(ConvertError::Edit("raté".into()))
        });
        assert!(r.is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"ancien");

        // Réussite : le nouveau remplace l'ancien, sans copie qui traîne.
        scoped(scope(), || {
            let out = unique_in_dir(&input, "a", "webp")?;
            Ok(std::fs::write(out, b"nouveau")?)
        })
        .unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"nouveau");
        assert_eq!(std::fs::read_dir(&d).unwrap().count(), 1);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn ask_reports_existing_file_and_cancel_stops() {
        let d = dir("morph-test-ask");
        std::fs::write(d.join("a.webp"), b"x").unwrap();
        let r = scoped(Scope { duplicates: Duplicates::Ask, ..Scope::default() }, || {
            unique_in_dir(&d.join("a.png"), "a", "webp")
        });
        assert!(matches!(r, Err(ConvertError::Exists(p)) if p == d.join("a.webp")));

        let flag = Arc::new(AtomicBool::new(true));
        let r = scoped(Scope { cancel: vec![flag], ..Scope::default() }, || Ok(()));
        assert!(matches!(r, Err(ConvertError::Cancelled)));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
