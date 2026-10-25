//! Éditeur vidéo : découpe en plusieurs passages, réglages et filtres (mêmes formules que
//! l'éditeur image), rotation/miroir, cadre (ratio recadré ou ajusté), résolution, vitesse
//! et son. Résultat `nom Edited.ext` à côté de l'original, dans le même conteneur.
//!
//! Les couleurs passent par une LUT 3D calculée avec les formules du shader de l'aperçu
//! (`src/tools/edit/renderer.ts`) et le vignettage par un masque exact : le rendu
//! enregistré correspond à ce qui est affiché.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use serde::Deserialize;

use super::trim::{self, Segment};
use crate::convert::formats::{self, Kind};
use crate::convert::{self, ConvertError, ffmpeg};

/// Réglages, déjà combinés avec le filtre prédéfini (mêmes échelles que l'éditeur image).
#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default)]
pub struct Adjust {
    pub exposure: f64,
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
    pub temperature: f64,
    pub sharpness: f64,
    pub blur: f64,
    pub vignette: f64,
    pub grain: f64,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Geometry {
    /// Quarts de tour horaires (0…3).
    pub quarter: u8,
    pub flip_h: bool,
    pub flip_v: bool,
    /// Redressement en degrés, avec agrandissement pour masquer les coins.
    pub straighten: f64,
}

/// Zone gardée, en fractions (0…1) de l'image après rotation.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct CropRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum Frame {
    /// Cadre d'origine.
    Original,
    /// Recadrer : on garde la zone `rect` (déjà au bon ratio).
    Crop { rect: CropRect },
    /// Ajuster : toute l'image, dans un cadre au ratio `ratio` (largeur / hauteur), complété
    /// par un fond flou ou une couleur (`#rrggbb`).
    Fit { ratio: f64, color: Option<String> },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoEdit {
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub adjust: Adjust,
    #[serde(default)]
    pub sepia: f64,
    #[serde(default)]
    pub fade: f64,
    #[serde(default)]
    pub invert: bool,
    #[serde(default)]
    pub geometry: Geometry,
    pub frame: Frame,
    /// Petit côté visé (1080, 720…) ; jamais d'agrandissement.
    pub short_side: Option<u32>,
    pub speed: f64,
    /// 0…2 (200 %).
    pub volume: f64,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub normalize: bool,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
}

/// Ce que dit l'en-tête de la vidéo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VideoInfo {
    pub duration: f64,
    /// Dimensions affichées (rotation des métadonnées appliquée, comme ffmpeg et le lecteur).
    pub width: u32,
    pub height: u32,
    pub has_audio: bool,
    /// Fréquence du son (Hz), si connue.
    pub sample_rate: Option<u32>,
}

pub fn info(input: &Path) -> Result<VideoInfo, ConvertError> {
    let probe = ffmpeg::probe(input)?;
    let duration = ffmpeg::probe_duration(input)?;
    parse_info(&probe, duration).ok_or_else(|| ConvertError::Ffmpeg(format!("aucune image dans {}", input.display())))
}

fn parse_info(probe: &str, duration: f64) -> Option<VideoInfo> {
    let line = probe
        .lines()
        .find(|l| l.contains("Stream #") && l.contains("Video: ") && !l.contains("attached pic"))?;
    let (w, h) = line.split([',', ' ']).find_map(|tok| {
        let (w, h) = tok.split_once('x')?;
        Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?)).filter(|(w, h)| *w > 0 && *h > 0)
    })?;
    // `displaymatrix: rotation of -90.00 degrees` (ou `rotate : 90` dans les anciens ffmpeg).
    let rotation = probe.lines().find_map(|l| {
        let l = l.trim();
        if let Some(rest) = l.split("rotation of ").nth(1) {
            return rest.split_whitespace().next()?.parse::<f64>().ok();
        }
        l.strip_prefix("rotate")?.trim_start().strip_prefix(':')?.trim().parse::<f64>().ok()
    });
    let quarter = rotation.map_or(0, |r| ((r / 90.0).round() as i64).rem_euclid(4));
    let (width, height) = if quarter % 2 == 1 { (h, w) } else { (w, h) };
    let sample_rate = probe
        .lines()
        .find(|l| l.contains("Stream #") && l.contains("Audio: "))
        .and_then(|l| l.split(',').find_map(|p| p.trim().strip_suffix(" Hz")?.parse().ok()));
    Some(VideoInfo { duration, width, height, has_audio: probe.contains("Audio: "), sample_rate })
}

