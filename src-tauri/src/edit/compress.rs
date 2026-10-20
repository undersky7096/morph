//! Compression : même format, poids réduit, résultat `nom Compressed.ext` à côté de l'original.
//! Rien n'est enregistré si le résultat n'est pas plus léger.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use image::DynamicImage;
use serde::{Deserialize, Serialize};

use crate::convert::formats::{self, Kind};
use crate::convert::{self, ConvertError, ffmpeg, raster};

const SUFFIX: &str = "Compressed";

/// Images compressibles (extension normalisée). BMP, TIFF, ICO, TGA : à convertir d'abord.
const IMAGE: &[&str] = &["png", "jpg", "webp", "gif"];
/// Audio compressible ; le WAV n'a pas de mode compressé (le convertir en FLAC ou MP3).
const AUDIO: &[&str] = &["mp3", "ogg", "opus", "m4a", "aac", "flac", "wma"];
/// Conteneurs vidéo conservés ; les autres (wmv, flv, mpeg, 3gp, ts) sont réencodés en MP4.
const VIDEO_KEPT: &[&str] = &["mp4", "mov", "mkv", "webm", "avi"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Light,
    Medium,
    Strong,
}

impl Level {
    fn pick<T>(self, light: T, medium: T, strong: T) -> T {
        match self {
            Level::Light => light,
            Level::Medium => medium,
            Level::Strong => strong,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub level: Level,
    /// PNG : réduction à 256 couleurs maximum (avec perte), en plus de l'optimisation sans perte.
    #[serde(default)]
    pub reduce_colors: bool,
    /// Audio/vidéo : taille visée en Mo (10⁶ octets) ; le débit est calculé d'après la durée.
    #[serde(default)]
    pub target_mb: Option<f64>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Outcome {
    #[serde(rename_all = "camelCase")]
    Saved { output: PathBuf, before: u64, after: u64 },
    /// Déjà aussi léger (ou sous la taille cible) : rien n'est enregistré.
    #[serde(rename_all = "camelCase")]
    NoGain { before: u64 },
}

pub fn supports(path: &Path) -> bool {
    let ext = formats::ext_of(path).unwrap_or_default();
    match formats::kind_of(path) {
        Some(Kind::Image) => IMAGE.contains(&ext.as_str()),
        Some(Kind::Audio) => AUDIO.contains(&ext.as_str()),
        Some(Kind::Video) => true,
        None => false,
    }
}

pub fn compress(input: &Path, opts: &Options, on_progress: &mut dyn FnMut(f32)) -> Result<Outcome, ConvertError> {
    if !input.is_file() {
        return Err(ConvertError::NotFound(input.display().to_string()));
    }
    if !supports(input) {
        return Err(ConvertError::Unsupported(input.display().to_string()));
    }
    let before = fs::metadata(input)?.len();
    let ext = formats::ext_of(input).unwrap_or_default();
    let kind = formats::kind_of(input).expect("type vérifié par supports");
    on_progress(0.0);

    if kind == Kind::Image {
        let data = compress_image(input, opts)?;
        on_progress(1.0);
        if data.len() as u64 >= before {
            return Ok(Outcome::NoGain { before });
        }
        let output = convert::suffixed_output(input, SUFFIX)?;
        fs::write(&output, &data)?;
        return Ok(Outcome::Saved { output, before, after: data.len() as u64 });
    }

    let target = opts.target_mb.filter(|_| ext != "flac").map(|mb| mb * 1e6);
    if target.is_some_and(|t| before as f64 <= t) {
        return Ok(Outcome::NoGain { before });
    }
    let out_ext = if kind == Kind::Video && !VIDEO_KEPT.contains(&ext.as_str()) { "mp4" } else { &ext };
    let output = convert::suffixed_output_as(input, SUFFIX, out_ext)?;
    match target {
        None => ffmpeg::run(input, &media_args(kind, out_ext, opts.level, None)?, &output, on_progress)?,
        Some(bytes) => {
            let total_kbps = bytes * 8.0 / 1000.0 / ffmpeg::probe_duration(input)? * 0.96;
            if kind == Kind::Audio {
                let kbps = total_kbps.clamp(32.0, 320.0) as u32;
                ffmpeg::run(input, &media_args(kind, out_ext, opts.level, Some(kbps))?, &output, on_progress)?;
            } else {
                two_pass(input, &output, out_ext, total_kbps, on_progress)?;
            }
        }
    }
    let after = fs::metadata(&output)?.len();
    if after >= before {
        let _ = fs::remove_file(&output);
        return Ok(Outcome::NoGain { before });
    }
    Ok(Outcome::Saved { output, before, after })
}

/// Image compressée, en mémoire (sert aussi à l'aperçu avant enregistrement).
pub fn compress_image(input: &Path, opts: &Options) -> Result<Vec<u8>, ConvertError> {
    match formats::ext_of(input).unwrap_or_default().as_str() {
        "png" => {
            let data = if opts.reduce_colors {
                // Qualité minimale inatteignable (dégradés riches) : on garde l'original.
                quantize(&raster::load(input)?, opts.level)?.map_or_else(|| fs::read(input), Ok)?
            } else {
                fs::read(input)?
            };
            let mut png_opts = oxipng::Options::from_preset(2);
            png_opts.strip = oxipng::StripChunks::Safe;
            oxipng::optimize_from_memory(&data, &png_opts).map_err(|e| ConvertError::Compress(e.to_string()))
        }
        "jpg" => raster::encode_jpeg(&raster::load(input)?, opts.level.pick(85, 75, 60)),
        "webp" => {
            let quality = opts.level.pick("80", "70", "55").to_string();
            via_ffmpeg(input, "webp", &["-c:v", "libwebp", "-quality", &quality, "-compression_level", "6"])
        }
        "gif" => {
            let colors = opts.level.pick(192, 128, 64);
            let filter = format!(
                "split[a][b];[a]palettegen=max_colors={colors}:stats_mode=diff[p];\
                 [b][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle"
            );
            via_ffmpeg(input, "gif", &["-vf", &filter, "-loop", "0"])
        }
        other => Err(ConvertError::Unsupported(other.to_string())),
    }
}

/// Quantification façon TinyPNG (libimagequant) en PNG indexé.
/// `None` si la qualité minimale du niveau ne peut pas être atteinte.
fn quantize(img: &DynamicImage, level: Level) -> Result<Option<Vec<u8>>, ConvertError> {
    let err = |e: imagequant::Error| ConvertError::Compress(e.to_string());
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let pixels: Vec<imagequant::RGBA> = rgba.pixels().map(|p| imagequant::RGBA::new(p[0], p[1], p[2], p[3])).collect();

    let mut attr = imagequant::Attributes::new();
    let (min, target) = level.pick((70, 95), (50, 85), (30, 70));
    attr.set_quality(min, target).map_err(err)?;
    let mut image = attr.new_image(pixels, width as usize, height as usize, 0.0).map_err(err)?;
    let mut result = match attr.quantize(&mut image) {
        Ok(result) => result,
        Err(imagequant::Error::QualityTooLow) => return Ok(None),
        Err(e) => return Err(err(e)),
    };
    result.set_dithering_level(1.0).map_err(err)?;
    let (palette, indexes) = result.remapped(&mut image).map_err(err)?;

    let png_err = |e: png::EncodingError| ConvertError::Compress(e.to_string());
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_palette(palette.iter().flat_map(|c| [c.r, c.g, c.b]).collect::<Vec<_>>());
        if palette.iter().any(|c| c.a < 255) {
            encoder.set_trns(palette.iter().map(|c| c.a).collect::<Vec<_>>());
        }
        let mut writer = encoder.write_header().map_err(png_err)?;
        writer.write_image_data(&indexes).map_err(png_err)?;
        writer.finish().map_err(png_err)?;
    }
    Ok(Some(out))
}

/// Encodage d'une image par ffmpeg via un fichier temporaire.
fn via_ffmpeg(input: &Path, ext: &str, args: &[&str]) -> Result<Vec<u8>, ConvertError> {
    let tmp = temp_path(ext);
    let data = ffmpeg::run(input, args, &tmp, &mut |_| {}).and_then(|_| Ok(fs::read(&tmp)?));
    let _ = fs::remove_file(&tmp);
    data
}

fn temp_path(ext: &str) -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("morph-{}-{n}.{ext}", std::process::id()))
}

