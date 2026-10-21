//! PDF : pages → images et fusion par pdfium (bibliothèque officielle de Chromium, chargée à
//! la demande), images → PDF et compression par lopdf.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use image::{DynamicImage, GenericImageView};
use lopdf::{Document, Object, Stream, dictionary};
use pdfium_render::prelude::*;

use super::{ConvertError, output, raster};

#[cfg(windows)]
pub const LIBRARY: &str = "pdfium.dll";
#[cfg(not(windows))]
pub const LIBRARY: &str = "pdfium/libpdfium.so";

static LIBRARY_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Chemin de la bibliothèque pdfium, fixé au démarrage.
pub fn set_library(path: PathBuf) {
    let _ = LIBRARY_PATH.set(path);
}

pub fn library_present() -> bool {
    LIBRARY_PATH.get().is_some_and(|p| p.is_file())
}

/// Résolution des pages rendues en images.
const RENDER_DPI: f32 = 200.0;
/// Plus grand côté d'une page rendue (affiches, plans…).
const RENDER_MAX_SIDE: f32 = 8000.0;
/// Page des images assemblées : A4, en points (1/72 de pouce).
const A4: (f32, f32) = (595.0, 842.0);
const PHOTO_QUALITY: u8 = 90;

fn pdf_error(e: impl std::fmt::Display) -> ConvertError {
    let text = e.to_string();
    if text.contains("PasswordError") || text.contains("password") {
        ConvertError::Document("PDF protégé par un mot de passe".into())
    } else {
        ConvertError::Document(format!("PDF illisible ({text})"))
    }
}

/// Exécute `f` avec pdfium, chargé pour l'occasion (une seule instance à la fois).
fn with_pdfium<T>(f: impl FnOnce(&Pdfium) -> Result<T, ConvertError>) -> Result<T, ConvertError> {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    let _guard = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let path = LIBRARY_PATH.get().filter(|p| p.is_file()).ok_or(ConvertError::EngineMissing(
        "Le moteur PDF (pdfium) est introuvable : réinstalle Morph.",
    ))?;
    let bindings = Pdfium::bind_to_library(path).map_err(pdf_error)?;
    f(&Pdfium::new(bindings))
}

/// Pages d'un PDF en images (`ext` : png ou jpg). Une seule page : `nom.ext` ; sinon un
/// dossier `nom` avec `nom 01.ext`, `nom 02.ext`… Renvoie le fichier ou le dossier créé.
pub fn render_pages(input: &Path, ext: &str, on_progress: &mut dyn FnMut(f32)) -> Result<PathBuf, ConvertError> {
    with_pdfium(|pdfium| {
        let doc = pdfium.load_pdf_from_file(input, None).map_err(pdf_error)?;
        let pages = doc.pages();
        let count = pages.len() as usize;
        if count == 0 {
            return Err(ConvertError::Document("PDF sans page".into()));
        }
        let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("document");
        let (dir, single) = if count == 1 {
            (None, Some(super::unique_output(input, ext)?))
        } else {
            (Some(output::unique_dir(input, stem)?), None)
        };
        let digits = count.to_string().len().max(2);
        for (i, page) in pages.iter().enumerate() {
            output::check_cancel()?;
            let side = page.width().value.max(page.height().value);
            let scale = (RENDER_DPI / 72.0).min(RENDER_MAX_SIDE / side.max(1.0));
            let bitmap = page
                .render_with_config(&PdfRenderConfig::new().scale_page_by_factor(scale).render_form_data(true))
                .map_err(pdf_error)?;
            let img = bitmap.as_image().map_err(pdf_error)?;
            let path = match (&dir, &single) {
                (_, Some(file)) => file.clone(),
                (Some(dir), _) => dir.join(format!("{stem} {:0digits$}.{ext}", i + 1)),
                _ => unreachable!(),
            };
            raster::save(&img, &path, ext)?;
            on_progress((i + 1) as f32 / count as f32);
        }
        Ok(dir.or(single).expect("dossier ou fichier"))
    })
}

/// Cible spéciale : fusionner des PDF (outil Fusionner).
pub const MERGE: &str = "merge";

