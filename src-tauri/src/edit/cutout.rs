//! Détourage (retirer le fond) exécuté en local par ONNX Runtime, au choix (Réglages) :
//! « Efficacité », IS-Net (DIS, Apache-2.0, 170 Mo, quelques secondes) ou « Qualité »,
//! BiRefNet (MIT, 970 Mo, plus lent mais bien plus précis sur les cheveux et les bords).
//! Le modèle est téléchargé à sa première utilisation, vérifié par son empreinte, puis
//! gardé dans les données de l'application ; rien n'est envoyé.
//! ONNX Runtime (bibliothèque officielle embarquée à côté de l'application) n'est chargé
//! qu'au premier détourage.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use image::{DynamicImage, GrayImage, ImageBuffer, Luma, RgbaImage, imageops};
use ort::session::Session;
use ort::value::Tensor;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::convert::formats::Kind;
use crate::convert::{self, ConvertError, ffmpeg, formats, output, raster, ticking};

/// Modèle choisi dans les Réglages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    /// IS-Net : rapide et léger.
    #[default]
    Fast,
    /// BiRefNet : plus lent, bien meilleur résultat.
    Best,
}

/// Un modèle et son prétraitement (celui de rembg, dont viennent les fichiers ONNX).
struct Model {
    url: &'static str,
    file: &'static str,
    size: u64,
    sha256: &'static str,
    /// Entrée : (valeur / max de l'image − moyenne) / écart-type, par canal.
    mean: [f32; 3],
    std: [f32; 3],
    /// Sortie en logits (à passer dans une sigmoïde) plutôt qu'en probabilités.
    logits: bool,
    /// Durées typiques (chargement du modèle, calcul) en millisecondes, pour faire avancer
    /// la progression pendant ces étapes muettes ; remplacées par les durées mesurées.
    load_ms: u64,
    run_ms: u64,
}

const FAST: Model = Model {
    url: "https://github.com/danielgatis/rembg/releases/download/v0.0.0/isnet-general-use.onnx",
    file: "isnet-general-use.onnx",
    size: 178_648_008,
    sha256: "60920e99c45464f2ba57bee2ad08c919a52bbf852739e96947fbb4358c0d964a",
    mean: [0.5; 3],
    std: [1.0; 3],
    logits: false,
    load_ms: 800,
    run_ms: 700,
};

const BEST: Model = Model {
    url: "https://github.com/danielgatis/rembg/releases/download/v0.0.0/BiRefNet-general-epoch_244.onnx",
    file: "birefnet-general.onnx",
    size: 972_666_916,
    sha256: "58f621f00f5d756097615970a88a791584600dcf7c45b18a0a6267535a1ebd3c",
    mean: [0.485, 0.456, 0.406],
    std: [0.229, 0.224, 0.225],
    logits: true,
    load_ms: 5_000,
    run_ms: 14_000,
};

/// Durées mesurées au dernier détourage (0 : pas encore), par modèle : [chargement, calcul].
static MEASURED: [[AtomicU64; 2]; 2] = [[AtomicU64::new(0), AtomicU64::new(0)], [AtomicU64::new(0), AtomicU64::new(0)]];

impl Quality {
    /// Durée attendue d'une étape (0 : chargement, 1 : calcul), mesurée ou typique.
    fn expected(self, stage: usize) -> Duration {
        let measured = MEASURED[self as usize][stage].load(Ordering::Relaxed);
        let typical = if stage == 0 { self.model().load_ms } else { self.model().run_ms };
        Duration::from_millis(if measured > 0 { measured } else { typical })
    }

    fn measured(self, stage: usize, took: Duration) {
        MEASURED[self as usize][stage].store(took.as_millis() as u64, Ordering::Relaxed);
    }