fn even(v: f64) -> u32 {
    ((v / 2.0).round() as u32 * 2).max(2)
}

/// Dimensions après chaque étape : rotation (quarts de tour), cadre, résolution.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Sizes {
    rotated: (u32, u32),
    framed: (u32, u32),
    output: (u32, u32),
}

fn sizes(info: &VideoInfo, edit: &VideoEdit) -> Sizes {
    let rotated = if edit.geometry.quarter % 2 == 1 { (info.height, info.width) } else { (info.width, info.height) };
    let (rw, rh) = (rotated.0 as f64, rotated.1 as f64);
    let framed = match &edit.frame {
        Frame::Original => (even(rw), even(rh)),
        Frame::Crop { rect } => (even(rect.w.clamp(0.01, 1.0) * rw), even(rect.h.clamp(0.01, 1.0) * rh)),
        Frame::Fit { ratio, .. } => {
            if rw / rh > *ratio { (even(rw), even(rw / ratio)) } else { (even(rh * ratio), even(rh)) }
        }
    };
    let (fw, fh) = (framed.0 as f64, framed.1 as f64);
    let output = match edit.short_side {
        Some(n) if (n as f64) < fw.min(fh) => {
            let k = n as f64 / fw.min(fh);
            (even(fw * k), even(fh * k))
        }
        _ => framed,
    };
    Sizes { rotated, framed, output }
}

/// Couleur d'un pixel, formules du shader `FINAL` (hors netteté, vignettage et grain).
fn color(rgb: [f64; 3], e: &VideoEdit) -> [f64; 3] {
    let a = &e.adjust;
    let gain = 2f64.powf(a.exposure / 100.0 * 1.5);
    let brightness = a.brightness / 100.0 * 0.3;
    let contrast = 1.0 + a.contrast / 100.0;
    let saturation = 1.0 + a.saturation / 100.0;
    let t = a.temperature / 100.0 * 0.12;
    let mut c = rgb.map(|v| (v * gain + brightness - 0.5) * contrast + 0.5);
    let l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    c = c.map(|v| l + (v - l) * saturation);
    c = [c[0] + t, c[1] + t * 0.2, c[2] - t];
    let sepia = [
        0.393 * c[0] + 0.769 * c[1] + 0.189 * c[2],
        0.349 * c[0] + 0.686 * c[1] + 0.168 * c[2],
        0.272 * c[0] + 0.534 * c[1] + 0.131 * c[2],
    ];
    for i in 0..3 {
        c[i] += (sepia[i] - c[i]) * e.sepia;
        c[i] = e.fade + c[i] * (1.0 - e.fade);
        if e.invert {
            c[i] = 1.0 - c[i];
        }
    }
    c.map(|v| v.clamp(0.0, 1.0))
}

fn has_color(e: &VideoEdit) -> bool {
    let a = &e.adjust;
    [a.exposure, a.brightness, a.contrast, a.saturation, a.temperature, e.sepia, e.fade].iter().any(|v| v.abs() > 1e-6)
        || e.invert
}

const LUT_SIZE: usize = 33;

/// LUT 3D au format `.cube` (rouge le plus rapide).
fn cube(e: &VideoEdit) -> String {
    let n = LUT_SIZE;
    let mut out = format!("LUT_3D_SIZE {n}\n");
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                let k = (n - 1) as f64;
                let [cr, cg, cb] = color([r as f64 / k, g as f64 / k, b as f64 / k], e);
                let _ = writeln!(out, "{cr:.6} {cg:.6} {cb:.6}");
            }
        }
    }
    out
}

/// Masque de vignettage (gris, 255 = intact), formule du shader, à la taille de sortie.
fn vignette_mask(strength: f64, (w, h): (u32, u32)) -> image::GrayImage {
    image::GrayImage::from_fn(w, h, |x, y| {
        let px = (x as f64 + 0.5) / w as f64 - 0.5;
        let py = (y as f64 + 0.5) / h as f64 - 0.5;
        let d = (px * px + py * py).sqrt() * std::f64::consts::SQRT_2;
        let t = ((d - 0.35) / 0.65).clamp(0.0, 1.0);
        let smooth = t * t * (3.0 - 2.0 * t);
        image::Luma([((1.0 - strength * smooth) * 255.0).round() as u8])
    })
}