/// Opération sur tout un lot, qui produit un seul fichier : fusion de PDF (`nom Merged.pdf`)
/// ou images assemblées en PDF (`nom.pdf`), nommé d'après le premier fichier.
pub fn batch(inputs: &[PathBuf], target: &str, on_progress: &mut dyn FnMut(f32)) -> Result<PathBuf, ConvertError> {
    let first = inputs.first().ok_or_else(|| ConvertError::Document("aucun fichier".into()))?;
    for input in inputs {
        if !input.is_file() {
            return Err(ConvertError::NotFound(input.display().to_string()));
        }
    }
    let output = if target == MERGE {
        let output = super::suffixed_output_as(first, "Merged", "pdf")?;
        merge(inputs, &output, on_progress)?;
        output
    } else {
        let output = super::unique_output(first, "pdf")?;
        images_to_pdf(inputs, &output, on_progress)?;
        output
    };
    on_progress(1.0);
    Ok(output)
}

/// Le lot `paths` vers `target` produit un seul fichier (voir [`batch`]).
pub fn is_batch(target: &str, paths: &[String]) -> bool {
    target == MERGE
        || (target == "pdf"
            && paths.len() > 1
            && paths.iter().all(|p| super::formats::kind_of(Path::new(p)) == Some(super::formats::Kind::Image)))
}

/// Fusionne des PDF, dans l'ordre, en `output`.
pub fn merge(inputs: &[PathBuf], output_path: &Path, on_progress: &mut dyn FnMut(f32)) -> Result<(), ConvertError> {
    with_pdfium(|pdfium| {
        let (first, rest) = inputs.split_first().ok_or_else(|| ConvertError::Document("aucun PDF".into()))?;
        let mut doc = pdfium.load_pdf_from_file(first, None).map_err(pdf_error)?;
        for (i, input) in rest.iter().enumerate() {
            output::check_cancel()?;
            let other = pdfium.load_pdf_from_file(input, None).map_err(pdf_error)?;
            doc.pages_mut().append(&other).map_err(pdf_error)?;
            on_progress((i + 1) as f32 / inputs.len() as f32);
        }
        doc.save_to_file(output_path).map_err(|e| {
            let _ = std::fs::remove_file(output_path);
            pdf_error(e)
        })
    })
}

/// Assemble des images en un PDF : une page A4 par image (portrait ou paysage selon
/// l'image), l'image ajustée à la page et centrée. Photos en JPEG, images transparentes ou
/// à aplats (PNG, GIF…) sans perte.
pub fn images_to_pdf(inputs: &[PathBuf], output_path: &Path, on_progress: &mut dyn FnMut(f32)) -> Result<(), ConvertError> {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();
    for (i, input) in inputs.iter().enumerate() {
        output::check_cancel()?;
        let img = raster::load(input)?;
        let (w, h) = img.dimensions();
        let image_id = doc.add_object(image_stream(&img, input)?);
        let (pw, ph) = if w > h { (A4.1, A4.0) } else { A4 };
        let scale = (pw / w as f32).min(ph / h as f32);
        let (dw, dh) = (w as f32 * scale, h as f32 * scale);
        let content = format!("q {dw:.2} 0 0 {dh:.2} {:.2} {:.2} cm /Im0 Do Q", (pw - dw) / 2.0, (ph - dh) / 2.0);
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), pw.into(), ph.into()],
            "Contents" => content_id,
            "Resources" => dictionary! { "XObject" => dictionary! { "Im0" => image_id } },
        });
        kids.push(Object::from(page_id));
        on_progress((i + 1) as f32 / inputs.len() as f32 * 0.95);
    }
    let count = kids.len() as i64;
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }));
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    doc.compress();
    doc.save(output_path).map(|_| ()).map_err(|e| {
        let _ = std::fs::remove_file(output_path);
        ConvertError::Document(e.to_string())
    })
}

/// Image d'une page : JPEG pour une photo opaque, sinon pixels compressés sans perte
/// (avec un masque de transparence s'il y en a).
fn image_stream(img: &DynamicImage, input: &Path) -> Result<Stream, ConvertError> {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let transparent = rgba.pixels().any(|p| p.0[3] < 255);
    let photo = matches!(super::formats::ext_of(input).as_deref(), Some("jpg" | "heic" | "avif" | "webp" | "tiff"));
    if photo && !transparent {
        let data = raster::encode_jpeg(img, PHOTO_QUALITY)?;
        return Ok(jpeg_stream(data, w, h, false));
    }
    let rgb: Vec<u8> = rgba.pixels().flat_map(|p| [p.0[0], p.0[1], p.0[2]]).collect();
    let mut stream = Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64,
            "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
        },
        rgb,
    );
    if transparent {
        let alpha: Vec<u8> = rgba.pixels().map(|p| p.0[3]).collect();
        let mut mask = Stream::new(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64,
                "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8,
            },
            alpha,
        );
        let _ = mask.compress();
        stream.dict.set("SMask", Object::Stream(mask));
    }
    let _ = stream.compress();
    Ok(stream)
}