    fn model(self) -> &'static Model {
        match self {
            Quality::Fast => &FAST,
            Quality::Best => &BEST,
        }
    }

    /// Poids du modèle à télécharger, pour les messages.
    pub fn download_size(self) -> &'static str {
        match self {
            Quality::Fast => "170 Mo",
            Quality::Best => "970 Mo",
        }
    }
}

/// Côté de l'image vue par le modèle.
const SIDE: u32 = 1024;
/// Modèle libéré de la mémoire après ce délai sans détourage.
const IDLE_RELEASE: Duration = Duration::from_secs(90);
/// Part de la progression occupée par le téléchargement du modèle, s'il a lieu.
const DOWNLOAD_SHARE: f32 = 0.85;
/// Part occupée par la lecture de l'image.
const IMAGE_SHARE: f32 = 0.04;

/// Bibliothèque ONNX Runtime, relative aux ressources de l'application. Sous Windows, à
/// côté de l'exécutable : le runtime Visual C++ qu'elle importe y est aussi.
#[cfg(windows)]
pub const RUNTIME_LIB: &str = "onnxruntime.dll";
#[cfg(not(windows))]
pub const RUNTIME_LIB: &str = "onnxruntime/libonnxruntime.so";

static MODELS_DIR: OnceLock<PathBuf> = OnceLock::new();
static RUNTIME: OnceLock<PathBuf> = OnceLock::new();

/// Dossier des modèles (données locales de l'application), fixé au démarrage.
pub fn set_models_dir(dir: PathBuf) {
    let _ = MODELS_DIR.set(dir);
}

/// Chemin de la bibliothèque ONNX Runtime, fixé au démarrage.
pub fn set_runtime(path: PathBuf) {
    let _ = RUNTIME.set(path);
}

/// Environnement ONNX Runtime : la bibliothèque n'est chargée qu'une fois.
fn environment() -> Result<&'static ort::environment::Environment, ConvertError> {
    static ENV: OnceLock<ort::environment::Environment> = OnceLock::new();
    if let Some(env) = ENV.get() {
        return Ok(env);
    }
    let lib = RUNTIME.get().ok_or_else(|| engine_error("ONNX Runtime introuvable"))?;
    if !lib.exists() {
        return Err(engine_error(format!("ONNX Runtime absent ({})", lib.display())));
    }
    let env = ort::init_from(lib).map_err(engine_error)?.with_name("morph").build().map_err(engine_error)?;
    Ok(ENV.get_or_init(|| env))
}

fn model_path(model: &Model) -> Result<PathBuf, ConvertError> {
    MODELS_DIR
        .get()
        .map(|d| d.join(model.file))
        .ok_or_else(|| ConvertError::Edit("dossier des modèles inconnu".into()))
}

/// Le modèle est déjà sur le disque (pas de téléchargement à prévoir).
pub fn model_ready(quality: Quality) -> bool {
    let model = quality.model();
    model_path(model).is_ok_and(|p| fs::metadata(p).is_ok_and(|m| m.len() == model.size))
}

/// Télécharge le modèle s'il manque ; `on_progress` reçoit l'avancement (0…1).
fn ensure_model(quality: Quality, on_progress: &mut dyn FnMut(f32)) -> Result<PathBuf, ConvertError> {
    let model = quality.model();
    let path = model_path(model)?;
    if model_ready(quality) {
        return Ok(path);
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let partial = path.with_extension("onnx.part");
    let result = download(model, &partial, on_progress);
    match result {
        Ok(()) => {
            fs::rename(&partial, &path)?;
            Ok(path)
        }
        Err(e) => {
            let _ = fs::remove_file(&partial);
            Err(e)
        }
    }
}

fn download(model: &Model, to: &Path, on_progress: &mut dyn FnMut(f32)) -> Result<(), ConvertError> {
    let failed = |e: &dyn std::fmt::Display| ConvertError::Download(e.to_string());
    let response = ureq::get(model.url).call().map_err(|e| failed(&e))?;
    let mut body = response.into_body().into_reader();
    let mut file = File::create(to)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    let mut received = 0u64;
    loop {
        output::check_cancel()?;
        let n = body.read(&mut buf).map_err(|e| failed(&e))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        hasher.update(&buf[..n]);
        received += n as u64;
        on_progress((received as f32 / model.size as f32).min(1.0));
    }
    file.flush()?;
    let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if received != model.size || digest != model.sha256 {
        return Err(failed(&crate::i18n::tr("fichier du modèle incomplet ou altéré", "model file incomplete or corrupted")));
    }
    Ok(())
}

/// Modèle chargé, gardé tant qu'il sert (un lot n'est chargé qu'une fois).
struct Engine {
    path: PathBuf,
    session: Session,
    used: Instant,
}

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);
static RELEASER: AtomicBool = AtomicBool::new(false);

