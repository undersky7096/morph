//! Table centrale des formats : ajouter un format revient à ajouter une ligne à [`FORMATS`].
//! Les menus (Rust et interface, via la commande `format_catalog`), les conversions possibles,
//! les outils proposés et la copie des flux sans réencodage en découlent.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Image,
    Audio,
    Video,
}

impl Kind {
    /// Types de sortie atteignables depuis un fichier de ce type (le sien d'abord).
    pub fn reaches(self) -> &'static [Kind] {
        match self {
            Kind::Image => &[Kind::Image],
            Kind::Audio => &[Kind::Audio],
            // Une vidéo peut perdre son image : extraction du son.
            Kind::Video => &[Kind::Video, Kind::Audio],
        }
    }
}

/// Moteur qui lit ou écrit un format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Engine {
    /// Crate `image`, intégrée.
    Image,
    /// Crate `resvg`, intégrée.
    Svg,
    /// ffmpeg, sidecar.
    Ffmpeg,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Format {
    /// Extension de référence, aussi nom du format dans le menu (en majuscules).
    pub ext: &'static str,
    /// Autres extensions du même format (`jpeg` → `jpg`).
    pub aliases: &'static [&'static str],
    pub kind: Kind,
    /// Moteur qui le lit (`None` : sortie seulement).
    pub read: Option<Engine>,
    /// Moteur qui l'écrit (`None` : entrée seulement).
    pub write: Option<Engine>,
    /// Sorties d'un autre type que ceux de [`Kind::reaches`] (GIF animé → vidéo).
    pub extra: &'static [&'static str],
    /// L'outil Compresser le prend tel quel (sinon : à convertir d'abord).
    pub compress: bool,
    /// Ouvrable dans Recadrer, Détourer et l'éditeur d'image (affichable par le webview
    /// et réenregistrable dans son format).
    pub edit: bool,
    /// Types MIME (Linux : « Ouvrir avec Morph » et clic droit des gestionnaires de fichiers).
    #[serde(skip)]
    pub mime: &'static [&'static str],
    /// Conteneur de sortie : codecs (noms ffmpeg) recopiés tels quels, sans réencodage.
    #[serde(skip)]
    pub copy_video: &'static [&'static str],
    #[serde(skip)]
    pub copy_audio: &'static [&'static str],
}

const IMAGE: Format = Format {
    ext: "",
    aliases: &[],
    kind: Kind::Image,
    read: Some(Engine::Image),
    write: Some(Engine::Image),
    extra: &[],
    compress: false,
    edit: true,
    mime: &[],
    copy_video: &[],
    copy_audio: &[],
};
const AUDIO: Format = Format {
    kind: Kind::Audio,
    read: Some(Engine::Ffmpeg),
    write: Some(Engine::Ffmpeg),
    compress: true,
    edit: true,
    ..IMAGE
};
const VIDEO: Format = Format { kind: Kind::Video, extra: &["gif"], ..AUDIO };