/// `atempo` n'accepte qu'un facteur entre 0,5 et 2 par filtre : on les enchaîne.
fn atempo(speed: f64) -> String {
    let mut parts = Vec::new();
    let mut s = speed;
    while s > 2.0 + 1e-9 {
        parts.push("atempo=2".to_string());
        s /= 2.0;
    }
    while s < 0.5 - 1e-9 {
        parts.push("atempo=0.5".to_string());
        s /= 0.5;
    }
    if (s - 1.0).abs() > 1e-6 {
        parts.push(format!("atempo={s:.6}"));
    }
    parts.join(",")
}

/// Fichiers annexes (LUT, masque) écrits dans le dossier de travail de ffmpeg.
struct Extras {
    lut: Option<String>,
    mask: bool,
}

/// Graphe complet. Entrée 0 : la vidéo ; entrée 1 (si `extras.mask`) : le masque de vignettage.
fn filter_graph(info: &VideoInfo, e: &VideoEdit, segs: &[Segment], extras: &Extras) -> String {
    let s = sizes(info, e);
    let audio = info.has_audio && !e.mute;
    let mut g = String::new();

    // Découpe : chaque passage est extrait puis recollé.
    for (i, seg) in segs.iter().enumerate() {
        let _ = write!(g, "[0:v]trim=start={:.6}:end={:.6},setpts=PTS-STARTPTS[v{i}];", seg.start, seg.end);
    }
    let audio_graph = if audio { trim::filter_graph(segs, info.duration) } else { String::new() };
    if audio {
        // `trim::filter_graph` concatène le son en [out] : on le renomme [acat].
        g += &audio_graph.replace("[out]", "[acat]");
        g += ";";
    }
    for i in 0..segs.len() {
        let _ = write!(g, "[v{i}]");
    }
    let _ = write!(g, "concat=n={}:v=1:a=0[vcat];", segs.len());

    // Image : vitesse, puis les mêmes étapes que l'aperçu, en RVB.
    let mut v: Vec<String> = Vec::new();
    if (e.speed - 1.0).abs() > 1e-6 {
        v.push(format!("setpts=PTS/{:.6}", e.speed));
    }
    v.push("format=gbrp".into());
    let blur = e.adjust.blur / 100.0 * info.width.max(info.height) as f64 * 0.012;
    if blur > 0.05 {
        v.push(format!("gblur=sigma={blur:.3}"));
    }
    let sharp = (e.adjust.sharpness / 100.0 * 1.5 * 100.0).round() as i32;
    if sharp > 0 {
        let m = format!("0 -{sharp} 0 -{sharp} {} -{sharp} 0 -{sharp} 0", 100 + 4 * sharp);
        v.push(format!("convolution=0m='{m}':1m='{m}':2m='{m}':0rdiv=0.01:1rdiv=0.01:2rdiv=0.01"));
    }
    if let Some(lut) = &extras.lut {
        v.push(format!("lut3d=file={lut}:interp=tetrahedral"));
    }
    if e.geometry.flip_h {
        v.push("hflip".into());
    }
    if e.geometry.flip_v {
        v.push("vflip".into());
    }
    match e.geometry.quarter % 4 {
        1 => v.push("transpose=clock".into()),
        2 => v.push("hflip,vflip".into()),
        3 => v.push("transpose=cclock".into()),
        _ => {}
    }
    let (rw, rh) = s.rotated;
    if e.geometry.straighten.abs() > 1e-3 {
        let a = e.geometry.straighten.to_radians();
        let (c, sn) = (a.cos().abs(), a.sin().abs());
        let (w, h) = (rw as f64, rh as f64);
        let cover = ((w * c + h * sn) / w).max((w * sn + h * c) / h);
        v.push(format!(
            "rotate={a:.6}:ow=iw:oh=ih,scale={}:{},crop={rw}:{rh}",
            even(w * cover),
            even(h * cover)
        ));
    }
    let (fw, fh) = s.framed;
    let mut graph_tail = String::new();
    match &e.frame {
        Frame::Original => v.push(format!("scale={fw}:{fh}")),
        Frame::Crop { rect } => {
            let x = (rect.x.clamp(0.0, 1.0) * rw as f64).round() as u32;
            let y = (rect.y.clamp(0.0, 1.0) * rh as f64).round() as u32;
            v.push(format!("crop={fw}:{fh}:{}:{}", x.min(rw - fw.min(rw)), y.min(rh - fh.min(rh))));
        }
        Frame::Fit { color: Some(color), .. } => {
            let hex = color.trim_start_matches('#');
            let hex = if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) { hex } else { "000000" };
            v.push(format!(
                "scale={fw}:{fh}:force_original_aspect_ratio=decrease,pad={fw}:{fh}:(ow-iw)/2:(oh-ih)/2:color=0x{hex}"
            ));
        }
        Frame::Fit { color: None, .. } => {
            // Fond : la même image agrandie et floutée ; devant : l'image entière.
            let sigma = fw.min(fh) as f64 / 25.0;
            let _ = write!(
                graph_tail,
                "split[bg0][fg0];[bg0]scale={fw}:{fh}:force_original_aspect_ratio=increase,crop={fw}:{fh},gblur=sigma={sigma:.1}[bg];\
                 [fg0]scale={fw}:{fh}:force_original_aspect_ratio=decrease[fg];[bg][fg]overlay=(W-w)/2:(H-h)/2"
            );
        }
    }

    g += "[vcat]";
    g += &v.join(",");
    if !graph_tail.is_empty() {
        g += ",";
        g += &graph_tail;
    }
    let (ow, oh) = s.output;
    let mut post: Vec<String> = Vec::new();
    if s.output != s.framed {
        post.push(format!("scale={ow}:{oh}:flags=lanczos"));
    }
    post.push("setsar=1".into());
    if extras.mask {
        // `overlay` (fond flou) repasse en YUV : le masque se multiplie en RVB.
        post.push("format=gbrp".into());
        g += ",";
        g += &post.join(",");
        g += "[pre];[1:v]format=gbrp,scale=";
        let _ = write!(g, "{ow}:{oh}[mask];[pre][mask]blend=all_mode=multiply:shortest=1");
        post.clear();
    }
    let grain = e.adjust.grain / 100.0 * 0.25;
    if grain > 0.0 {
        post.push(format!("noise=alls={}:allf=t+u", (grain * 128.0).round() as u32));
    }
    post.push("format=yuv420p".into());
    g += ",";
    g += &post.join(",");
    g += "[vout]";

    // Son : vitesse sans effet « chipmunk », volume, normalisation, fondus.
    if audio {
        let out_len = segs.iter().map(|s| s.end - s.start).sum::<f64>() / e.speed;
        let mut a: Vec<String> = Vec::new();
        let tempo = atempo(e.speed);
        if !tempo.is_empty() {
            a.push(tempo);
        }
        if let Some(gain) = trim::volume_filter(e.volume) {
            a.push(gain);
        }
        if e.normalize {
            // loudnorm travaille (et sort) en 192 kHz : retour à la fréquence d'origine.
            a.push(format!("loudnorm=I=-16:TP=-1.5:LRA=11,aresample={}", info.sample_rate.unwrap_or(48_000)));
        }
        if e.fade_in > 0.0 {
            a.push(format!("afade=t=in:d={:.3}", e.fade_in.min(out_len)));
        }
        if e.fade_out > 0.0 {
            let d = e.fade_out.min(out_len);
            a.push(format!("afade=t=out:st={:.3}:d={d:.3}", (out_len - d).max(0.0)));
        }
        if a.is_empty() {
            a.push("anull".into());
        }
        let _ = write!(g, ";[acat]{}[aout]", a.join(","));
    }
    g
}