fn engine_error(e: impl std::fmt::Display) -> ConvertError {
    ConvertError::Edit(crate::i18n::tr(format!("détourage impossible : {e}"), format!("background removal failed: {e}")))
}

/// Libère le modèle (de 200 Mo à 2 Go en mémoire) quand il ne sert plus.
fn schedule_release() {
    if RELEASER.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(Duration::from_secs(10));
            let mut slot = ENGINE.lock().unwrap();
            if slot.as_ref().is_none_or(|e| e.used.elapsed() >= IDLE_RELEASE) {
                *slot = None;
                RELEASER.store(false, Ordering::SeqCst);
                return;
            }
        }
    });
}

/// Entrée du modèle : RVB 1024×1024, normalisé comme le fait rembg.
fn input_tensor(img: &DynamicImage, model: &Model) -> Vec<f32> {
    let rgb = imageops::resize(&img.to_rgb8(), SIDE, SIDE, imageops::FilterType::Lanczos3);
    let max = rgb.as_raw().iter().copied().max().unwrap_or(255).max(1) as f32;
    let plane = (SIDE * SIDE) as usize;
    let mut data = vec![0f32; 3 * plane];
    for (i, px) in rgb.pixels().enumerate() {
        for c in 0..3 {
            data[c * plane + i] = (px[c] as f32 / max - model.mean[c]) / model.std[c];
        }
    }
    data
}

/// Modèle chargé (gardé en mémoire entre deux détourages ; un seul à la fois). Le
/// chargement fait avancer `on_progress` de 0 à 1.
fn load_engine(
    path: &Path,
    quality: Quality,
    on_progress: &mut dyn FnMut(f32),
) -> Result<MutexGuard<'static, Option<Engine>>, ConvertError> {
    let mut slot = ENGINE.lock().unwrap();
    if slot.as_ref().is_none_or(|e| e.path != path) {
        *slot = None;
        let env = environment()?;
        let (session, took) = ticking(quality.expected(0), 0.0, 1.0, on_progress, || {
            Session::builder(env)?.commit_from_file(path)
        });
        quality.measured(0, took);
        *slot = Some(Engine { path: path.to_path_buf(), session: session.map_err(engine_error)?, used: Instant::now() });
    }
    Ok(slot)
}

/// Passe l'entrée dans le modèle.
fn infer(session: &mut Session, data: Vec<f32>) -> Result<Vec<f32>, ConvertError> {
    let input = Tensor::from_array(([1usize, 3, SIDE as usize, SIDE as usize], data)).map_err(engine_error)?;
    let outputs = session.run(ort::inputs![input]).map_err(engine_error)?;
    let (_, values) = outputs[0].try_extract_tensor::<f32>().map_err(engine_error)?;
    Ok(values.to_vec())
}

/// Sortie ramenée à 0…1, puis à la taille de l'image.
fn output_mask(mut pred: Vec<f32>, model: &Model, width: u32, height: u32) -> GrayImage {
    if model.logits {
        pred.iter_mut().for_each(|v| *v = 1.0 / (1.0 + (-*v).exp()));
    }
    let (lo, hi) = pred.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    let span = (hi - lo).max(1e-6);
    let small: GrayImage = ImageBuffer::from_fn(SIDE, SIDE, |x, y| {
        Luma([(((pred[(y * SIDE + x) as usize] - lo) / span) * 255.0).round() as u8])
    });
    imageops::resize(&small, width, height, imageops::FilterType::Lanczos3)
}

