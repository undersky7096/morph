use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;

use image::DynamicImage;

use super::catalog::{self, Kind};
use super::{ConvertError, formats};

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
        "wmv" => &["-c:v", "wmv2", "-q:v", "3", "-c:a", "wmav2", "-b:a", "192k"],
        "flv" => &[
            "-vf", EVEN, "-c:v", "libx264", "-preset", "medium", "-crf", "23",
            "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "160k", "-ar", "44100",
        ],
        // Téléphones anciens : H.264 baseline, 720p au plus.
        "3gp" => &[
            "-vf", "scale='trunc(min(1280,iw)/2)*2':-2,setsar=1", "-c:v", "libx264",
            "-profile:v", "baseline", "-crf", "26", "-pix_fmt", "yuv420p",
            "-c:a", "aac", "-b:a", "96k", "-ar", "44100",
        ],
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
        "aac" => &["-vn", "-c:a", "aac", "-b:a", "192k"],
        "wma" => &["-vn", "-c:a", "wmav2", "-b:a", "192k"],
        "aiff" => &["-vn", "-c:a", "pcm_s16be"],
        other => return Err(ConvertError::Unsupported(other.to_string())),
    };
    Ok(args.to_vec())
}

/// Son réencodé quand l'image est recopiée telle quelle mais pas le son.
fn audio_args(target: &str) -> &'static [&'static str] {
    match target {
        "webm" => &["-c:a", "libopus", "-b:a", "128k"],
        _ => &["-c:a", "aac", "-b:a", "192k"],
    }
}

/// Conversion audio/vidéo. `on_progress` reçoit une valeur entre 0.0 et 1.0.
/// Quand le conteneur cible accepte les codecs de la source, les flux sont recopiés sans
/// réencodage (bien plus rapide, sans perte) ; en cas d'échec, on réencode.
pub fn convert(
    input: &Path,
    output: &Path,
    target: &str,
    on_progress: &mut dyn FnMut(f32),
) -> Result<(), ConvertError> {
    if let Some(args) = probe(input).ok().and_then(|header| copy_args(input, &header, target)) {
        match run(input, &args, output, on_progress) {
            Err(ConvertError::Ffmpeg(_)) => {}
            result => return result,
        }
    }
    run(input, &codec_args(target)?, output, on_progress)
}

/// Codecs de la vidéo principale et des pistes audio, d'après l'en-tête de `ffmpeg -i`
/// (les pochettes, « attached pic », ne comptent pas).
fn stream_codecs(header: &str) -> (Option<&str>, Vec<&str>) {
    fn codec<'a>(line: &'a str, tag: &str) -> Option<&'a str> {
        let rest = &line[line.find(tag)? + tag.len()..];
        rest.split([' ', ',']).next().filter(|c| !c.is_empty())
    }
    let streams = header.lines().filter(|l| l.trim_start().starts_with("Stream #"));
    let mut video = None;
    let mut audio = Vec::new();
    for line in streams {
        if video.is_none() && !line.contains("(attached pic)") {
            video = codec(line, "Video: ");
        }
        audio.extend(codec(line, "Audio: "));
    }
    (video, audio)
}

/// Arguments pour recopier les flux de `input` vers `target` sans les réencoder, si le
/// conteneur cible les accepte (au moins l'image, pour une vidéo).
fn copy_args(input: &Path, header: &str, target: &str) -> Option<Vec<&'static str>> {
    let src = formats::format_of(input)?;
    let to = catalog::find(target)?;
    if src.kind == Kind::Image || to.kind == Kind::Image {
        return None;
    }
    let (video, audio) = stream_codecs(header);
    let audio_ok = audio.iter().all(|c| to.copy_audio.contains(c));
    let mut args = vec!["-sn", "-dn"];
    if to.kind == Kind::Audio {
        if audio.is_empty() || !audio_ok {
            return None;
        }
        args.extend(["-vn", "-c:a", "copy"]);
    } else {
        let video = video.filter(|v| to.copy_video.contains(v))?;
        args.extend(["-c:v", "copy"]);
        args.extend(if audio_ok { &["-c:a", "copy"][..] } else { audio_args(target) });
        if matches!(target, "mp4" | "mov") {
            args.extend(["-movflags", "+faststart"]);
            // Étiquette attendue par les lecteurs d'Apple pour le HEVC.
            if video == "hevc" {
                args.extend(["-tag:v", "hvc1"]);
            }
        }
    }
    Some(args)
}