/// Encodage dans le conteneur d'origine, en bonne qualité (une édition n'est pas une compression).
fn codec_args(ext: &str, audio: bool) -> Vec<String> {
    let (video, sound): (&[&str], &[&str]) = match ext {
        "webm" => (
            &["-c:v", "libvpx-vp9", "-crf", "30", "-b:v", "0", "-row-mt", "1"],
            &["-c:a", "libopus", "-b:a", "160k"],
        ),
        "avi" => (&["-c:v", "mpeg4", "-q:v", "2"], &["-c:a", "libmp3lame", "-q:a", "2"]),
        "wmv" => (&["-c:v", "wmv2", "-b:v", "8M"], &["-c:a", "wmav2", "-b:a", "192k"]),
        "mpeg" => (&["-c:v", "mpeg2video", "-q:v", "2"], &["-c:a", "mp2", "-b:a", "224k"]),
        _ => (
            &["-c:v", "libx264", "-preset", "medium", "-crf", "18"],
            &["-c:a", "aac", "-b:a", "192k"],
        ),
    };
    let mut args: Vec<String> = video.iter().map(|s| s.to_string()).collect();
    if audio {
        args.extend(sound.iter().map(|s| s.to_string()));
    }
    if matches!(ext, "mp4" | "mov") {
        args.extend(["-movflags".into(), "+faststart".into()]);
    }
    args
}

fn work_dir() -> PathBuf {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("morph-video-{}-{n}", std::process::id()))
}