/// Masque du sujet (255 = garder), à la taille de l'image.
pub fn mask(img: &DynamicImage, quality: Quality, on_progress: &mut dyn FnMut(f32)) -> Result<GrayImage, ConvertError> {
    let downloading = !model_ready(quality);
    let path = ensure_model(quality, &mut |p| on_progress(IMAGE_SHARE + p * (DOWNLOAD_SHARE - IMAGE_SHARE)))?;
    let model = quality.model();
    let base = if downloading { DOWNLOAD_SHARE } else { IMAGE_SHARE };
    let step = |p: f32| base + (1.0 - base) * p;
    output::check_cancel()?;

    let data = input_tensor(img, model);
    on_progress(step(0.05));
    let mut slot = load_engine(&path, quality, &mut |p| on_progress(step(0.05 + 0.25 * p)))?;
    let engine = slot.as_mut().unwrap();
    let session = &mut engine.session;
    let (pred, took) = ticking(quality.expected(1), step(0.3), step(0.95), on_progress, move || infer(session, data));
    quality.measured(1, took);
    engine.used = Instant::now();
    drop(slot);
    schedule_release();
    let pred = pred?;
    output::check_cancel()?;
    Ok(output_mask(pred, model, img.width(), img.height()))
}

/// Applique le masque comme transparence (en gardant celle de l'image).
pub fn apply_mask(img: &DynamicImage, mask: &GrayImage) -> RgbaImage {
    let mut rgba = img.to_rgba8();
    for (px, m) in rgba.pixels_mut().zip(mask.pixels()) {
        px[3] = ((px[3] as u16 * m[0] as u16) / 255) as u8;
    }
    rgba
}

/// Lit l'image à détourer ; la progression avance dès le début.
fn load(input: &Path, on_progress: &mut dyn FnMut(f32)) -> Result<DynamicImage, ConvertError> {
    on_progress(0.0);
    ticking(Duration::from_millis(600), 0.0, IMAGE_SHARE, on_progress, || raster::load(input)).0
}

/// Image détourée, en PNG (compression rapide), pour l'aperçu de l'éditeur d'image.
pub fn cutout_png(input: &Path, quality: Quality, on_progress: &mut dyn FnMut(f32)) -> Result<Vec<u8>, ConvertError> {
    use image::ImageEncoder;
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let img = load(input, on_progress)?;
    let mask = mask(&img, quality, on_progress)?;
    let rgba = apply_mask(&img, &mask);
    let mut png = Vec::new();
    PngEncoder::new_with_quality(&mut png, CompressionType::Fast, FilterType::Adaptive).write_image(
        rgba.as_raw(),
        rgba.width(),
        rgba.height(),
        image::ExtendedColorType::Rgba8,
    )?;
    on_progress(1.0);
    Ok(png)
}

/// Retire le fond d'une image (`nom Cutout.png`) ou d'une vidéo (`nom Cutout.webm`),
/// à côté de l'original.
pub fn remove_background(input: &Path, quality: Quality, on_progress: &mut dyn FnMut(f32)) -> Result<PathBuf, ConvertError> {
    if formats::format_of(input).is_some_and(|f| f.kind == Kind::Video) {
        return remove_video_background(input, quality, on_progress);
    }
    let img = load(input, on_progress)?;
    let mask = mask(&img, quality, on_progress)?;
    let output = convert::suffixed_output_as(input, "Cutout", "png")?;
    raster::save(&DynamicImage::ImageRgba8(apply_mask(&img, &mask)), &output, "png")?;
    on_progress(1.0);
    Ok(output)
}

