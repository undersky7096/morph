//! Découpe audio : on garde un ou plusieurs segments (garder A + C, enlever B),
//! résultat `nom Edited.ext` à côté de l'original, dans le même format.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Deserialize;

use crate::convert::formats::{self, Kind};
use crate::convert::{self, ConvertError, ffmpeg};

/// Résolution de la forme d'onde : un pic par 10 ms.
pub const PEAKS_PER_SEC: u32 = 100;
/// Fréquence du décodage pour la forme d'onde (suffisant pour l'allure du signal).
const PEAKS_RATE: u32 = 16_000;
/// Fondu aux points de coupe, pour éviter les clics.
const FADE_SECS: f64 = 0.008;
/// Segment plus court ignoré ; écart plus petit entre deux segments : recollés sans coupe.
const MIN_SECS: f64 = 0.01;
const JOIN_SECS: f64 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
}

fn check_audio(input: &Path) -> Result<(), ConvertError> {
    if !input.is_file() {
        return Err(ConvertError::NotFound(input.display().to_string()));
    }
    if formats::kind_of(input) != Some(Kind::Audio) {
        return Err(ConvertError::Unsupported(input.display().to_string()));
    }
    Ok(())
}

/// Forme d'onde (son d'un fichier audio ou d'une vidéo) : durée exacte (échantillons décodés)
/// et un pic (0-255) par 1/[`PEAKS_PER_SEC`] s. Vidéo muette : durée de l'en-tête, aucun pic.
pub fn peaks(input: &Path) -> Result<(f64, Vec<u8>), ConvertError> {
    if !input.is_file() {
        return Err(ConvertError::NotFound(input.display().to_string()));
    }
    let kind = formats::kind_of(input);
    if kind == Some(Kind::Video) && !ffmpeg::probe(input)?.contains("Audio: ") {
        return Ok((ffmpeg::probe_duration(input)?, Vec::new()));
    }
    if !matches!(kind, Some(Kind::Audio | Kind::Video)) {
        return Err(ConvertError::Unsupported(input.display().to_string()));
    }
    let mut child = ffmpeg::command()
        .arg("-i")
        .arg(input)
        .args(["-vn", "-ac", "1", "-ar", &PEAKS_RATE.to_string(), "-f", "s16le", "-c:a", "pcm_s16le", "pipe:1"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(ffmpeg::spawn_error)?;

    let per_peak = (PEAKS_RATE / PEAKS_PER_SEC) as usize;
    let to_byte = |max: u16| (max.min(32767) as u32 * 255 / 32767) as u8;
    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut buf = vec![0u8; 64 * 1024];
    let mut carry = 0; // octet d'un échantillon coupé entre deux lectures, recopié en tête
    let (mut peaks, mut max, mut count, mut total) = (Vec::new(), 0u16, 0usize, 0u64);
    loop {
        let n = stdout.read(&mut buf[carry..])?;
        if n == 0 {
            break;
        }
        let len = carry + n;
        let whole = len - len % 2;
        for c in buf[..whole].as_chunks::<2>().0 {
            max = max.max(i16::from_le_bytes([c[0], c[1]]).unsigned_abs());
            count += 1;
            total += 1;
            if count == per_peak {
                peaks.push(to_byte(max));
                (max, count) = (0, 0);
            }
        }
        carry = len - whole;
        if carry == 1 {
            buf[0] = buf[whole];
        }
    }
    if count > 0 {
        peaks.push(to_byte(max));
    }
    if !child.wait()?.success() || total == 0 {
        return Err(ConvertError::Ffmpeg(format!("lecture audio impossible : {}", input.display())));
    }
    Ok((total as f64 / PEAKS_RATE as f64, peaks))
}

/// Version WAV pour l'écoute, quand le webview ne sait pas lire le format d'origine (WMA…).
pub fn preview_wav(input: &Path) -> Result<Vec<u8>, ConvertError> {
    check_audio(input)?;
    let out = ffmpeg::command()
        .arg("-i")
        .arg(input)
        .args(["-vn", "-ac", "2", "-ar", "32000", "-c:a", "pcm_s16le", "-f", "wav", "pipe:1"])
        .stderr(Stdio::null())
        .output()
        .map_err(ffmpeg::spawn_error)?;
    if !out.status.success() || out.stdout.is_empty() {
        return Err(ConvertError::Ffmpeg(format!("lecture audio impossible : {}", input.display())));
    }
    Ok(out.stdout)
}

/// Segments triés, ramenés dans le fichier, trop courts retirés, contigus recollés.
pub(crate) fn normalize(segments: &[Segment], duration: f64) -> Vec<Segment> {
    let mut segs: Vec<Segment> = segments
        .iter()
        .map(|s| Segment { start: s.start.max(0.0), end: s.end.min(duration) })
        .filter(|s| s.end - s.start >= MIN_SECS)
        .collect();
    segs.sort_by(|a, b| a.start.total_cmp(&b.start));
    let mut out: Vec<Segment> = Vec::with_capacity(segs.len());
    for s in segs {
        match out.last_mut() {
            Some(last) if s.start <= last.end + JOIN_SECS => last.end = last.end.max(s.end),
            _ => out.push(s),
        }
    }
    out
}

/// Graphe de filtres : chaque segment est extrait (fondu aux points de coupe), puis concaténé.
pub(crate) fn filter_graph(segs: &[Segment], duration: f64) -> String {
    let mut graph = String::new();
    for (i, s) in segs.iter().enumerate() {
        let len = s.end - s.start;
        graph += &format!("[0:a]atrim=start={:.6}:end={:.6},asetpts=PTS-STARTPTS", s.start, s.end);
        if s.start > JOIN_SECS {
            graph += &format!(",afade=t=in:d={FADE_SECS}");
        }
        if s.end < duration - JOIN_SECS {
            graph += &format!(",afade=t=out:st={:.6}:d={FADE_SECS}", (len - FADE_SECS).max(0.0));
        }
        graph += &format!("[a{i}];");
    }
    for i in 0..segs.len() {
        graph += &format!("[a{i}]");
    }
    graph += &format!("concat=n={}:v=0:a=1[out]", segs.len());
    graph
}

/// Codec et débit du premier flux audio (`Stream #0:0: Audio: mp3 (mp3float), 44100 Hz, stereo, fltp, 128 kb/s`).
pub(crate) fn source_audio(probe: &str) -> (String, Option<u32>) {
    let Some(line) = probe.lines().find(|l| l.contains("Stream #") && l.contains("Audio: ")) else {
        return (String::new(), None);
    };
    let rest = &line[line.find("Audio: ").unwrap() + 7..];
    let codec = rest.split([' ', ',']).next().unwrap_or("").to_string();
    let kbps = rest
        .split(',')
        .find_map(|part| part.trim().strip_suffix(" kb/s")?.trim().parse::<u32>().ok());
    (codec, kbps)
}

/// Encodage dans le format d'origine, au débit de la source quand il est connu
/// (une découpe ne doit ni dégrader ni gonfler le fichier).
fn codec_args(ext: &str, codec: &str, kbps: Option<u32>, s16: bool) -> Result<Vec<String>, ConvertError> {
    let rate = |default: u32, min: u32, max: u32| format!("{}k", kbps.unwrap_or(default).clamp(min, max));
    let args: Vec<String> = match ext {
        "mp3" => vec!["-c:a".into(), "libmp3lame".into(), "-b:a".into(), rate(192, 64, 320)],
        "ogg" => vec!["-c:a".into(), "libvorbis".into(), "-b:a".into(), rate(192, 64, 320)],
        "opus" => vec!["-c:a".into(), "libopus".into(), "-b:a".into(), rate(128, 32, 256)],
        "m4a" => vec!["-c:a".into(), "aac".into(), "-b:a".into(), rate(192, 64, 320)],
        "aac" => vec!["-c:a".into(), "aac".into(), "-b:a".into(), rate(192, 64, 320), "-f".into(), "adts".into()],
        "wma" => vec!["-c:a".into(), "wmav2".into(), "-b:a".into(), rate(192, 48, 320)],
        // Les filtres travaillent en flottant : sans précision, un FLAC 16 bits ressortirait en 24.
        "flac" if s16 => vec!["-c:a".into(), "flac".into(), "-sample_fmt".into(), "s16".into()],
        "flac" => vec!["-c:a".into(), "flac".into()],
        // Même profondeur que la source (16, 24, 32 bits…).
        "wav" => {
            let pcm = if codec.starts_with("pcm_") { codec } else { "pcm_s16le" };
            vec!["-c:a".into(), pcm.into()]
        }
        other => return Err(ConvertError::Unsupported(other.to_string())),
    };
    Ok(args)
}

/// Gain (0…2) : au-delà de 1, un limiteur évite la saturation.
pub(crate) fn volume_filter(volume: f64) -> Option<String> {
    if (volume - 1.0).abs() < 1e-6 {
        return None;
    }
    let mut f = format!("volume={:.3}", volume.clamp(0.0, 2.0));
    if volume > 1.0 {
        f += ",alimiter=limit=0.98:level=false";
    }
    Some(f)
}

/// Garde les `segments` (en secondes) de `input`, dans l'ordre, au volume `volume` (0…2),
/// et enregistre le résultat.
pub fn trim(
    input: &Path,
    segments: &[Segment],
    volume: f64,
    on_progress: &mut dyn FnMut(f32),
) -> Result<PathBuf, ConvertError> {
    check_audio(input)?;
    let ext = formats::ext_of(input).ok_or_else(|| ConvertError::Unsupported(input.display().to_string()))?;
    let probe = ffmpeg::probe(input)?;
    let duration = ffmpeg::probe_duration(input)?;
    let segs = normalize(segments, duration);
    if segs.is_empty() {
        return Err(ConvertError::Edit("aucun passage à garder".into()));
    }
    let kept: f64 = segs.iter().map(|s| s.end - s.start).sum();

    let (codec, kbps) = source_audio(&probe);
    let s16 = probe.lines().any(|l| l.contains("Audio: ") && l.contains(", s16"));
    let mut graph = filter_graph(&segs, duration);
    if let Some(gain) = volume_filter(volume) {
        graph = graph.replace("[out]", "[cat];[cat]") + &format!("{gain}[out]");
    }
    let mut args: Vec<String> = vec!["-filter_complex".into(), graph, "-map".into(), "[out]".into()];
    // Pochette conservée là où le conteneur la prend en charge.
    if matches!(ext.as_str(), "mp3" | "flac" | "m4a") && probe.contains("Video: ") {
        args.extend(["-map", "0:v:0", "-c:v", "copy", "-disposition:v:0", "attached_pic"].map(String::from));
    }
    args.extend(codec_args(&ext, &codec, kbps, s16)?);

    let output = convert::suffixed_output(input, "Edited")?;
    on_progress(0.0);
    ffmpeg::run_for(input, &args, &output, Some(kept), None, on_progress)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn seg(start: f64, end: f64) -> Segment {
        Segment { start, end }
    }

    #[test]
    fn normalizes_segments() {
        let segs = [seg(5.0, 8.0), seg(-1.0, 2.0), seg(2.0, 3.0), seg(9.0, 9.005), seg(9.5, 20.0)];
        assert_eq!(normalize(&segs, 10.0), vec![seg(0.0, 3.0), seg(5.0, 8.0), seg(9.5, 10.0)]);
    }

    #[test]
    fn fades_only_at_cuts() {
        let graph = filter_graph(&[seg(0.0, 2.0), seg(3.0, 10.0)], 10.0);
        assert_eq!(
            graph,
            "[0:a]atrim=start=0.000000:end=2.000000,asetpts=PTS-STARTPTS,afade=t=out:st=1.992000:d=0.008[a0];\
             [0:a]atrim=start=3.000000:end=10.000000,asetpts=PTS-STARTPTS,afade=t=in:d=0.008[a1];\
             [a0][a1]concat=n=2:v=0:a=1[out]"
        );
    }

    #[test]
    fn reads_source_audio() {
        let probe = "  Duration: 00:03:12.00, start: 0.025057, bitrate: 129 kb/s\n  \
                     Stream #0:0: Audio: mp3 (mp3float), 44100 Hz, stereo, fltp, 128 kb/s\n  \
                     Stream #0:1: Video: mjpeg (Baseline), yuvj420p, 500x500, 90k tbr (attached pic)";
        assert_eq!(source_audio(probe), ("mp3".to_string(), Some(128)));
        let wav = "  Stream #0:0: Audio: pcm_s24le ([1][0][0][0] / 0x0001), 48000 Hz, 2 channels, s32 (24 bit), 2304 kb/s";
        assert_eq!(source_audio(wav).0, "pcm_s24le");
    }

    /// Nécessite ffmpeg (sidecar ou PATH) ; ignoré sinon.
    #[test]
    fn trims_and_reads_peaks() {
        if ffmpeg::command().arg("-version").output().is_err() {
            eprintln!("ffmpeg absent : test ignoré");
            return;
        }
        let dir = std::env::temp_dir().join("morph-test-trim");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("tone.mp3");
        let status = ffmpeg::command()
            .args(["-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=4", "-c:a", "libmp3lame", "-b:a", "128k"])
            .arg(&input)
            .status()
            .unwrap();
        assert!(status.success());

        let (duration, peaks) = peaks(&input).unwrap();
        assert!((duration - 4.0).abs() < 0.1, "durée {duration}");
        assert!((peaks.len() as f64 - duration * PEAKS_PER_SEC as f64).abs() <= 1.0);
        assert!(peaks[200] > 20, "pic {}", peaks[200]); // sinus lavfi : amplitude 1/8

        let out = trim(&input, &[seg(0.0, 1.0), seg(2.5, 3.5)], 1.5, &mut |_| {}).unwrap();
        assert_eq!(out, dir.join("tone Edited.mp3"));
        let kept = ffmpeg::probe_duration(&out).unwrap();
        assert!((kept - 2.0).abs() < 0.1, "durée gardée {kept}");
        // Volume à 150 % : le signal est plus fort.
        let (_, louder) = super::peaks(&out).unwrap();
        assert!(louder[50] as f64 > peaks[50] as f64 * 1.3, "{} → {}", peaks[50], louder[50]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
