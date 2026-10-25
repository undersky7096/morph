//! Rendu SVG → image (resvg). L'inverse (vectoriser une image) n'est pas proposé.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use image::{DynamicImage, RgbaImage};
use resvg::{tiny_skia, usvg};

use super::ConvertError;

/// Un SVG n'a pas de taille en pixels : un dessin plus petit que ça (une icône de 24 px)
/// est agrandi pour que son plus grand côté atteigne cette taille.
const MIN_SIDE: f32 = 1024.0;
/// Garde-fou contre les dessins déclarés immenses.
const MAX_SIDE: f32 = 8192.0;

/// Polices du système, chargées une fois (pour le texte des SVG).
fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

pub fn render(input: &Path) -> Result<DynamicImage, ConvertError> {
    let data = std::fs::read(input)?;
    let options = usvg::Options {
        resources_dir: input.parent().map(Path::to_path_buf),
        fontdb: fonts(),
        ..usvg::Options::default()
    };
    let unreadable = |e: &dyn std::fmt::Display| ConvertError::Edit(crate::i18n::trf("core.unreadable_svg", &[("error", &e)]));
    let tree = usvg::Tree::from_data(&data, &options).map_err(|e| unreadable(&e))?;

    let size = tree.size();
    let longest = size.width().max(size.height());
    let scale = (MIN_SIDE / longest).max(1.0).min(MAX_SIDE / longest);
    let width = (size.width() * scale).round().max(1.0) as u32;
    let height = (size.height() * scale).round().max(1.0) as u32;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| unreadable(&"taille invalide"))?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());

    // tiny-skia travaille en alpha prémultiplié.
    let pixels = pixmap.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    });
    let img = RgbaImage::from_raw(width, height, pixels.collect()).expect("taille cohérente");
    Ok(DynamicImage::ImageRgba8(img))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_small_icon_enlarged_with_transparency() {
        let dir = std::env::temp_dir().join("morph-test-svg");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("icone.svg");
        std::fs::write(
            &input,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="12"><rect width="12" height="12" fill="#ff0000"/></svg>"##,
        )
        .unwrap();
        let img = render(&input).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (1024, 512));
        assert_eq!(img.get_pixel(10, 10).0, [255, 0, 0, 255]);
        assert_eq!(img.get_pixel(1000, 10).0[3], 0);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