/// Vidéo décodée par ffmpeg, telle qu'il l'annonce avant les premières images.
#[derive(Debug, PartialEq)]
struct Stream {
    width: u32,
    height: u32,
    /// Images par seconde, telles qu'écrites par ffmpeg (« 29.97 »).
    fps: String,
    /// Durée de l'entrée en secondes (0 : inconnue).
    secs: f64,
}

impl Stream {
    /// Lit l'en-tête de ffmpeg : durée de l'entrée, taille et cadence de la sortie
    /// `rawvideo` (rotation appliquée).
    fn parse(header: &[String]) -> Option<Stream> {
        let secs = header.iter().find_map(|l| ffmpeg::parse_duration(l)).map_or(0.0, |us| us as f64 / 1e6);
        let after = header.iter().position(|l| l.starts_with("Output #0"))?;
        let line = header[after..].iter().find(|l| l.contains("Video: rawvideo"))?;
        let parts: Vec<&str> = line.split([',', ' ']).filter(|p| !p.is_empty()).collect();
        let (width, height) = parts.iter().find_map(|p| {
            let (w, h) = p.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })?;
        let fps = parts.windows(2).find(|w| w[1] == "fps").map_or("30", |w| w[0]).to_string();
        Some(Stream { width, height, fps, secs })
    }

    /// Nombre d'images attendu, pour la progression.
    fn frames(&self) -> f64 {
        (self.secs * self.fps.parse::<f64>().unwrap_or(30.0)).max(1.0)
    }
}

/// Lit le journal d'un ffmpeg à part ; ses dernières lignes servent au message d'erreur.
fn drain(stderr: impl Read + Send + 'static) -> std::thread::JoinHandle<String> {
    std::thread::spawn(move || {
        let lines: Vec<String> = BufReader::new(stderr).lines().map_while(Result::ok).collect();
        lines[lines.len().saturating_sub(10)..].join("\n")
    })
}

