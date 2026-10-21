//! Intégration au bureau Linux (l'AppImage ne s'installe pas d'elle-même) : entrée dans le
//! menu des applications avec « Ouvrir avec Morph » pour les images, sons et vidéos, et
//! actions « Convertir / Éditer / Compresser avec Morph » dans le clic droit des
//! gestionnaires de fichiers (Nautilus, Dolphin, Nemo).
//!
//! Tout est écrit dans le dossier personnel (`~/.local/share`), sans droits d'administrateur.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const ICON: &[u8] = include_bytes!("../icons/128x128@2x.png");

/// Types de fichiers ouverts par Morph (formats lisibles de la table des formats).
fn mime_types() -> String {
    let readable = crate::convert::catalog::FORMATS.iter().filter(|f| f.read.is_some());
    readable.flat_map(|f| f.mime).copied().collect::<Vec<_>>().join(";")
}

/// Actions du clic droit : (identifiant, libellé français, libellé anglais, option de la
/// ligne de commande).
const ACTION_TEXTS: [(&str, &str, &str, &str); 3] = [
    ("convert", "Convertir avec Morph", "Convert with Morph", ""),
    ("edit", "Éditer avec Morph", "Edit with Morph", "--edit"),
    ("compress", "Compresser avec Morph", "Compress with Morph", "--compress"),
];

/// Actions du clic droit dans la langue de l'interface : (identifiant, libellé, option).
fn actions() -> [(&'static str, &'static str, &'static str); 3] {
    ACTION_TEXTS.map(|(id, fr, en, arg)| (id, crate::i18n::tr(fr, en), arg))
}

/// Fichiers créés, relatifs à `~/.local/share`.
fn files() -> Vec<String> {
    let mut list = vec![
        "applications/morph.desktop".to_string(),
        "icons/hicolor/256x256/apps/morph.png".to_string(),
        "kio/servicemenus/morph.desktop".to_string(),
        "kservices5/ServiceMenus/morph.desktop".to_string(),
    ];
    // Scripts Nautilus nommés d'après le libellé : ceux des deux langues, pour retirer aussi
    // ceux créés avant un changement de langue.
    for (id, fr, en, _) in ACTION_TEXTS {
        list.push(format!("nautilus/scripts/{fr}"));
        list.push(format!("nautilus/scripts/{en}"));
        list.push(format!("nemo/actions/morph-{id}.nemo_action"));
    }
    list
}

/// Chemin entre guillemets pour une ligne `Exec=` (spécification Desktop Entry : `"`,
/// `` ` ``, `$` et `\` sont précédés d'une barre oblique inverse, puis chaque barre oblique
/// inverse est doublée au niveau de la chaîne).
fn quoted(exe: &Path) -> String {
    let mut arg = String::new();
    for c in exe.display().to_string().chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            arg.push('\\');
        }
        arg.push(c);
    }
    format!("\"{}\"", arg.replace('\\', "\\\\"))
}

/// Chemin entre apostrophes pour un script shell.
fn shell_quoted(exe: &Path) -> String {
    format!("'{}'", exe.display().to_string().replace('\'', r"'\''"))
}

fn desktop_entry(exe: &Path) -> String {
    let exe = quoted(exe);
    let mut s = format!(
        "[Desktop Entry]\nType=Application\nName=Morph\nGenericName=Convertisseur de fichiers\n\
         Comment=Convertir et retoucher images, sons et vidéos\nExec={exe} %F\nIcon=morph\nTerminal=false\n\
         Categories=Graphics;AudioVideo;Utility;\nMimeType={};\nActions=edit;compress;\n",
        mime_types()
    );
    for (id, label, arg) in &actions()[1..] {
        s += &format!("\n[Desktop Action {id}]\nName={label}\nExec={exe} {arg} %F\n");
    }
    s
}

/// Menu de service Dolphin (KDE).
fn service_menu(exe: &Path) -> String {
    let exe = quoted(exe);
    let ids: Vec<&str> = ACTION_TEXTS.iter().map(|(id, _, _, _)| *id).collect();
    let mut s = format!(
        "[Desktop Entry]\nType=Service\nMimeType={};\nActions={};\nX-KDE-Submenu=Morph\n",
        mime_types(),
        ids.join(";")
    );
    for (id, label, arg) in actions() {
        s += &format!("\n[Desktop Action {id}]\nName={label}\nIcon=morph\nExec={exe} {arg} %F\n");
    }
    s
}