/// Image fixe lue par ffmpeg (HEIC, AVIF) : première image, transparence AVIF comprise
/// (stockée dans un second flux, recombiné ici). Rotation et tuiles HEIC sont appliquées.
pub fn decode_still(input: &Path) -> Result<DynamicImage, ConvertError> {
    let header = probe(input)?;
    let mut videos = header.lines().filter(|l| l.contains("Video: "));
    let alpha = videos.nth(1).is_some_and(|l| l.contains("gray"));
    let mut cmd = command();
    cmd.args(["-loglevel", "error", "-i"]).arg(input);
    if alpha {
        cmd.args(["-filter_complex", "[0:v:0][0:v:1]alphamerge"]);
    }
    let out = cmd.args(["-frames:v", "1", "-f", "image2pipe", "-c:v", "png", "-"]).output().map_err(spawn_error)?;
    if !out.status.success() || out.stdout.is_empty() {
        return Err(ConvertError::Ffmpeg(String::from_utf8_lossy(&out.stderr).into_owned()));
    }
    Ok(image::load_from_memory_with_format(&out.stdout, image::ImageFormat::Png)?)
}

/// Enregistre une image en AVIF (libaom, image fixe). La transparence, s'il y en a, part
/// dans un second flux comme le prévoit le format.
pub fn encode_avif(img: &DynamicImage, output: &Path) -> Result<(), ConvertError> {
    encode_still(img, output, |cmd, alpha| {
        if alpha {
            cmd.args(["-filter_complex", "[0]format=yuva420p,split[c][a];[a]alphaextract[al]", "-map", "[c]", "-map", "[al]"]);
        } else {
            cmd.args(["-pix_fmt", "yuv420p"]);
        }
        cmd.args(["-c:v", "libaom-av1", "-still-picture", "1", "-crf", "28", "-cpu-used", "6", "-row-mt", "1"])
            .args(["-f", "avif"]);
    })
}

/// WebP avec perte (libwebp, qualité 82, transparence gardée). L'encodeur de la crate
/// `image` ne fait que du sans perte, souvent plus lourd que la photo d'origine.
pub fn encode_webp(img: &DynamicImage, output: &Path) -> Result<(), ConvertError> {
    encode_still(img, output, |cmd, alpha| {
        cmd.args(["-pix_fmt", if alpha { "yuva420p" } else { "yuv420p" }])
            .args(["-c:v", "libwebp", "-quality", "82", "-compression_level", "4", "-f", "webp"]);
    })
}

/// Encode une image fixe : pixels RGBA envoyés à ffmpeg par stdin, `codec` ajoute les
/// arguments d'encodage (son second paramètre dit si l'image a de la transparence).
fn encode_still(img: &DynamicImage, output: &Path, codec: impl FnOnce(&mut Command, bool)) -> Result<(), ConvertError> {
    use std::io::Write;
    let rgba = img.to_rgba8();
    let alpha = rgba.pixels().any(|p| p.0[3] < 255);
    let size = format!("{}x{}", rgba.width(), rgba.height());
    let mut cmd = command();
    cmd.args(["-loglevel", "error", "-n", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &size, "-i", "-"]);
    codec(&mut cmd, alpha);
    cmd.arg(output)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(spawn_error)?;
    let mut stdin = child.stdin.take().expect("stdin piped");
    let writer = thread::spawn(move || stdin.write_all(rgba.as_raw()));
    let out = child.wait_with_output()?;
    let written = writer.join().map(|r| r.is_ok()).unwrap_or(false);
    if out.status.success() && written {
        Ok(())
    } else {
        let _ = std::fs::remove_file(output);
        Err(ConvertError::Ffmpeg(String::from_utf8_lossy(&out.stderr).into_owned()))
    }
}

/// Sortie à jeter (1re passe d'un encodage en deux passes), avec `-f null`.
pub fn null_output() -> &'static Path {
    Path::new(if cfg!(windows) { "NUL" } else { "/dev/null" })
}

pub(crate) fn command() -> Command {
    let mut cmd = Command::new(ffmpeg_path());
    cmd.args(["-hide_banner", "-nostdin", "-nostats"]);
    super::no_window(&mut cmd);
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
/// (jamais écrasé). `on_progress` reçoit une valeur entre 0.0 et 1.0. S'arrête si
/// l'opération est annulée (voir [`super::output`]) ; le fichier partiel est supprimé.
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
    let mut killed = false;
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        // Annulation : vérifiée à chaque ligne de progression (environ deux fois par seconde).
        if !killed && super::output::cancelled() {
            let _ = child.kill();
            killed = true;
        }
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
    if status.success() && !killed {
        Ok(())
    } else {
        if output != null_output() {
            let _ = std::fs::remove_file(output);
        }
        Err(if killed { ConvertError::Cancelled } else { ConvertError::Ffmpeg(log) })
    }
}

