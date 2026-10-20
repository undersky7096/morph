//! Outils d'édition (menu d'actions) : recadrage, compression, éditeur.

pub mod compress;

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::convert::{self, ConvertError, formats, raster};

/// Zone de recadrage, en pixels de l'image (orientation EXIF appliquée).
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Recadre une image et l'enregistre à côté de l'original avec le suffixe « Cropped ».
pub fn crop(input: &Path, rect: Rect) -> Result<PathBuf, ConvertError> {
    let img = raster::load(input)?;
    let x = rect.x.min(img.width().saturating_sub(1));
    let y = rect.y.min(img.height().saturating_sub(1));
    let width = rect.width.clamp(1, img.width() - x);
    let height = rect.height.clamp(1, img.height() - y);

    let ext = formats::ext_of(input).ok_or_else(|| ConvertError::Unsupported(input.display().to_string()))?;
    let output = convert::suffixed_output(input, "Cropped");
    raster::save(&img.crop_imm(x, y, width, height), &output, &ext)?;
    Ok(output)
}

/// Enregistre le rendu PNG de l'éditeur image au format d'origine, à côté de l'original
/// avec le suffixe « Edited ».
pub fn save_edited(input: &Path, png: &[u8]) -> Result<PathBuf, ConvertError> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)?;
    let ext = formats::ext_of(input).ok_or_else(|| ConvertError::Unsupported(input.display().to_string()))?;
    let output = convert::suffixed_output(input, "Edited");
    raster::save(&img, &output, &ext)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crops_and_clamps() {
        let dir = std::env::temp_dir().join("morph-test-crop");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("img.png");
        image::RgbaImage::new(100, 50).save(&input).unwrap();

        let out = crop(&input, Rect { x: 10, y: 5, width: 40, height: 20 }).unwrap();
        assert_eq!(out, dir.join("img Cropped.png"));
        assert_eq!(image::image_dimensions(&out).unwrap(), (40, 20));

        // Zone qui déborde : ramenée dans l'image.
        let out = crop(&input, Rect { x: 90, y: 40, width: 500, height: 500 }).unwrap();
        assert_eq!(out, dir.join("img Cropped (2).png"));
        assert_eq!(image::image_dimensions(&out).unwrap(), (10, 10));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn saves_edited_in_original_format() {
        let dir = std::env::temp_dir().join("morph-test-edited");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("photo.jpg");
        let mut png = Vec::new();
        image::RgbaImage::new(30, 20)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();

        let out = save_edited(&input, &png).unwrap();
        assert_eq!(out, dir.join("photo Edited.jpg"));
        assert_eq!(image::ImageReader::open(&out).unwrap().with_guessed_format().unwrap().format(), Some(image::ImageFormat::Jpeg));
        assert_eq!(image::image_dimensions(&out).unwrap(), (30, 20));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