pub fn edit(input: &Path, e: &VideoEdit, on_progress: &mut dyn FnMut(f32)) -> Result<PathBuf, ConvertError> {
    if !input.is_file() {
        return Err(ConvertError::NotFound(input.display().to_string()));
    }
    if formats::kind_of(input) != Some(Kind::Video) {
        return Err(ConvertError::Unsupported(input.display().to_string()));
    }
    let ext = formats::ext_of(input).ok_or_else(|| ConvertError::Unsupported(input.display().to_string()))?;
    if !(0.25..=4.0).contains(&e.speed) {
        return Err(ConvertError::Edit(crate::i18n::tr("vitesse hors limites (0,25× à 4×)", "speed out of range (0.25× to 4×)").into()));
    }
    let info = info(input)?;
    let segs = trim::normalize(&e.segments, info.duration);
    if segs.is_empty() {
        return Err(ConvertError::Edit(crate::i18n::tr("aucun passage à garder", "no section to keep").into()));
    }

    let dir = work_dir();
    fs::create_dir_all(&dir)?;
    let result = (|| {
        let lut = if has_color(e) {
            fs::write(dir.join("colors.cube"), cube(e))?;
            Some("colors.cube".to_string())
        } else {
            None
        };
        let mask = e.adjust.vignette > 0.0;
        if mask {
            vignette_mask(e.adjust.vignette / 100.0, sizes(&info, e).output).save(dir.join("vignette.png"))?;
        }
        let extras = Extras { lut, mask };
        let audio = info.has_audio && !e.mute;

        let mut args: Vec<String> = Vec::new();
        if mask {
            args.extend(["-loop", "1", "-i", "vignette.png"].map(String::from));
        }
        args.extend(["-filter_complex".into(), filter_graph(&info, e, &segs, &extras), "-map".into(), "[vout]".into()]);
        if audio {
            args.extend(["-map".into(), "[aout]".into()]);
        }
        args.extend(codec_args(&ext, audio));

        let output = convert::suffixed_output(input, "Edited")?;
        let out_secs = segs.iter().map(|s| s.end - s.start).sum::<f64>() / e.speed;
        on_progress(0.0);
        ffmpeg::run_for(input, &args, &output, Some(out_secs), Some(&dir), on_progress)?;
        Ok(output)
    })();
    let _ = fs::remove_dir_all(&dir);
    result
}

/// Copie légère lisible par le webview (H.264 720p), pour les conteneurs qu'il ne lit pas
/// (AVI, WMV, FLV, MPEG…). Recréée à chaque ouverture, supprimée à la fermeture de Morph.
/// Chemin neuf (dossier temporaire du processus) pour une copie d'aperçu au format `ext`.
pub(crate) fn preview_path(ext: &str) -> Result<PathBuf, ConvertError> {
    let dir = std::env::temp_dir().join(format!("morph-preview-{}", std::process::id()));
    fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("{}.{ext}", work_dir().file_name().unwrap().to_string_lossy())))
}