/// Arguments d'encodage audio/vidéo. `kbps` : débit imposé (taille cible, audio seulement).
fn media_args(kind: Kind, ext: &str, level: Level, kbps: Option<u32>) -> Result<Vec<String>, ConvertError> {
    if kind == Kind::Video {
        let mut args = video_args(ext, level, None, None);
        args.extend(audio_args_for_video(ext, level.pick(160, 128, 96)));
        return Ok(args);
    }
    let mut args: Vec<String> = Vec::new();
    let mut push = |a: &[&str]| args.extend(a.iter().map(|s| s.to_string()));
    let bitrate = |l: u32, m: u32, s: u32| format!("{}k", kbps.unwrap_or_else(|| level.pick(l, m, s)));
    push(&["-vn"]);
    match ext {
        "mp3" => push(&["-c:a", "libmp3lame", "-b:a", &bitrate(192, 128, 96)]),
        "ogg" => push(&["-c:a", "libvorbis", "-b:a", &bitrate(160, 112, 80)]),
        "opus" => push(&["-c:a", "libopus", "-b:a", &bitrate(96, 64, 48)]),
        "m4a" => push(&["-c:a", "aac", "-b:a", &bitrate(160, 128, 96)]),
        "aac" => push(&["-c:a", "aac", "-b:a", &bitrate(160, 128, 96), "-f", "adts"]),
        "wma" => push(&["-c:a", "wmav2", "-b:a", &bitrate(128, 96, 64)]),
        "flac" => push(&["-c:a", "flac", "-compression_level", "12"]),
        other => return Err(ConvertError::Unsupported(other.to_string())),
    }
    Ok(args)
}