/// Script Nautilus (GNOME) : les fichiers sélectionnés arrivent en arguments.
fn nautilus_script(exe: &Path, arg: &str) -> String {
    let arg = if arg.is_empty() { String::new() } else { format!("{arg} ") };
    format!("#!/bin/sh\nexec {} {arg}\"$@\"\n", shell_quoted(exe))
}

fn nemo_action(exe: &Path, label: &str, arg: &str) -> String {
    let arg = if arg.is_empty() { String::new() } else { format!("{arg} ") };
    format!(
        "[Nemo Action]\nName={label}\nIcon-Name=morph\nExec={} {arg}%F\nSelection=notnone\nExtensions=any;\n",
        quoted(exe)
    )
}

fn write(path: &Path, content: &[u8], executable: bool) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(path, content)?;
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(())
}

/// Installe l'intégration pour l'exécutable `exe` dans `share` (`~/.local/share`).
pub fn integrate(exe: &Path, share: &Path) -> io::Result<()> {
    write(&share.join("applications/morph.desktop"), desktop_entry(exe).as_bytes(), false)?;
    write(&share.join("icons/hicolor/256x256/apps/morph.png"), ICON, false)?;
    let menu = service_menu(exe);
    write(&share.join("kio/servicemenus/morph.desktop"), menu.as_bytes(), true)?;
    write(&share.join("kservices5/ServiceMenus/morph.desktop"), menu.as_bytes(), false)?;
    for (id, label, arg) in actions() {
        write(&share.join(format!("nautilus/scripts/{label}")), nautilus_script(exe, arg).as_bytes(), true)?;
        write(&share.join(format!("nemo/actions/morph-{id}.nemo_action")), nemo_action(exe, label, arg).as_bytes(), false)?;
    }
    // Rafraîchit les associations (« Ouvrir avec ») ; sans effet si l'outil est absent.
    let _ = std::process::Command::new("update-desktop-database").arg(share.join("applications")).status();
    Ok(())
}

pub fn remove(share: &Path) -> io::Result<()> {
    for f in files() {
        match fs::remove_file(share.join(f)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

pub fn is_integrated(share: &Path) -> bool {
    share.join("applications/morph.desktop").exists()
}

/// `~/.local/share` (ou `$XDG_DATA_HOME`).
pub fn share_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
}

/// Exécutable à lancer : l'AppImage elle-même (pas son point de montage temporaire).
pub fn executable() -> io::Result<PathBuf> {
    match std::env::var_os("APPIMAGE") {
        Some(p) => Ok(PathBuf::from(p)),
        None => std::env::current_exe(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_removes_integration() {
        let share = std::env::temp_dir().join("morph-test-linux");
        let _ = fs::remove_dir_all(&share);
        let exe = Path::new(r#"/home/moi/Mes Apps/Morph "v1" l'app.AppImage"#);
        integrate(exe, &share).unwrap();
        assert!(is_integrated(&share));

        let entry = fs::read_to_string(share.join("applications/morph.desktop")).unwrap();
        // `"` → `\"` (règle de Exec), puis `\` → `\\` (règle des chaînes).
        assert!(entry.contains(r#"Exec="/home/moi/Mes Apps/Morph \\"v1\\" l'app.AppImage" %F"#), "{entry}");
        assert!(entry.contains(r#"l'app.AppImage" --edit %F"#));
        assert!(entry.contains("MimeType=image/png;"));
        let script = fs::read_to_string(share.join("nautilus/scripts/Éditer avec Morph")).unwrap();
        assert_eq!(script, "#!/bin/sh\nexec '/home/moi/Mes Apps/Morph \"v1\" l'\\''app.AppImage' --edit \"$@\"\n");

        remove(&share).unwrap();
        assert!(!is_integrated(&share));
        assert!(!share.join("nautilus/scripts/Convertir avec Morph").exists());
        let _ = fs::remove_dir_all(&share);
    }
}