/// Extrait la durée d'une ligne `  Duration: 00:01:02.34, start: …` en microsecondes.
pub(crate) fn parse_duration(line: &str) -> Option<u64> {
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

    /// Nécessite ffmpeg (sidecar ou PATH) ; ignoré sinon.
    #[test]
    fn cancel_stops_encoding_and_removes_partial_file() {
        use std::sync::atomic::AtomicBool;
        use crate::convert::output::{Scope, scoped};
        if command().arg("-version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join("morph-test-cancel");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("long.mp4");
        let ok = command()
            .args(["-loglevel", "error", "-f", "lavfi", "-i", "testsrc2=size=640x360:rate=30:duration=30", "-c:v", "libx264", "-preset", "ultrafast"])
            .arg(&input)
            .status()
            .unwrap()
            .success();
        assert!(ok);

        let flag = Arc::new(AtomicBool::new(false));
        let stop = flag.clone();
        thread::spawn(move || {
            thread::sleep(std::time::Duration::from_millis(600));
            stop.store(true, Ordering::Relaxed);
        });
        let output = dir.join("long.webm");
        let result = scoped(Scope { cancel: vec![flag], ..Scope::default() }, || {
            convert(&input, &output, "webm", &mut |_| {})
        });
        assert!(matches!(result, Err(ConvertError::Cancelled)), "{result:?}");
        assert!(!output.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Nécessite ffmpeg avec libwebp ; ignoré sinon.
    #[test]
    fn webp_is_lossy_and_keeps_transparency() {
        if command().arg("-version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join("morph-test-webp");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Photo bruitée (proche d'une vraie photo), puis la même avec un coin transparent.
        let photo = image::RgbaImage::from_fn(640, 480, |x, y| {
            let n = ((x * 7919 + y * 104_729) % 251) as u8;
            image::Rgba([(x / 3) as u8 ^ n, (y / 2) as u8, n, if x < 64 && y < 64 { 0 } else { 255 }])
        });
        let img = DynamicImage::ImageRgba8(photo);

        let lossy = dir.join("lossy.webp");
        encode_webp(&img, &lossy).unwrap();
        let lossless = dir.join("lossless.webp");
        img.save_with_format(&lossless, image::ImageFormat::WebP).unwrap();
        let size = |p: &Path| std::fs::metadata(p).unwrap().len();
        assert!(size(&lossy) * 2 < size(&lossless), "{} contre {}", size(&lossy), size(&lossless));

        let back = image::open(&lossy).unwrap().to_rgba8();
        assert_eq!(back.dimensions(), (640, 480));
        assert!(back.get_pixel(10, 10).0[3] < 32, "le coin transparent doit le rester");
        assert!(back.get_pixel(320, 240).0[3] > 223);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_stream_codecs() {
        let header = [
            "  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p, 1920x1080",
            "  Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706D), 48000 Hz, stereo",
            "  Stream #0:2: Video: mjpeg (Baseline), yuvj420p, 600x600 (attached pic)",
        ]
        .join("\n");
        assert_eq!(stream_codecs(&header), (Some("h264"), vec!["aac"]));
        let header = "  Stream #0:0: Video: mjpeg, yuvj420p, 600x600 (attached pic)\n  Stream #0:1: Audio: mp3, 44100 Hz";
        assert_eq!(stream_codecs(header), (None, vec!["mp3"]));
    }

    #[test]
    fn copies_only_what_the_container_accepts() {
        let has = |args: &[&str], pair: [&str; 2]| args.windows(2).any(|w| w == pair);
        let h264_aac = "  Stream #0:0: Video: h264 (High), yuv420p\n  Stream #0:1: Audio: aac (LC), 48000 Hz";
        let args = copy_args(Path::new("a.mkv"), h264_aac, "mp4").unwrap();
        assert!(has(&args, ["-c:v", "copy"]) && has(&args, ["-c:a", "copy"]));
        assert!(copy_args(Path::new("a.mp4"), h264_aac, "webm").is_none());
        assert!(copy_args(Path::new("a.mp4"), h264_aac, "m4a").unwrap().contains(&"-vn"));
        assert!(copy_args(Path::new("a.mp4"), h264_aac, "mp3").is_none());
        let vp9_opus = "  Stream #0:0: Video: vp9, yuv420p\n  Stream #0:1: Audio: opus, 48000 Hz";
        assert!(has(&copy_args(Path::new("a.webm"), vp9_opus, "mkv").unwrap(), ["-c:a", "copy"]));
        // L'image est recopiée, le son réencodé.
        let av1_opus = "  Stream #0:0: Video: av1, yuv420p\n  Stream #0:1: Audio: opus, 48000 Hz";
        let args = copy_args(Path::new("a.webm"), av1_opus, "mp4").unwrap();
        assert!(has(&args, ["-c:v", "copy"]) && has(&args, ["-c:a", "aac"]));
    }

    #[test]
    fn parses_duration() {
        assert_eq!(
            parse_duration("  Duration: 00:01:02.50, start: 0.000000, bitrate: 128 kb/s"),
            Some(62_500_000)
        );
        assert_eq!(parse_duration("  Duration: N/A, bitrate: N/A"), None);
    }
}
