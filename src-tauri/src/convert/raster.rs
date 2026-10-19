use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Rgb, RgbImage};

use super::ConvertError;

const JPEG_QUALITY: u8 = 90;
const ICO_MAX_SIZE: u32 = 256;

/// Conversion image → image via la crate `image`.
pub fn convert(input: &Path, output: &Path, target: &str) -> Result<(), ConvertError> {
    save(&load(input)?, output, target)
}

/// Charge une image en appliquant son orientation EXIF (photos de téléphone),
/// pour travailler sur la même image que celle affichée à l'écran.
pub fn load(input: &Path) -> Result<DynamicImage, ConvertError> {
    let mut decoder = ImageReader::open(input)?.with_guessed_format()?.into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
    Ok(img)
}

/// Enregistre `img` au format `ext`, en gérant les contraintes propres à chaque format.
pub fn save(img: &DynamicImage, output: &Path, ext: &str) -> Result<(), ConvertError> {
    let result = match ext {
        "jpg" => (|| {
            let rgb = flatten_on_white(img);
            let mut out = BufWriter::new(File::create(output)?);
            JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY).encode_image(&rgb)?;
            Ok(())
        })(),
        "ico" => {
            // Le format ICO est limité à 256×256.
            let img = if img.width() > ICO_MAX_SIZE || img.height() > ICO_MAX_SIZE {
                &img.resize(ICO_MAX_SIZE, ICO_MAX_SIZE, FilterType::Lanczos3)
            } else {
                img
            };
            img.to_rgba8().save_with_format(output, ImageFormat::Ico).map_err(Into::into)
        }
        _ => ImageFormat::from_extension(ext)
            .ok_or_else(|| ConvertError::Unsupported(ext.to_string()))
            .and_then(|format| img.save_with_format(output, format).map_err(Into::into)),
    };
    if result.is_err() {
        let _ = std::fs::remove_file(output);
    }
    result
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
