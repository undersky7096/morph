use std::path::Path;

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, ImageDecoder, ImageFormat, ImageReader, Rgb, RgbImage, RgbaImage};

use super::catalog::{self, Engine};
use super::{ConvertError, ffmpeg, svg};

const JPEG_QUALITY: u8 = 90;
/// Tailles d'une icône Windows (ICO) : chacune est dessinée à part, pour rester nette.
const ICO_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];

/// Conversion image → image via la crate `image`.
pub fn convert(input: &Path, output: &Path, target: &str) -> Result<(), ConvertError> {
    save(&load(input)?, output, target)
}

/// Charge une image en appliquant son orientation EXIF (photos de téléphone),
/// pour travailler sur la même image que celle affichée à l'écran.
pub fn load(input: &Path) -> Result<DynamicImage, ConvertError> {
    let ext = input.extension().and_then(|e| e.to_str()).unwrap_or_default();
    match catalog::find(ext).and_then(|f| f.read) {
        Some(Engine::Svg) => svg::render(input),
        // ffmpeg applique lui-même la rotation (HEIC) et assemble les tuiles.
        Some(Engine::Ffmpeg) => ffmpeg::decode_still(input),
        _ => load_raster(input),
    }
}

fn load_raster(input: &Path) -> Result<DynamicImage, ConvertError> {
    let mut decoder = ImageReader::open(input)?.with_guessed_format()?.into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    Ok(img)
}

/// Enregistre `img` au format `ext`, en gérant les contraintes propres à chaque format.
pub fn save(img: &DynamicImage, output: &Path, ext: &str) -> Result<(), ConvertError> {
    let result = match ext {
        "jpg" => encode_jpeg(img, JPEG_QUALITY).and_then(|data| Ok(std::fs::write(output, data)?)),
        "ico" => encode_ico(img).and_then(|data| Ok(std::fs::write(output, data)?)),
        "avif" => ffmpeg::encode_avif(img, output),
        // Sans libwebp dans ffmpeg (selon la plateforme) : WebP sans perte de la crate `image`.
        "webp" => ffmpeg::encode_webp(img, output)
            .or_else(|_| img.save_with_format(output, ImageFormat::WebP).map_err(Into::into)),
        _ => ImageFormat::from_extension(ext)
            .ok_or_else(|| ConvertError::Unsupported(ext.to_string()))
            .and_then(|format| img.save_with_format(output, format).map_err(Into::into)),
    };
    if result.is_err() {
        let _ = std::fs::remove_file(output);
    }
    result
}

/// Icône multi-résolution : toutes les tailles de [`ICO_SIZES`] jusqu'à celle de l'image
/// (au moins 16 px), chacune centrée sur un carré transparent.
fn encode_ico(img: &DynamicImage) -> Result<Vec<u8>, ConvertError> {
    let side = img.width().max(img.height());
    let frames = ICO_SIZES
        .iter()
        .filter(|&&s| s <= side.max(16))
        .map(|&size| {
            let fitted = img.resize(size, size, FilterType::Lanczos3).to_rgba8();
            let mut square = RgbaImage::new(size, size);
            let (x, y) = ((size - fitted.width()) / 2, (size - fitted.height()) / 2);
            image::imageops::overlay(&mut square, &fitted, x.into(), y.into());
            IcoFrame::as_png(square.as_raw(), size, size, ExtendedColorType::Rgba8)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::new();
    IcoEncoder::new(&mut out).encode_images(&frames)?;
    Ok(out)
}

/// Encode en JPEG (transparence composée sur fond blanc).
pub fn encode_jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>, ConvertError> {
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality).encode_image(&flatten_on_white(img))?;
    Ok(out)
}

/// JPEG ne gère pas la transparence : on compose sur fond blanc plutôt que noir.
fn flatten_on_white(img: &DynamicImage) -> RgbImage {
    let rgba = img.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let a = a as u16;
        let blend = |c: u8| ((c as u16 * a + 255 * (255 - a)) / 255) as u8;
        Rgb([blend(r), blend(g), blend(b)])
    })
}
