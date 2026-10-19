use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, ImageReader, Rgb, RgbImage};

use super::ConvertError;

const JPEG_QUALITY: u8 = 90;
const ICO_MAX_SIZE: u32 = 256;

/// Conversion image → image via la crate `image`.
pub fn convert(input: &Path, output: &Path, target: &str) -> Result<(), ConvertError> {
    let img = ImageReader::open(input)?.with_guessed_format()?.decode()?;

    match target {
        "jpg" => {
            let rgb = flatten_on_white(&img);
            let mut out = BufWriter::new(File::create(output)?);
            JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY).encode_image(&rgb)?;
        }
        "ico" => {
            // Le format ICO est limité à 256×256.
            let img = if img.width() > ICO_MAX_SIZE || img.height() > ICO_MAX_SIZE {
                img.resize(ICO_MAX_SIZE, ICO_MAX_SIZE, FilterType::Lanczos3)
            } else {
                img
            };
            img.to_rgba8().save_with_format(output, ImageFormat::Ico)?;
        }
        _ => {
            let format = ImageFormat::from_extension(target)
                .ok_or_else(|| ConvertError::Unsupported(target.to_string()))?;
            img.save_with_format(output, format)?;
        }
    }
    Ok(())
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
