//! Formats bureautiques (DOCX, ODT, XLSX, PPTX…) et export PDF par LibreOffice, s'il est
//! installé : lancé sans interface (`soffice --headless --convert-to`), avec un profil à
//! part pour ne pas dépendre d'un LibreOffice déjà ouvert par l'utilisateur.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::catalog::{self, Family};
use super::{ConvertError, no_window, ticking};

/// Au-delà, LibreOffice est considéré comme bloqué (boîte de dialogue invisible…).
const TIMEOUT: Duration = Duration::from_secs(180);

/// Programme de LibreOffice, s'il est installé.
pub fn soffice() -> Option<PathBuf> {
    candidates().into_iter().find(|p| p.is_file())
}

#[cfg(windows)]
fn candidates() -> Vec<PathBuf> {
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    use windows::core::w;
    let mut list = Vec::new();
    // Chemin déclaré par l'installeur de LibreOffice (machine ou utilisateur).
    for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        let mut buf = [0u16; 1024];
        let mut size = (buf.len() * 2) as u32;
        let found = unsafe {
            RegGetValueW(
                root,
                w!(r"SOFTWARE\LibreOffice\UNO\InstallPath"),
                None,
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut size),
            )
        };
        if found.is_ok() {
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            list.push(PathBuf::from(String::from_utf16_lossy(&buf[..len])).join("soffice.exe"));
        }
    }
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(dir) = std::env::var_os(var) {
            list.push(PathBuf::from(dir).join(r"LibreOffice\program\soffice.exe"));
        }
    }
    list
}

#[cfg(not(windows))]
fn candidates() -> Vec<PathBuf> {
    let mut list: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).flat_map(|d| [d.join("soffice"), d.join("libreoffice")]).collect())
        .unwrap_or_default();
    list.extend(
        ["/usr/lib/libreoffice/program/soffice", "/opt/libreoffice/program/soffice", "/snap/bin/libreoffice"]
            .map(PathBuf::from),
    );
    // Versions officielles installées dans /opt/libreofficeX.Y.
    if let Ok(entries) = std::fs::read_dir("/opt") {
        list.extend(
            entries
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("libreoffice"))
                .map(|e| e.path().join("program/soffice")),
        );
    }
    list
}

/// Filtre d'export de LibreOffice pour une extension cible.
fn filter(to: &str, family: Family) -> Option<&'static str> {
    Some(match (to, family) {
        ("pdf", Family::Sheet) => "pdf:calc_pdf_Export",
        ("pdf", Family::Slides) => "pdf:impress_pdf_Export",
        ("pdf", _) => "pdf:writer_pdf_Export",
        ("docx", _) => "docx:MS Word 2007 XML",
        ("odt", _) => "odt:writer8",
        ("rtf", _) => "rtf:Rich Text Format",
        ("xlsx", _) => "xlsx:Calc MS Excel 2007 XML",
        ("ods", _) => "ods:calc8",
        // Virgule, guillemets, UTF-8.
        ("csv", _) => "csv:Text - txt - csv (StarCalc):44,34,76,1",
        ("pptx", _) => "pptx:Impress MS PowerPoint 2007 XML",
        ("odp", _) => "odp:impress8",
        _ => return None,
    })
}

/// Profil LibreOffice de Morph, en URL (`file:///…`), gardé entre deux conversions.
fn profile_url() -> String {
    let dir = std::env::temp_dir().join("morph-libreoffice");
    let path = dir.to_string_lossy().replace('\\', "/");
    let encoded: String = path
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect();
    format!("file:///{}", encoded.trim_start_matches('/'))
}

/// Convertit `input` en `output` (extension `to`). Une seule conversion à la fois : les
/// instances de LibreOffice partageant un profil se gênent.
pub fn convert(input: &Path, output: &Path, to: &str, on_progress: &mut dyn FnMut(f32)) -> Result<(), ConvertError> {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    let soffice = soffice().ok_or(ConvertError::EngineMissing(super::engines::LIBREOFFICE_MISSING.reason))?;
    let from = catalog::find(input.extension().and_then(|e| e.to_str()).unwrap_or_default());
    let family = from.map_or(Family::Text, |f| f.family);
    let filter = filter(to, family).ok_or_else(|| ConvertError::NoRoute {
        from: from.map_or("?", |f| f.ext).into(),
        to: to.into(),
    })?;

    let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let work = tempdir()?;
    let mut cmd = Command::new(&soffice);
    no_window(&mut cmd);
    cmd.arg(format!("-env:UserInstallation={}", profile_url()))
        .args(["--headless", "--norestore", "--nolockcheck", "--nodefault", "--nologo"]);
    if from.is_some_and(|f| f.ext == "csv") {
        cmd.arg("--infilter=CSV:44,34,76,1");
    }
    cmd.args(["--convert-to", filter, "--outdir"])
        .arg(&work)
        .arg(input)
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    // Démarrage de LibreOffice compris : quelques secondes.
    let (status, _) = ticking(Duration::from_secs(6), 0.0, 0.95, on_progress, || run_with_timeout(cmd));
    let result = status.and_then(|log| {
        let stem = input.file_stem().unwrap_or_default();
        let produced = work.join(stem).with_extension(to);
        if produced.is_file() {
            move_file(&produced, output)
        } else {
            Err(ConvertError::Document(if log.is_empty() { "LibreOffice n'a rien produit".into() } else { log }))
        }
    });
    let _ = std::fs::remove_dir_all(&work);
    if result.is_ok() {
        on_progress(1.0);
    }
    result
}

/// Lance LibreOffice et attend sa fin (journal d'erreurs renvoyé), en l'arrêtant s'il dépasse
/// [`TIMEOUT`] ou si l'opération est annulée.
fn run_with_timeout(mut cmd: Command) -> Result<String, ConvertError> {
    let mut child = cmd
        .spawn()
        .map_err(|e| ConvertError::Document(format!("impossible de lancer LibreOffice : {e}")))?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            let mut log = String::new();
            if let Some(mut err) = child.stderr.take() {
                use std::io::Read;
                let _ = err.read_to_string(&mut log);
            }
            let log = log.trim().to_string();
            return if status.success() { Ok(log) } else { Err(ConvertError::Document(log)) };
        }
        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            return Err(ConvertError::Document("LibreOffice ne répond pas (délai dépassé)".into()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Dossier de travail vide, propre à cette conversion.
fn tempdir() -> Result<PathBuf, ConvertError> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!("morph-office-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Déplace un fichier, même vers un autre disque.
pub(crate) fn move_file(from: &Path, to: &Path) -> Result<(), ConvertError> {
    if std::fs::rename(from, to).is_err() {
        std::fs::copy(from, to)?;
        let _ = std::fs::remove_file(from);
    }
    Ok(())
}
