//! Détourage (retirer le fond) : modèle IS-Net (DIS, licence Apache-2.0) exécuté en local
//! par ONNX Runtime. Le modèle (~170 Mo) est téléchargé à la première utilisation, vérifié
//! par son empreinte, puis gardé dans les données de l'application ; rien n'est envoyé.
//! ONNX Runtime (bibliothèque officielle embarquée à côté de l'application) n'est chargé
//! qu'au premier détourage.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use image::{DynamicImage, GrayImage, ImageBuffer, Luma, RgbaImage, imageops};
use ort::session::Session;
use ort::value::Tensor;
use sha2::{Digest, Sha256};

use crate::convert::{self, ConvertError, output, raster};

const MODEL_URL: &str = "https://github.com/danielgatis/rembg/releases/download/v0.0.0/isnet-general-use.onnx";
const MODEL_FILE: &str = "isnet-general-use.onnx";
const MODEL_SIZE: u64 = 178_648_008;
const MODEL_SHA256: &str = "60920e99c45464f2ba57bee2ad08c919a52bbf852739e96947fbb4358c0d964a";

/// Côté de l'image vue par le modèle.
const SIDE: u32 = 1024;
/// Modèle libéré de la mémoire après ce délai sans détourage.
const IDLE_RELEASE: Duration = Duration::from_secs(90);
/// Part de la progression occupée par le téléchargement du modèle, s'il a lieu.
const DOWNLOAD_SHARE: f32 = 0.85;

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

fn model_path() -> Result<PathBuf, ConvertError> {
    MODELS_DIR
        .get()
        .map(|d| d.join(MODEL_FILE))
        .ok_or_else(|| ConvertError::Edit("dossier des modèles inconnu".into()))
}

/// Le modèle est déjà sur le disque (pas de téléchargement à prévoir).
pub fn model_ready() -> bool {
    model_path().is_ok_and(|p| fs::metadata(p).is_ok_and(|m| m.len() == MODEL_SIZE))
}

