use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use super::ConvertError;

/// Chemin de ffmpeg : sidecar installé à côté de l'exécutable, sinon celui du PATH.
pub fn ffmpeg_path() -> PathBuf {
    let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    std::env::current_exe()
        .ok()
        .map(|exe| exe.with_file_name(name))
        .filter(|p| p.exists())
        .unwrap_or_else(|| PathBuf::from("ffmpeg"))
}

/// Arguments d'encodage selon le format cible.
fn codec_args(target: &str) -> Result<Vec<&'static str>, ConvertError> {
    // H.264/yuv420p exige des dimensions paires.
    const EVEN: &str = "scale=trunc(iw/2)*2:trunc(ih/2)*2,setsar=1";
    let args: &[&str] = match target {
        "mp4" | "mov" | "mkv" => &[
            "-vf", EVEN, "-c:v", "libx264", "-preset", "medium", "-crf", "23",
            "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart",
        ],
        "webm" => &[
            "-c:v", "libvpx-vp9", "-crf", "32", "-b:v", "0", "-row-mt", "1",
            "-c:a", "libopus", "-b:a", "128k",
        ],
        "avi" => &["-c:v", "mpeg4", "-q:v", "3", "-c:a", "libmp3lame", "-q:a", "3"],
        "gif" => &[
            "-an", "-vf",
            "fps=15,scale='min(720,iw)':-1:flags=lanczos,split[a][b];[a]palettegen[p];[b][p]paletteuse",
            "-loop", "0",
        ],
        "mp3" => &["-vn", "-c:a", "libmp3lame", "-q:a", "2"],
        "wav" => &["-vn", "-c:a", "pcm_s16le"],
        "ogg" => &["-vn", "-c:a", "libvorbis", "-q:a", "5"],
        "flac" => &["-vn", "-c:a", "flac"],
        "m4a" => &["-vn", "-c:a", "aac", "-b:a", "192k"],
        "opus" => &["-vn", "-c:a", "libopus", "-b:a", "128k"],
        other => return Err(ConvertError::Unsupported(other.to_string())),
    };
    Ok(args.to_vec())
}

/// Conversion audio/vidéo. `on_progress` reçoit une valeur entre 0.0 et 1.0.
pub fn convert(
    input: &Path,
    output: &Path,
    target: &str,
    on_progress: &mut dyn FnMut(f32),
) -> Result<(), ConvertError> {
    run(input, &codec_args(target)?, output, on_progress)
}

/// Sortie à jeter (1re passe d'un encodage en deux passes), avec `-f null`.
pub fn null_output() -> &'static Path {
    Path::new(if cfg!(windows) { "NUL" } else { "/dev/null" })
}

pub(crate) fn command() -> Command {
    let mut cmd = Command::new(ffmpeg_path());
    cmd.args(["-hide_banner", "-nostdin", "-nostats"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

pub(crate) fn spawn_error(e: std::io::Error) -> ConvertError {
    ConvertError::Ffmpeg(format!("impossible de lancer ffmpeg ({}) : {e}", ffmpeg_path().display()))
}

/// En-tête affiché par `ffmpeg -i` (durée, flux, codecs, débits).
pub fn probe(input: &Path) -> Result<String, ConvertError> {
    let out = command().arg("-i").arg(input).output().map_err(spawn_error)?;
    Ok(String::from_utf8_lossy(&out.stderr).into_owned())
}

/// Durée d'un fichier audio/vidéo en secondes (lue dans l'en-tête affiché par `ffmpeg -i`).
pub fn probe_duration(input: &Path) -> Result<f64, ConvertError> {
    probe(input)?
        .lines()
        .find_map(parse_duration)
        .map(|us| us as f64 / 1_000_000.0)
        .ok_or_else(|| ConvertError::Ffmpeg(format!("durée inconnue : {}", input.display())))
}

/// Lance ffmpeg sur `input` avec les arguments d'encodage `args`, vers `output`
/// (jamais écrasé). `on_progress` reçoit une valeur entre 0.0 et 1.0.
pub fn run(
    input: &Path,
    args: &[impl AsRef<std::ffi::OsStr>],
    output: &Path,
    on_progress: &mut dyn FnMut(f32),
) -> Result<(), ConvertError> {
    run_for(input, args, output, None, None, on_progress)
}

/// Comme [`run`], quand la sortie ne dure pas autant que l'entrée (découpe) :
/// la progression se mesure alors sur `out_secs`. `cwd` : dossier de travail, pour
/// désigner des fichiers annexes par un nom simple dans les filtres (les chemins
/// Windows, avec leurs `C:\`, y sont pénibles à échapper).
pub fn run_for(
    input: &Path,
    args: &[impl AsRef<std::ffi::OsStr>],
    output: &Path,
    out_secs: Option<f64>,
    cwd: Option<&Path>,
    on_progress: &mut dyn FnMut(f32),
) -> Result<(), ConvertError> {
    let mut cmd = command();
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.args(["-n", "-i"])
        .arg(input)
        .args(args)
        .args(["-progress", "pipe:1"])
        .arg(output)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(spawn_error)?;

    // stderr : on y lit la durée totale, et on garde les dernières lignes pour les erreurs.
    let duration_us = Arc::new(AtomicU64::new(out_secs.map_or(0, |s| (s * 1_000_000.0) as u64)));
    let stderr = child.stderr.take().expect("stderr piped");
    let stderr_thread = {
        let duration_us = Arc::clone(&duration_us);
        thread::spawn(move || {
            let mut tail = VecDeque::with_capacity(10);
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if duration_us.load(Ordering::Relaxed) == 0 {
                    if let Some(us) = parse_duration(&line) {
                        duration_us.store(us, Ordering::Relaxed);
                    }
                }
                if tail.len() == 10 {
                    tail.pop_front();
                }
                tail.push_back(line);
            }
            Vec::from(tail).join("\n")
        })
    };

    // stdout : flux `-progress` (lignes clé=valeur).
    let stdout = child.stdout.take().expect("stdout piped");
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if let Some(value) = line.strip_prefix("out_time_us=") {
            let total = duration_us.load(Ordering::Relaxed);
            if let (Ok(done), true) = (value.parse::<u64>(), total > 0) {
                on_progress((done as f32 / total as f32).clamp(0.0, 1.0));
            }
        } else if line == "progress=end" {
            on_progress(1.0);
        }
    }

    let status = child.wait()?;
    let log = stderr_thread.join().unwrap_or_default();
    if status.success() {
        Ok(())
    } else {
        if output != null_output() {
            let _ = std::fs::remove_file(output);
        }
        Err(ConvertError::Ffmpeg(log))
    }
}

/// Extrait la durée d'une ligne `  Duration: 00:01:02.34, start: …` en microsecondes.
fn parse_duration(line: &str) -> Option<u64> {
    let rest = line.trim_start().strip_prefix("Duration: ")?;
    let hms = rest.split(',').next()?;
    let mut parts = hms.split(':');
    let h: f64 = parts.next()?.parse().ok()?;
    let m: f64 = parts.next()?.parse().ok()?;
    let s: f64 = parts.next()?.parse().ok()?;
    Some(((h * 3600.0 + m * 60.0 + s) * 1_000_000.0) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_duration() {
        assert_eq!(
            parse_duration("  Duration: 00:01:02.50, start: 0.000000, bitrate: 128 kb/s"),
            Some(62_500_000)
        );
        assert_eq!(parse_duration("  Duration: N/A, bitrate: N/A"), None);
    }
}