/// Vidéo détourée, image par image : `nom Cutout.webm` (VP9 avec transparence, son
/// d'origine en Opus). Un ffmpeg décode la vidéo en images brutes, le modèle calcule
/// chaque masque, un second ffmpeg encode le résultat.
fn remove_video_background(input: &Path, quality: Quality, on_progress: &mut dyn FnMut(f32)) -> Result<PathBuf, ConvertError> {
    on_progress(0.0);
    let downloading = !model_ready(quality);
    let path = ensure_model(quality, &mut |p| on_progress(p * DOWNLOAD_SHARE))?;
    let base = if downloading { DOWNLOAD_SHARE } else { 0.0 };
    let step = |p: f32| base + (1.0 - base) * p;
    let model = quality.model();
    output::check_cancel()?;

    // Décodage : images RVB brutes sur la sortie standard.
    let mut decoder = ffmpeg::command()
        .args(["-loglevel", "info", "-i"])
        .arg(input)
        .args(["-map", "0:v:0", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(ffmpeg::spawn_error)?;
    let mut stderr = BufReader::new(decoder.stderr.take().expect("stderr piped"));
    let mut header = Vec::new();
    let mut line = String::new();
    let stream = loop {
        line.clear();
        if stderr.read_line(&mut line)? == 0 {
            let _ = decoder.wait();
            return Err(ConvertError::Ffmpeg(header.join("\n")));
        }
        header.push(line.trim_end().to_string());
        if header.last().is_some_and(|l| l.contains("Video: rawvideo")) {
            match Stream::parse(&header) {
                Some(stream) => break stream,
                None => {
                    let _ = decoder.kill();
                    return Err(ConvertError::Ffmpeg(header.join("\n")));
                }
            }
        }
    };
    let decoder_log = drain(stderr.into_inner());
    let frames = stream.frames();

    let output = match convert::suffixed_output_as(input, "Cutout", "webm") {
        Ok(output) => output,
        Err(e) => {
            let _ = decoder.kill();
            return Err(e);
        }
    };
    let size = format!("{}x{}", stream.width, stream.height);
    let encoder = ffmpeg::command()
        .args(["-loglevel", "error", "-n", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &size, "-framerate", &stream.fps, "-i", "-", "-i"])
        .arg(input)
        .args(["-map", "0:v", "-map", "1:a:0?", "-c:v", "libvpx-vp9", "-pix_fmt", "yuva420p", "-crf", "30", "-b:v", "0"])
        .args(["-row-mt", "1", "-deadline", "good", "-cpu-used", "4", "-c:a", "libopus", "-b:a", "128k", "-shortest"])
        .arg(&output)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn();
    let mut encoder = match encoder {
        Ok(encoder) => encoder,
        Err(e) => {
            let _ = decoder.kill();
            return Err(ffmpeg::spawn_error(e));
        }
    };
    let encoder_log = drain(encoder.stderr.take().expect("stderr piped"));

    let mut reader = decoder.stdout.take().expect("stdout piped");
    let mut writer = encoder.stdin.take().expect("stdin piped");
    let result = load_engine(&path, quality, &mut |p| on_progress(step(0.03 * p))).and_then(|mut slot| {
        let engine = slot.as_mut().unwrap();
        let mut rgb = vec![0u8; (stream.width * stream.height * 3) as usize];
        let mut done = 0f64;
        let result = loop {
            if let Err(e) = output::check_cancel() {
                break Err(e);
            }
            match reader.read_exact(&mut rgb) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break Ok(()),
                Err(e) => break Err(e.into()),
            }
            let frame = DynamicImage::ImageRgb8(ImageBuffer::from_raw(stream.width, stream.height, rgb.clone()).expect("taille de l'image"));
            let pred = match infer(&mut engine.session, input_tensor(&frame, model)) {
                Ok(pred) => pred,
                Err(e) => break Err(e),
            };
            let mask = output_mask(pred, model, stream.width, stream.height);
            // Écriture refusée : l'encodeur s'est arrêté, son journal dira pourquoi.
            if writer.write_all(apply_mask(&frame, &mask).as_raw()).is_err() {
                break Ok(());
            }
            engine.used = Instant::now();
            done += 1.0;
            on_progress(step(0.03 + 0.96 * (done / frames).min(1.0) as f32));
        };
        engine.used = Instant::now();
        result
    });
    schedule_release();
    drop(writer);
    drop(reader);

    if result.is_err() {
        let _ = decoder.kill();
        let _ = encoder.kill();
    }
    let decoded = decoder.wait()?;
    let encoded = encoder.wait()?;
    let decoder_log = decoder_log.join().unwrap_or_default();
    let encoder_log = encoder_log.join().unwrap_or_default();
    let result = result.and_then(|()| match (decoded.success(), encoded.success()) {
        (true, true) => Ok(()),
        (false, _) => Err(ConvertError::Ffmpeg(decoder_log)),
        (_, false) => Err(ConvertError::Ffmpeg(encoder_log)),
    });
    match result {
        Ok(()) => {
            on_progress(1.0);
            Ok(output)
        }
        Err(e) => {
            let _ = fs::remove_file(&output);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Avec les modèles sous la main (`MORPH_MODELS=dossier` contenant `isnet-general-use.onnx`
    /// et/ou `birefnet-general.onnx`) : un carré net sur fond uni garde son centre et perd
    /// ses coins, avec chaque modèle présent.
    #[test]
    fn removes_plain_background() {
        let Ok(dir) = std::env::var("MORPH_MODELS") else {
            eprintln!("MORPH_MODELS absent : test du détourage ignoré");
            return;
        };
        set_models_dir(PathBuf::from(dir));
        set_runtime(Path::new(env!("CARGO_MANIFEST_DIR")).join("onnxruntime").join(Path::new(RUNTIME_LIB).file_name().unwrap()));
        let img = DynamicImage::ImageRgb8(ImageBuffer::from_fn(400, 300, |x, y| {
            if (120..280).contains(&x) && (80..220).contains(&y) { image::Rgb([220, 40, 30]) } else { image::Rgb([240, 240, 240]) }
        }));
        for quality in [Quality::Fast, Quality::Best].into_iter().filter(|q| model_ready(*q)) {
            let started = Instant::now();
            let mask = mask(&img, quality, &mut |_| {}).unwrap();
            eprintln!("{quality:?} : {:?}", started.elapsed());
            assert_eq!(mask.dimensions(), (400, 300));
            assert!(mask.get_pixel(200, 150)[0] > 200, "{quality:?} : sujet gardé");
            assert!(mask.get_pixel(5, 5)[0] < 40, "{quality:?} : fond retiré");
        }
    }

    /// Bout en bout, réseau compris (lancé à la main : `cargo test -- --ignored`) : télécharge
    /// le modèle dans un dossier vide, le vérifie, puis détoure `MORPH_CUTOUT_IMAGE`.
    #[test]
    #[ignore]
    fn downloads_model_then_cuts_out() {
        let dir = std::env::temp_dir().join("morph-test-cutout");
        let _ = fs::remove_dir_all(&dir);
        set_models_dir(dir.join("models"));
        set_runtime(Path::new(env!("CARGO_MANIFEST_DIR")).join("onnxruntime").join(Path::new(RUNTIME_LIB).file_name().unwrap()));
        let image = PathBuf::from(std::env::var("MORPH_CUTOUT_IMAGE").expect("MORPH_CUTOUT_IMAGE"));
        let input = dir.join(image.file_name().unwrap());
        fs::create_dir_all(&dir).unwrap();
        fs::copy(&image, &input).unwrap();
        let mut last = 0.0;
        let started = Instant::now();
        let out = remove_background(&input, Quality::Fast, &mut |p| {
            assert!(p >= last, "progression croissante");
            last = p;
        })
        .unwrap();
        eprintln!("détourage avec téléchargement : {:?}", started.elapsed());
        assert!(model_ready(Quality::Fast));
        assert_eq!(out.file_name().unwrap().to_string_lossy(), format!("{} Cutout.png", input.file_stem().unwrap().to_string_lossy()));
        let started = Instant::now();
        remove_background(&input, Quality::Fast, &mut |_| {}).unwrap();
        eprintln!("détourage suivant : {:?}", started.elapsed());
    }

    #[test]
    fn reads_decoded_stream() {
        let header: Vec<String> = [
            "Input #0, mov,mp4,m4a,3gp,3g2,mj2, from 'phone.mp4':",
            "  Duration: 00:00:10.50, start: 0.000000, bitrate: 9000 kb/s",
            "  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p, 1920x1080, 29.97 fps",
            "Output #0, rawvideo, to 'pipe:':",
            "  Stream #0:0(und): Video: rawvideo (RGB[24] / 0x18424752), rgb24(pc, gbr/bt709/bt709, progressive), 1080x1920, q=2-31, 1492992 kb/s, 29.97 fps, 29.97 tbn",
        ]
        .map(String::from)
        .to_vec();
        let stream = Stream::parse(&header).unwrap();
        assert_eq!(stream, Stream { width: 1080, height: 1920, fps: "29.97".into(), secs: 10.5 });
        assert_eq!(stream.frames().round(), 315.0);
    }

    #[test]
    fn mask_keeps_existing_transparency() {
        let img = DynamicImage::ImageRgba8(ImageBuffer::from_pixel(2, 1, image::Rgba([1, 2, 3, 128])));
        let mask = GrayImage::from_raw(2, 1, vec![255, 0]).unwrap();
        let out = apply_mask(&img, &mask);
        assert_eq!(out.get_pixel(0, 0)[3], 128);
        assert_eq!(out.get_pixel(1, 0)[3], 0);
    }
}