/// Télécharge le modèle s'il manque ; `on_progress` reçoit l'avancement (0…1).
fn ensure_model(on_progress: &mut dyn FnMut(f32)) -> Result<PathBuf, ConvertError> {
    let path = model_path()?;
    if model_ready() {
        return Ok(path);
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let partial = path.with_extension("onnx.part");
    let result = download(&partial, on_progress);
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

fn download(to: &Path, on_progress: &mut dyn FnMut(f32)) -> Result<(), ConvertError> {
    let failed = |e: &dyn std::fmt::Display| ConvertError::Download(e.to_string());
    let response = ureq::get(MODEL_URL).call().map_err(|e| failed(&e))?;
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
        on_progress((received as f32 / MODEL_SIZE as f32).min(1.0));
    }
    file.flush()?;
    let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if received != MODEL_SIZE || digest != MODEL_SHA256 {
        return Err(failed(&"fichier du modèle incomplet ou altéré"));
    }
    Ok(())
}

/// Modèle chargé, gardé tant qu'il sert (un lot n'est chargé qu'une fois).
struct Engine {
    session: Session,
    used: Instant,
}

static ENGINE: Mutex<Option<Engine>> = Mutex::new(None);
static RELEASER: AtomicBool = AtomicBool::new(false);

fn engine_error(e: impl std::fmt::Display) -> ConvertError {
    ConvertError::Edit(format!("détourage impossible : {e}"))
}

fn with_session<T>(model: &Path, f: impl FnOnce(&mut Session) -> Result<T, ConvertError>) -> Result<T, ConvertError> {
    let mut slot = ENGINE.lock().unwrap();
    if slot.is_none() {
        let session = Session::builder(environment()?).map_err(engine_error)?.commit_from_file(model).map_err(engine_error)?;
        *slot = Some(Engine { session, used: Instant::now() });
    }
    let engine = slot.as_mut().unwrap();
    let result = f(&mut engine.session);
    engine.used = Instant::now();
    drop(slot);
    schedule_release();
    result
}

/// Libère le modèle (~200 Mo en mémoire) quand il ne sert plus.
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

/// Masque du sujet (255 = garder), à la taille de l'image.
pub fn mask(img: &DynamicImage, on_progress: &mut dyn FnMut(f32)) -> Result<GrayImage, ConvertError> {
    let downloading = !model_ready();
    let model = ensure_model(&mut |p| on_progress(p * DOWNLOAD_SHARE))?;
    let base = if downloading { DOWNLOAD_SHARE } else { 0.0 };
    let step = |p: f32| base + (1.0 - base) * p;
    output::check_cancel()?;

    // Entrée du modèle : RVB 1024×1024, valeurs / max − 0,5 (prétraitement de rembg).
    let rgb = imageops::resize(&img.to_rgb8(), SIDE, SIDE, imageops::FilterType::Lanczos3);
    let max = rgb.as_raw().iter().copied().max().unwrap_or(255).max(1) as f32;
    let plane = (SIDE * SIDE) as usize;
    let mut data = vec![0f32; 3 * plane];
    for (i, px) in rgb.pixels().enumerate() {
        for c in 0..3 {
            data[c * plane + i] = px[c] as f32 / max - 0.5;
        }
    }
    on_progress(step(0.15));

    let pred = with_session(&model, |session| {
        on_progress(step(0.3));
        let input = Tensor::from_array(([1usize, 3, SIDE as usize, SIDE as usize], data)).map_err(engine_error)?;
        let outputs = session.run(ort::inputs![input]).map_err(engine_error)?;
        let (_, values) = outputs[0].try_extract_tensor::<f32>().map_err(engine_error)?;
        Ok(values.to_vec())
    })?;
    output::check_cancel()?;
    on_progress(step(0.85));

    // Sortie ramenée à 0…1, puis à la taille de l'image.
    let (lo, hi) = pred.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    let span = (hi - lo).max(1e-6);
    let small: GrayImage = ImageBuffer::from_fn(SIDE, SIDE, |x, y| {
        Luma([(((pred[(y * SIDE + x) as usize] - lo) / span) * 255.0).round() as u8])
    });
    Ok(imageops::resize(&small, img.width(), img.height(), imageops::FilterType::Lanczos3))
}

/// Applique le masque comme transparence (en gardant celle de l'image).
pub fn apply_mask(img: &DynamicImage, mask: &GrayImage) -> RgbaImage {
    let mut rgba = img.to_rgba8();
    for (px, m) in rgba.pixels_mut().zip(mask.pixels()) {
        px[3] = ((px[3] as u16 * m[0] as u16) / 255) as u8;
    }
    rgba
}

/// Image détourée, en PNG (compression rapide), pour l'aperçu de l'éditeur d'image.
pub fn cutout_png(input: &Path, on_progress: &mut dyn FnMut(f32)) -> Result<Vec<u8>, ConvertError> {
    use image::ImageEncoder;
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    let img = raster::load(input)?;
    let mask = mask(&img, on_progress)?;
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

/// Retire le fond d'une image : `nom Cutout.png` (transparent) à côté de l'original.
pub fn remove_background(input: &Path, on_progress: &mut dyn FnMut(f32)) -> Result<PathBuf, ConvertError> {
    let img = raster::load(input)?;
    let mask = mask(&img, on_progress)?;
    let output = convert::suffixed_output_as(input, "Cutout", "png")?;
    raster::save(&DynamicImage::ImageRgba8(apply_mask(&img, &mask)), &output, "png")?;
    on_progress(1.0);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Avec le modèle sous la main (`MORPH_ISNET=chemin/isnet-general-use.onnx`) : un carré
    /// net sur fond uni garde son centre et perd ses coins.
    #[test]
    fn removes_plain_background() {
        let Ok(model) = std::env::var("MORPH_ISNET") else {
            eprintln!("MORPH_ISNET absent : test du détourage ignoré");
            return;
        };
        set_models_dir(Path::new(&model).parent().unwrap().to_path_buf());
        set_runtime(Path::new(env!("CARGO_MANIFEST_DIR")).join("onnxruntime").join(Path::new(RUNTIME_LIB).file_name().unwrap()));
        let img = DynamicImage::ImageRgb8(ImageBuffer::from_fn(400, 300, |x, y| {
            if (120..280).contains(&x) && (80..220).contains(&y) { image::Rgb([220, 40, 30]) } else { image::Rgb([240, 240, 240]) }
        }));
        let mask = mask(&img, &mut |_| {}).unwrap();
        assert_eq!(mask.dimensions(), (400, 300));
        assert!(mask.get_pixel(200, 150)[0] > 200, "sujet gardé");
        assert!(mask.get_pixel(5, 5)[0] < 40, "fond retiré");
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
        let out = remove_background(&input, &mut |p| {
            assert!(p >= last, "progression croissante");
            last = p;
        })
        .unwrap();
        eprintln!("détourage avec téléchargement : {:?}", started.elapsed());
        assert!(model_ready());
        assert_eq!(out.file_name().unwrap().to_string_lossy(), format!("{} Cutout.png", input.file_stem().unwrap().to_string_lossy()));
        let started = Instant::now();
        remove_background(&input, &mut |_| {}).unwrap();
        eprintln!("détourage suivant : {:?}", started.elapsed());
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