pub fn preview(input: &Path) -> Result<PathBuf, ConvertError> {
    if formats::kind_of(input) != Some(Kind::Video) {
        return Err(ConvertError::Unsupported(input.display().to_string()));
    }
    let output = preview_path("mp4")?;
    let args = [
        "-vf", "scale='if(gte(iw,ih),-2,min(720,trunc(iw/2)*2))':'if(gte(iw,ih),min(720,trunc(ih/2)*2),-2)',setsar=1",
        "-c:v", "libx264", "-preset", "veryfast", "-crf", "26", "-pix_fmt", "yuv420p",
        "-c:a", "aac", "-b:a", "128k", "-movflags", "+faststart",
    ];
    ffmpeg::run(input, &args, &output, &mut |_| {})?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> VideoEdit {
        VideoEdit {
            segments: vec![Segment { start: 0.0, end: 10.0 }],
            adjust: Adjust::default(),
            sepia: 0.0,
            fade: 0.0,
            invert: false,
            geometry: Geometry::default(),
            frame: Frame::Original,
            short_side: None,
            speed: 1.0,
            volume: 1.0,
            mute: false,
            normalize: false,
            fade_in: 0.0,
            fade_out: 0.0,
        }
    }

    const INFO: VideoInfo = VideoInfo { duration: 10.0, width: 1920, height: 1080, has_audio: true, sample_rate: Some(48_000) };

    #[test]
    fn reads_rotated_phone_video() {
        let probe = "  Duration: 00:00:05.00, start: 0.000000, bitrate: 9000 kb/s\n  \
                     Stream #0:0[0x1](eng): Video: h264 (High) (avc1 / 0x31637661), yuv420p(tv, bt709), 1920x1080, 8000 kb/s, 30 fps\n    \
                     Side data:\n      displaymatrix: rotation of -90.00 degrees\n  \
                     Stream #0:1[0x2](eng): Audio: aac (LC), 48000 Hz, stereo, fltp, 192 kb/s";
        assert_eq!(
            parse_info(probe, 5.0),
            Some(VideoInfo { duration: 5.0, width: 1080, height: 1920, has_audio: true, sample_rate: Some(48_000) })
        );
    }

    #[test]
    fn identity_lut_is_neutral() {
        let e = base();
        assert!(!has_color(&e));
        let c = color([0.2, 0.5, 0.9], &e);
        assert!(c.iter().zip([0.2, 0.5, 0.9]).all(|(a, b)| (a - b).abs() < 1e-9));
        let mut inv = base();
        inv.invert = true;
        assert!((color([0.2, 0.5, 0.9], &inv)[0] - 0.8).abs() < 1e-9);
        assert_eq!(cube(&e).lines().count(), 1 + LUT_SIZE.pow(3));
    }

    #[test]
    fn computes_sizes() {
        let mut e = base();
        e.geometry.quarter = 1;
        e.frame = Frame::Fit { ratio: 1.0, color: None };
        e.short_side = Some(720);
        let s = sizes(&INFO, &e);
        assert_eq!(s.rotated, (1080, 1920));
        assert_eq!(s.framed, (1920, 1920));
        assert_eq!(s.output, (720, 720));
        // Jamais d'agrandissement.
        e.short_side = Some(4000);
        assert_eq!(sizes(&INFO, &e).output, (1920, 1920));
    }

    #[test]
    fn chains_atempo() {
        assert_eq!(atempo(1.0), "");
        assert_eq!(atempo(4.0), "atempo=2,atempo=2.000000");
        assert_eq!(atempo(0.25), "atempo=0.5,atempo=0.500000");
    }

    /// Nécessite ffmpeg (sidecar ou PATH) ; ignoré sinon.
    #[test]
    fn edits_a_video() {
        if ffmpeg::command().arg("-version").output().is_err() {
            eprintln!("ffmpeg absent : test ignoré");
            return;
        }
        let dir = std::env::temp_dir().join("morph-test-video");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let input = dir.join("clip.mp4");
        let status = ffmpeg::command()
            .args(["-loglevel", "error", "-f", "lavfi", "-i", "testsrc2=size=320x240:rate=25:duration=4"])
            .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=4", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest"])
            .arg(&input)
            .status()
            .unwrap();
        assert!(status.success());

        let mut e = base();
        e.segments = vec![Segment { start: 0.0, end: 1.0 }, Segment { start: 2.0, end: 4.0 }];
        e.adjust = Adjust { contrast: 30.0, saturation: -50.0, vignette: 60.0, grain: 20.0, sharpness: 30.0, blur: 10.0, ..Adjust::default() };
        e.sepia = 0.5;
        e.geometry = Geometry { quarter: 1, flip_h: true, flip_v: false, straighten: 5.0 };
        e.frame = Frame::Fit { ratio: 16.0 / 9.0, color: None };
        e.short_side = Some(180);
        e.speed = 2.0;
        e.volume = 1.5;
        e.normalize = true;
        e.fade_in = 0.3;
        e.fade_out = 0.3;

        let out = edit(&input, &e, &mut |_| {}).unwrap();
        assert_eq!(out, dir.join("clip Edited.mp4"));
        let got = info(&out).unwrap();
        assert!((got.duration - 1.5).abs() < 0.15, "durée {}", got.duration);
        assert_eq!((got.width, got.height), (320, 180));
        assert!(got.has_audio);
        assert_eq!(got.sample_rate, Some(44_100), "fréquence d'origine après normalisation");

        // Recadrage carré, sans le son.
        let mut e = base();
        e.segments = vec![Segment { start: 0.0, end: 4.0 }];
        e.frame = Frame::Crop { rect: CropRect { x: 0.25, y: 0.0, w: 0.5, h: 2.0 / 3.0 } };
        e.mute = true;
        let out = edit(&input, &e, &mut |_| {}).unwrap();
        let got = info(&out).unwrap();
        assert_eq!((got.width, got.height), (160, 160));
        assert!(!got.has_audio);
        fs::remove_dir_all(&dir).unwrap();
    }
}