/// Tous les formats, dans l'ordre des pétales (les plus courants d'abord).
pub const FORMATS: &[Format] = &[
    // Images
    Format { ext: "png", mime: &["image/png"], compress: true, ..IMAGE },
    Format { ext: "jpg", aliases: &["jpeg", "jfif"], mime: &["image/jpeg"], compress: true, ..IMAGE },
    Format { ext: "webp", mime: &["image/webp"], compress: true, ..IMAGE },
    // AVIF : libaom (écriture, transparence comprise) et dav1d (lecture), dans ffmpeg.
    Format {
        ext: "avif",
        mime: &["image/avif"],
        read: Some(Engine::Ffmpeg),
        write: Some(Engine::Ffmpeg),
        ..IMAGE
    },
    // Un GIF peut être animé : il peut devenir une vidéo.
    Format { ext: "gif", mime: &["image/gif"], extra: &["mp4", "webm"], compress: true, ..IMAGE },
    Format { ext: "bmp", mime: &["image/bmp"], ..IMAGE },
    Format { ext: "tiff", aliases: &["tif"], mime: &["image/tiff"], ..IMAGE },
    Format { ext: "ico", mime: &["image/x-icon", "image/vnd.microsoft.icon"], ..IMAGE },
    Format { ext: "tga", mime: &["image/x-tga"], ..IMAGE },
    // Photos d'iPhone : décodeur HEVC de ffmpeg (tuiles et rotation gérées). Lecture seule.
    Format {
        ext: "heic",
        aliases: &["heif", "hif"],
        mime: &["image/heic", "image/heif"],
        read: Some(Engine::Ffmpeg),
        write: None,
        edit: false,
        ..IMAGE
    },
    // Vectoriel → image seulement.
    Format {
        ext: "svg",
        aliases: &["svgz"],
        mime: &["image/svg+xml", "image/svg+xml-compressed"],
        read: Some(Engine::Svg),
        write: None,
        edit: false,
        ..IMAGE
    },
    // Audio
    Format { ext: "mp3", mime: &["audio/mpeg"], copy_audio: &["mp3"], ..AUDIO },
    Format { ext: "wav", mime: &["audio/wav", "audio/x-wav"], compress: false, copy_audio: &["pcm_s16le"], ..AUDIO },
    Format { ext: "flac", mime: &["audio/flac", "audio/x-flac"], copy_audio: &["flac"], ..AUDIO },
    Format { ext: "ogg", aliases: &["oga"], mime: &["audio/ogg"], copy_audio: &["vorbis"], ..AUDIO },
    Format { ext: "aac", mime: &["audio/aac"], copy_audio: &["aac"], ..AUDIO },
    Format { ext: "m4a", mime: &["audio/mp4", "audio/x-m4a"], copy_audio: &["aac", "alac"], ..AUDIO },
    Format { ext: "opus", mime: &["audio/opus"], copy_audio: &["opus"], ..AUDIO },
    Format { ext: "wma", mime: &["audio/x-ms-wma"], copy_audio: &["wmav2"], ..AUDIO },
    Format {
        ext: "aiff",
        aliases: &["aif"],
        mime: &["audio/x-aiff", "audio/aiff"],
        compress: false,
        copy_audio: &["pcm_s16be"],
        ..AUDIO
    },
    // Vidéo
    Format {
        ext: "mp4",
        aliases: &["m4v"],
        mime: &["video/mp4"],
        copy_video: &["h264", "hevc", "av1", "mpeg4"],
        copy_audio: &["aac", "mp3", "alac"],
        ..VIDEO
    },
    Format {
        ext: "webm",
        mime: &["video/webm"],
        copy_video: &["vp8", "vp9", "av1"],
        copy_audio: &["opus", "vorbis"],
        ..VIDEO
    },
    Format {
        ext: "mkv",
        mime: &["video/x-matroska"],
        copy_video: &["h264", "hevc", "av1", "vp8", "vp9", "mpeg4"],
        copy_audio: &["aac", "mp3", "opus", "vorbis", "flac", "ac3", "eac3", "alac"],
        ..VIDEO
    },
    Format {
        ext: "mov",
        mime: &["video/quicktime"],
        copy_video: &["h264", "hevc", "mpeg4", "prores"],
        copy_audio: &["aac", "mp3", "alac", "pcm_s16le"],
        ..VIDEO
    },
    Format { ext: "avi", mime: &["video/x-msvideo"], ..VIDEO },
    Format { ext: "wmv", mime: &["video/x-ms-wmv"], ..VIDEO },
    Format { ext: "flv", mime: &["video/x-flv"], copy_video: &["h264"], copy_audio: &["aac", "mp3"], ..VIDEO },
    Format { ext: "3gp", mime: &["video/3gpp"], ..VIDEO },
    Format { ext: "mpeg", aliases: &["mpg"], mime: &["video/mpeg"], write: None, ..VIDEO },
    Format { ext: "ts", aliases: &["mts", "m2ts"], mime: &["video/mp2t"], write: None, ..VIDEO },
];

/// Format d'une extension (alias compris, sans tenir compte de la casse).
pub fn find(ext: &str) -> Option<&'static Format> {
    let ext = ext.to_ascii_lowercase();
    FORMATS.iter().find(|f| f.ext == ext || f.aliases.contains(&ext.as_str()))
}

impl Format {
    /// Moteurs nécessaires pour passer de ce format à `to`.
    pub fn engines_to(&self, to: &Format) -> impl Iterator<Item = Engine> {
        self.read.into_iter().chain(to.write)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for f in FORMATS {
            for ext in std::iter::once(&f.ext).chain(f.aliases) {
                assert!(seen.insert(*ext), "extension en double : {ext}");
                assert_eq!(*ext, ext.to_ascii_lowercase());
            }
        }
    }

    #[test]
    fn extras_are_writable_formats() {
        for f in FORMATS {
            for e in f.extra {
                assert!(find(e).is_some_and(|t| t.write.is_some()), "{} → {e}", f.ext);
            }
        }
    }
}