/// Arguments vidéo : qualité constante selon le niveau, ou débit fixe `kbps` (2 passes).
/// `max_side` : plus petit côté maximal (débit trop faible pour la résolution d'origine).
fn video_args(ext: &str, level: Level, kbps: Option<u32>, max_side: Option<u32>) -> Vec<String> {
    let scale = match max_side {
        // Paysage : hauteur plafonnée ; portrait : largeur. L'autre côté suit (pair).
        Some(cap) => format!(
            "scale='if(gte(iw,ih),-2,min({cap},trunc(iw/2)*2))':'if(gte(iw,ih),min({cap},trunc(ih/2)*2),-2)',setsar=1"
        ),
        // H.264/yuv420p exige des dimensions paires.
        None => "scale=trunc(iw/2)*2:trunc(ih/2)*2,setsar=1".to_string(),
    };
    let rate = |crf_flag: &str, crf: u32| match kbps {
        Some(k) => vec!["-b:v".to_string(), format!("{k}k")],
        None => vec![crf_flag.to_string(), crf.to_string()],
    };
    let mut args: Vec<String> = match ext {
        "webm" => {
            let mut a = vec!["-c:v".into(), "libvpx-vp9".into(), "-row-mt".into(), "1".into()];
            a.extend(rate("-crf", level.pick(34, 38, 43)));
            if kbps.is_none() {
                a.extend(["-b:v".into(), "0".into()]);
            }
            a
        }
        "avi" => {
            let mut a = vec!["-c:v".into(), "mpeg4".into()];
            a.extend(rate("-q:v", level.pick(4, 7, 11)));
            a
        }
        _ => {
            let mut a = vec!["-c:v".into(), "libx264".into(), "-preset".into(), "medium".into()];
            a.extend(rate("-crf", level.pick(24, 28, 32)));
            a.extend(["-pix_fmt".into(), "yuv420p".into()]);
            a
        }
    };
    args.extend(["-vf".into(), scale]);
    args
}

fn audio_args_for_video(ext: &str, kbps: u32) -> Vec<String> {
    let codec = match ext {
        "webm" => "libopus",
        "avi" => "libmp3lame",
        _ => "aac",
    };
    let mut args = vec!["-c:a".into(), codec.into(), "-b:a".into(), format!("{kbps}k")];
    if matches!(ext, "mp4" | "mov") {
        args.extend(["-movflags".into(), "+faststart".into()]);
    }
    args
}