fn jpeg_stream(data: Vec<u8>, w: u32, h: u32, gray: bool) -> Stream {
    Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => w as i64, "Height" => h as i64,
            "ColorSpace" => if gray { "DeviceGray" } else { "DeviceRGB" }, "BitsPerComponent" => 8,
            "Filter" => "DCTDecode",
        },
        data,
    )
    .with_compression(false)
}

/// Compresse un PDF : ses images (photos, scans) sont réduites à `max_side` pixels au plus
/// et réenregistrées en JPEG de qualité `quality` ; le reste (texte, vectoriel) est gardé
/// et ses flux recompressés. Renvoie le PDF compressé, en mémoire.
pub fn compress(input: &Path, max_side: u32, quality: u8, on_progress: &mut dyn FnMut(f32)) -> Result<Vec<u8>, ConvertError> {
    let mut doc = Document::load(input).map_err(|e| ConvertError::Document(format!("PDF illisible ({e})")))?;
    if doc.is_encrypted() {
        return Err(ConvertError::Document("PDF protégé par un mot de passe".into()));
    }
    let ids: Vec<_> = doc
        .objects
        .iter()
        .filter(|(_, o)| matches!(o, Object::Stream(s) if is_image(s)))
        .map(|(id, _)| *id)
        .collect();
    for (i, id) in ids.iter().enumerate() {
        output::check_cancel()?;
        if let Some(Object::Stream(stream)) = doc.objects.get_mut(id) {
            if let Some(smaller) = recompress_image(stream, max_side, quality) {
                *stream = smaller;
            }
        }
        on_progress((i + 1) as f32 / ids.len().max(1) as f32 * 0.9);
    }
    doc.compress();
    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| ConvertError::Document(e.to_string()))?;
    Ok(out)
}

fn is_image(stream: &Stream) -> bool {
    stream.dict.get(b"Subtype").and_then(|s| s.as_name()).is_ok_and(|n| n == b"Image")
}

/// Image réduite et réencodée en JPEG, si elle est d'un genre qu'on sait lire (JPEG, ou
/// pixels RVB/gris 8 bits) et que le résultat est plus léger ; sinon `None` (inchangée).
fn recompress_image(stream: &Stream, max_side: u32, quality: u8) -> Option<Stream> {
    let dict = &stream.dict;
    let int = |key: &[u8]| dict.get(key).and_then(|v| v.as_i64()).ok();
    if dict.get(b"ImageMask").and_then(|v| v.as_bool()).unwrap_or(false) {
        return None;
    }
    let (w, h) = (int(b"Width")? as u32, int(b"Height")? as u32);
    let filters: Vec<Vec<u8>> = match dict.get(b"Filter").ok()? {
        Object::Name(n) => vec![n.clone()],
        Object::Array(a) => a.iter().filter_map(|o| o.as_name().ok().map(<[u8]>::to_vec)).collect(),
        _ => return None,
    };
    let color = dict.get(b"ColorSpace").and_then(|c| c.as_name()).ok().map(<[u8]>::to_vec);
    let img = match filters.as_slice() {
        [f] if f == b"DCTDecode" => image::load_from_memory_with_format(&stream.content, image::ImageFormat::Jpeg).ok()?,
        [f] if f == b"FlateDecode" && int(b"BitsPerComponent") == Some(8) => {
            let raw = stream.decompressed_content().ok()?;
            match color.as_deref() {
                Some(b"DeviceRGB") => DynamicImage::ImageRgb8(image::RgbImage::from_raw(w, h, raw)?),
                Some(b"DeviceGray") => DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, raw)?),
                _ => return None,
            }
        }
        _ => return None,
    };
    // Petites images (icônes, logos) : rien à gagner, et le JPEG les abîmerait.
    if w.max(h) < 300 {
        return None;
    }
    let img = if w.max(h) > max_side { img.resize(max_side, max_side, image::imageops::FilterType::Lanczos3) } else { img };
    let gray = matches!(img, DynamicImage::ImageLuma8(_));
    let data = if gray {
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality).encode_image(&img).ok()?;
        out
    } else {
        raster::encode_jpeg(&img, quality).ok()?
    };
    if data.len() >= stream.content.len() {
        return None;
    }
    let mut smaller = jpeg_stream(data, img.width(), img.height(), gray);
    // La transparence (masque à part) est gardée telle quelle.
    if let Ok(mask) = dict.get(b"SMask") {
        smaller.dict.set("SMask", mask.clone());
    }
    Some(smaller)
}