/// Encodage vidéo en deux passes pour tenir la taille cible au plus près.
fn two_pass(
    input: &Path,
    output: &Path,
    ext: &str,
    total_kbps: f64,
    on_progress: &mut dyn FnMut(f32),
) -> Result<(), ConvertError> {
    let audio_kbps = if total_kbps >= 1000.0 { 128.0 } else if total_kbps >= 400.0 { 96.0 } else { 64.0 };
    let video_kbps = total_kbps - audio_kbps;
    if video_kbps < 60.0 {
        return Err(ConvertError::Compress("taille cible trop petite pour la durée de cette vidéo".into()));
    }
    // Débit faible : on réduit la résolution plutôt que d'obtenir une bouillie de blocs.
    let max_side = if video_kbps < 800.0 { Some(480) } else if video_kbps < 1600.0 { Some(720) } else { None };
    let video = video_args(ext, Level::Medium, Some(video_kbps as u32), max_side);
    let log = temp_path("2pass");
    let log_arg = log.display().to_string();

    let mut pass1 = video.clone();
    pass1.extend(["-pass", "1", "-passlogfile", &log_arg, "-an", "-f", "null"].map(String::from));
    let mut pass2 = video;
    pass2.extend(["-pass", "2", "-passlogfile", &log_arg].map(String::from));
    pass2.extend(audio_args_for_video(ext, audio_kbps as u32));

    let result = ffmpeg::run(input, &pass1, ffmpeg::null_output(), &mut |p| on_progress(p * 0.5))
        .and_then(|_| ffmpeg::run(input, &pass2, output, &mut |p| on_progress(0.5 + p * 0.5)));

    // Journaux de passes : `<log>-0.log` (+ `.mbtree` pour x264).
    if let (Some(dir), Some(name)) = (log.parent(), log.file_name().and_then(|n| n.to_str())) {
        for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
            if entry.file_name().to_str().is_some_and(|f| f.starts_with(name)) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Dégradé bruité : compressible, mais pas trivialement.
    fn sample(w: u32, h: u32) -> image::RgbaImage {
        image::RgbaImage::from_fn(w, h, |x, y| {
            let n = ((x * 7919 + y * 104_729) % 23) as u8;
            image::Rgba([(x * 255 / w) as u8 ^ n, (y * 255 / h) as u8, 128u8.wrapping_add(n), 255])
        })
    }

    #[test]
    fn png_reduce_colors_is_smaller() {
        let dir = test_dir("morph-test-compress-png");
        let input = dir.join("img.png");
        sample(300, 200).save(&input).unwrap();

        let opts = Options { level: Level::Medium, reduce_colors: true, target_mb: None };
        match compress(&input, &opts, &mut |_| {}).unwrap() {
            Outcome::Saved { output, before, after } => {
                assert_eq!(output, dir.join("img Compressed.png"));
                assert!(after < before);
                assert_eq!(image::image_dimensions(&output).unwrap(), (300, 200));
            }
            other => panic!("attendu Saved, obtenu {other:?}"),
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn jpeg_strong_is_smaller_than_light() {
        let dir = test_dir("morph-test-compress-jpg");
        let input = dir.join("photo.jpg");
        let img = DynamicImage::ImageRgba8(sample(400, 300));
        fs::write(&input, raster::encode_jpeg(&img, 98).unwrap()).unwrap();

        let size = |level| compress_image(&input, &Options { level, reduce_colors: false, target_mb: None }).unwrap().len();
        assert!(size(Level::Strong) < size(Level::Light));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn already_under_target_saves_nothing() {
        let dir = test_dir("morph-test-compress-target");
        let input = dir.join("clip.mp4");
        fs::write(&input, vec![0u8; 1000]).unwrap();
        let opts = Options { level: Level::Medium, reduce_colors: false, target_mb: Some(20.0) };
        assert!(matches!(compress(&input, &opts, &mut |_| {}).unwrap(), Outcome::NoGain { before: 1000 }));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn supported_formats() {
        assert!(supports(Path::new("a.PNG")));
        assert!(supports(Path::new("a.jpeg")));
        assert!(supports(Path::new("a.flv")));
        assert!(!supports(Path::new("a.bmp")));
        assert!(!supports(Path::new("a.wav")));
    }
}
