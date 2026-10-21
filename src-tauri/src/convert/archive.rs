//! Archives : extraire (ZIP, 7Z, TAR, GZ, BZ2, XZ par des crates intégrées ; RAR par le
//! bsdtar du système), créer une archive ZIP ou 7Z à partir de fichiers et de dossiers, et
//! changer le format d'une archive (RAR → ZIP…).
//!
//! L'extraction passe par un dossier provisoire voisin : si l'archive ne contient qu'un
//! élément (un dossier, le plus souvent), il est sorti tel quel, sans `nom/nom/`.

use std::fs::{self, File};
use std::cell::RefCell;
use std::io::{self, BufReader, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{ConvertError, no_window, output};

/// Cibles spéciales : archiver toute la sélection, extraire une archive.
pub const ARCHIVE_ZIP: &str = "archive-zip";
pub const ARCHIVE_7Z: &str = "archive-7z";
pub const EXTRACT: &str = "extract";

fn archive_error(e: impl std::fmt::Display) -> ConvertError {
    ConvertError::Archive(e.to_string())
}

/// Genre d'une archive, d'après son nom (`.tar.gz` et `.tgz` comptent comme des TAR).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shape {
    Zip,
    SevenZ,
    Rar,
    /// TAR, éventuellement compressé.
    Tar(Codec),
    /// Un seul fichier compressé (`notes.txt.gz`).
    Single(Codec),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Codec {
    None,
    Gz,
    Bz2,
    Xz,
}

fn shape(path: &Path) -> Option<Shape> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let inner_tar = path
        .file_stem()
        .and_then(|s| Path::new(s).extension())
        .is_some_and(|e| e.eq_ignore_ascii_case("tar"));
    let packed = |codec, tar_alias: bool| if inner_tar || tar_alias { Shape::Tar(codec) } else { Shape::Single(codec) };
    Some(match ext.as_str() {
        "zip" => Shape::Zip,
        "7z" => Shape::SevenZ,
        "rar" => Shape::Rar,
        "tar" => Shape::Tar(Codec::None),
        "gz" => packed(Codec::Gz, false),
        "tgz" => packed(Codec::Gz, true),
        "bz2" => packed(Codec::Bz2, false),
        "tbz2" | "tbz" => packed(Codec::Bz2, true),
        "xz" => packed(Codec::Xz, false),
        "txz" => packed(Codec::Xz, true),
        _ => return None,
    })
}

/// Nom de l'archive sans ses extensions (`photos.tar.gz` → `photos`).
pub fn base_name(path: &Path) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("archive");
    let stem = Path::new(stem);
    match stem.extension() {
        Some(e) if e.eq_ignore_ascii_case("tar") => stem.file_stem().unwrap_or_default().to_string_lossy().into_owned(),
        _ => stem.to_string_lossy().into_owned(),
    }
}

/// Lecteur qui compte les octets lus (progression d'après la taille de l'archive).
struct Counting<R> {
    inner: R,
    read: Arc<AtomicU64>,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

impl<R: Seek> Seek for Counting<R> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

fn decoder<'a>(codec: Codec, reader: impl Read + 'a) -> Result<Box<dyn Read + 'a>, ConvertError> {
    let reader = BufReader::new(reader);
    Ok(match codec {
        Codec::None => Box::new(reader),
        Codec::Gz => Box::new(flate2::read::MultiGzDecoder::new(reader)),
        Codec::Bz2 => Box::new(bzip2::read::MultiBzDecoder::new(reader)),
        Codec::Xz => Box::new(lzma_rust2::XzReader::new(reader, true)),
    })
}

/// Chemin d'un élément d'archive sous `root`, refusé s'il en sortirait (`../`, absolu).
fn inside(root: &Path, name: &str) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for part in Path::new(&name.replace('\\', "/")).components() {
        match part {
            Component::Normal(p) => path.push(p),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (path != root).then_some(path)
}

fn write_entry(path: &Path, reader: &mut dyn Read) -> Result<(), ConvertError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::create(path)?;
    io::copy(reader, &mut file)?;
    Ok(())
}

/// Dossier provisoire, à côté de la destination (même disque : déplacement instantané).
fn staging(near: &Path) -> Result<PathBuf, ConvertError> {
    let dir = near.join(format!(".morph-extract-{}-{}", std::process::id(), output::unique_suffix()));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Extrait une archive. Renvoie le dossier créé (`nom/`), ou l'élément unique qu'elle
/// contenait, ou le fichier décompressé (`notes.txt.gz` → `notes.txt`).
pub fn extract(input: &Path, on_progress: &mut dyn FnMut(f32)) -> Result<PathBuf, ConvertError> {
    if !input.is_file() {
        return Err(ConvertError::NotFound(input.display().to_string()));
    }
    let shape = shape(input).ok_or_else(|| ConvertError::Unsupported(input.display().to_string()))?;
    // Où écrire : à côté de l'archive, ou dans le dossier de sortie choisi.
    let dest_parent = output::out_dir(input);
    fs::create_dir_all(&dest_parent)?;

    if let Shape::Single(codec) = shape {
        let name = input.file_stem().and_then(|s| s.to_str()).unwrap_or("fichier");
        let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
        let out = output::unique_in_dir(input, stem, ext)?;
        let out = if ext.is_empty() { out.with_extension("") } else { out };
        let total = fs::metadata(input)?.len().max(1);
        let read = Arc::new(AtomicU64::new(0));
        let mut reader = decoder(codec, Counting { inner: File::open(input)?, read: read.clone() })?;
        let mut file = File::create(&out)?;
        let mut buf = vec![0u8; 1 << 16];
        let result = (|| loop {
            output::check_cancel()?;
            let n = reader.read(&mut buf).map_err(archive_error)?;
            if n == 0 {
                return Ok(());
            }
            file.write_all(&buf[..n])?;
            on_progress(read.load(Ordering::Relaxed) as f32 / total as f32);
        })();
        if let Err(e) = result {
            drop(file);
            let _ = fs::remove_file(&out);
            return Err(e);
        }
        return Ok(out);
    }

    let stage = staging(&dest_parent)?;
    let result = unpack(input, shape, &stage, on_progress).and_then(|()| place(&stage, input, &dest_parent));
    let _ = fs::remove_dir_all(&stage);
    result
}

/// Sort le contenu extrait du dossier provisoire : son élément unique tel quel, sinon tout
/// dans un dossier au nom de l'archive.
fn place(stage: &Path, input: &Path, dest_parent: &Path) -> Result<PathBuf, ConvertError> {
    let items: Vec<PathBuf> = fs::read_dir(stage)?.flatten().map(|e| e.path()).collect();
    let free = |name: &str| {
        let mut target = dest_parent.join(name);
        let (stem, ext) = match Path::new(name).extension() {
            Some(e) if !dest_parent.join(name).is_dir() => {
                (Path::new(name).file_stem().unwrap_or_default().to_string_lossy().into_owned(), format!(".{}", e.to_string_lossy()))
            }
            _ => (name.to_string(), String::new()),
        };
        let mut n = 2;
        while target.exists() {
            target = dest_parent.join(format!("{stem} ({n}){ext}"));
            n += 1;
        }
        target
    };
    let target = match items.as_slice() {
        [] => return Err(ConvertError::Archive("archive vide".into())),
        [single] => {
            let target = free(&single.file_name().unwrap_or_default().to_string_lossy());
            fs::rename(single, &target)?;
            target
        }
        _ => {
            let target = free(&base_name(input));
            fs::rename(stage, &target)?;
            return Ok(target);
        }
    };
    Ok(target)
}

fn unpack(input: &Path, shape: Shape, dest: &Path, on_progress: &mut dyn FnMut(f32)) -> Result<(), ConvertError> {
    match shape {
        Shape::Zip => {
            let mut zip = zip::ZipArchive::new(BufReader::new(File::open(input)?)).map_err(archive_error)?;
            let count = zip.len().max(1);
            for i in 0..zip.len() {
                output::check_cancel()?;
                let mut entry = zip.by_index(i).map_err(archive_error)?;
                let Some(path) = entry.enclosed_name().and_then(|n| inside(dest, &n.to_string_lossy())) else { continue };
                if entry.is_dir() {
                    fs::create_dir_all(&path)?;
                } else {
                    write_entry(&path, &mut entry)?;
                }
                on_progress((i + 1) as f32 / count as f32);
            }
            Ok(())
        }
        Shape::SevenZ => {
            let total = fs::metadata(input)?.len().max(1) as f32;
            let mut done = 0u64;
            let mut cancelled = false;
            let result = sevenz_rust2::decompress_file_with_extract_fn(input, dest, |entry, reader, _| {
                if output::cancelled() {
                    cancelled = true;
                    return Ok(false);
                }
                let Some(path) = inside(dest, entry.name()) else { return Ok(true) };
                if entry.is_directory() {
                    fs::create_dir_all(&path)?;
                } else {
                    if let Some(parent) = path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    io::copy(reader, &mut File::create(&path)?)?;
                }
                done += entry.compressed_size;
                on_progress((done as f32 / total).min(1.0));
                Ok(true)
            });
            if cancelled {
                return Err(ConvertError::Cancelled);
            }
            result.map_err(archive_error)
        }
        Shape::Tar(codec) => {
            let total = fs::metadata(input)?.len().max(1) as f32;
            let read = Arc::new(AtomicU64::new(0));
            let reader = decoder(codec, Counting { inner: File::open(input)?, read: read.clone() })?;
            let mut tar = tar::Archive::new(reader);
            for entry in tar.entries().map_err(archive_error)? {
                output::check_cancel()?;
                // `unpack_in` refuse les chemins qui sortiraient du dossier.
                entry.map_err(archive_error)?.unpack_in(dest).map_err(archive_error)?;
                on_progress(read.load(Ordering::Relaxed) as f32 / total);
            }
            Ok(())
        }
        Shape::Rar => unrar(input, dest),
        Shape::Single(_) => unreachable!("traité à part"),
    }
}

/// Programme du système qui sait lire les RAR, avec ses arguments d'extraction.
pub fn rar_tool() -> Option<(PathBuf, &'static [&'static str])> {
    #[cfg(windows)]
    {
        // bsdtar (libarchive) fait partie de Windows 10/11.
        let tar = PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
            .join("System32")
            .join("tar.exe");
        tar.is_file().then_some((tar, &["-xf"][..]))
    }
    #[cfg(not(windows))]
    {
        let in_path = |name: &str| {
            std::env::var_os("PATH")
                .and_then(|p| std::env::split_paths(&p).map(|d| d.join(name)).find(|p| p.is_file()))
        };
        if let Some(p) = in_path("bsdtar") {
            Some((p, &["-xf"][..]))
        } else if let Some(p) = in_path("unrar") {
            Some((p, &["x", "-o-", "-y"][..]))
        } else {
            in_path("7z").or_else(|| in_path("7zz")).map(|p| (p, &["x", "-y"][..]))
        }
    }
}

fn unrar(input: &Path, dest: &Path) -> Result<(), ConvertError> {
    let (tool, args) = rar_tool().ok_or(ConvertError::EngineMissing(super::engines::RAR_MISSING.reason))?;
    let mut cmd = Command::new(&tool);
    no_window(&mut cmd);
    let name = tool.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
    cmd.args(args).arg(input);
    if name.starts_with("unrar") || name.starts_with("7z") {
        // unrar x archive dossier/ ; 7z x archive -odossier
        if name.starts_with("7z") {
            cmd.arg(format!("-o{}", dest.display()));
        } else {
            cmd.arg(format!("{}{}", dest.display(), std::path::MAIN_SEPARATOR));
        }
    } else {
        cmd.arg("-C").arg(dest);
    }
    let out = cmd.output().map_err(archive_error)?;
    if out.status.success() {
        Ok(())
    } else {
        Err(ConvertError::Archive(String::from_utf8_lossy(&out.stderr).trim().to_string()))
    }
}

/// Fichiers à archiver : (chemin, nom dans l'archive). Un dossier choisi garde son nom en
/// tête de ses fichiers ; ses sous-dossiers vides sont gardés aussi (nom en `/`).
fn collect(inputs: &[PathBuf]) -> Result<Vec<(PathBuf, String)>, ConvertError> {
    let mut entries = Vec::new();
    for input in inputs {
        let base = input.parent().unwrap_or(Path::new(""));
        for item in walkdir::WalkDir::new(input).sort_by_file_name() {
            let item = item.map_err(|e| ConvertError::Io(io::Error::other(e)))?;
            let rel = item.path().strip_prefix(base).unwrap_or(item.path());
            let mut name = rel.to_string_lossy().replace('\\', "/");
            if item.file_type().is_dir() {
                name.push('/');
            } else if !item.file_type().is_file() {
                continue;
            }
            entries.push((item.path().to_path_buf(), name));
        }
    }
    Ok(entries)
}

/// Archive (`format` : zip ou 7z) de `inputs`, fichiers et dossiers, nommée d'après le
/// premier élément (`Photos.zip`), à côté de lui ou dans le dossier de sortie.
pub fn create(inputs: &[PathBuf], format: &str, on_progress: &mut dyn FnMut(f32)) -> Result<PathBuf, ConvertError> {
    let first = inputs.first().ok_or_else(|| ConvertError::Archive("rien à archiver".into()))?;
    for input in inputs {
        if !input.exists() {
            return Err(ConvertError::NotFound(input.display().to_string()));
        }
    }
    let name = if first.is_dir() {
        first.file_name().unwrap_or_default().to_string_lossy().into_owned()
    } else {
        first.file_stem().unwrap_or_default().to_string_lossy().into_owned()
    };
    let output = output::unique_in_dir(first, &name, format)?;
    let result = write_archive(&collect(inputs)?, &output, format, on_progress);
    if result.is_err() {
        let _ = fs::remove_file(&output);
    }
    result.map(|()| output)
}

/// Archive ZIP ou 7Z (compression standard) de `entries`.
fn write_archive(entries: &[(PathBuf, String)], output: &Path, format: &str, on_progress: &mut dyn FnMut(f32)) -> Result<(), ConvertError> {
    let total: u64 = entries.iter().filter_map(|(p, _)| fs::metadata(p).ok()).filter(|m| m.is_file()).map(|m| m.len()).sum();
    let read = Arc::new(AtomicU64::new(0));
    let progress = |read: &Arc<AtomicU64>| read.load(Ordering::Relaxed) as f32 / total.max(1) as f32;
    match format {
        "zip" => {
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .large_file(total > u32::MAX as u64);
            let mut zip = zip::ZipWriter::new(io::BufWriter::new(File::create(output)?));
            for (path, name) in entries {
                output::check_cancel()?;
                if name.ends_with('/') {
                    zip.add_directory(name.trim_end_matches('/'), options).map_err(archive_error)?;
                    continue;
                }
                zip.start_file(name.as_str(), options).map_err(archive_error)?;
                io::copy(&mut Counting { inner: File::open(path)?, read: read.clone() }, &mut zip)?;
                on_progress(progress(&read));
            }
            zip.finish().map_err(archive_error)?.flush()?;
            Ok(())
        }
        "7z" => {
            let mut writer = sevenz_rust2::ArchiveWriter::create(output).map_err(archive_error)?;
            let (dirs, files): (Vec<_>, Vec<_>) = entries.iter().partition(|(_, n)| n.ends_with('/'));
            for (_, name) in dirs {
                let entry = sevenz_rust2::ArchiveEntry::new_directory(name.trim_end_matches('/'));
                writer.push_archive_entry::<&[u8]>(entry, None).map_err(archive_error)?;
            }
            // Compression « solide » (tous les fichiers d'un bloc) : meilleur taux. La
            // progression et l'annulation sont relevées à chaque lecture des fichiers.
            let report = RefCell::new(on_progress);
            let mut list = Vec::new();
            let mut readers = Vec::new();
            for (path, name) in files {
                list.push(sevenz_rust2::ArchiveEntry::from_path(path, name.clone()));
                readers.push(sevenz_rust2::SourceReader::new(Watched {
                    inner: File::open(path)?,
                    read: read.clone(),
                    total,
                    report: &report,
                }));
            }
            if !list.is_empty() {
                let result = writer.push_archive_entries(list, readers);
                output::check_cancel()?;
                result.map_err(archive_error)?;
            }
            writer.finish().map_err(archive_error)?;
            Ok(())
        }
        other => Err(ConvertError::Unsupported(other.to_string())),
    }
}

/// Fichier lu pendant la compression : fait avancer la progression, et s'arrête si
/// l'opération est annulée.
struct Watched<'a, 'b, R> {
    inner: R,
    read: Arc<AtomicU64>,
    total: u64,
    report: &'a RefCell<&'b mut dyn FnMut(f32)>,
}

impl<R: Read> Read for Watched<'_, '_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if output::cancelled() {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "annulé"));
        }
        let n = self.inner.read(buf)?;
        let done = self.read.fetch_add(n as u64, Ordering::Relaxed) + n as u64;
        (self.report.borrow_mut())(done as f32 / self.total.max(1) as f32);
        Ok(n)
    }
}

/// Change le format d'une archive (RAR, TAR… → ZIP ou 7Z) : extraction provisoire, puis
/// nouvelle archive du contenu.
pub fn repack(input: &Path, output_path: &Path, format: &str, on_progress: &mut dyn FnMut(f32)) -> Result<(), ConvertError> {
    let shape = shape(input).ok_or_else(|| ConvertError::Unsupported(input.display().to_string()))?;
    let parent = output_path.parent().unwrap_or(Path::new("."));
    let stage = staging(parent)?;
    let result = (|| {
        if let Shape::Single(codec) = shape {
            let name = input.file_stem().unwrap_or_default();
            let mut reader = decoder(codec, File::open(input)?)?;
            write_entry(&stage.join(name), &mut reader)?;
        } else {
            unpack(input, shape, &stage, &mut |p| on_progress(p * 0.5))?;
        }
        let items: Vec<PathBuf> = fs::read_dir(&stage)?.flatten().map(|e| e.path()).collect();
        write_archive(&collect(&items)?, output_path, format, &mut |p| on_progress(0.5 + p * 0.5))
    })();
    let _ = fs::remove_dir_all(&stage);
    if result.is_err() {
        let _ = fs::remove_file(output_path);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_shapes() {
        assert_eq!(shape(Path::new("a.tar.gz")), Some(Shape::Tar(Codec::Gz)));
        assert_eq!(shape(Path::new("a.TGZ")), Some(Shape::Tar(Codec::Gz)));
        assert_eq!(shape(Path::new("notes.txt.gz")), Some(Shape::Single(Codec::Gz)));
        assert_eq!(shape(Path::new("a.tar.xz")), Some(Shape::Tar(Codec::Xz)));
        assert_eq!(base_name(Path::new("photos.tar.bz2")), "photos");
        assert_eq!(base_name(Path::new("photos.zip")), "photos");
    }

    #[test]
    fn refuses_paths_outside() {
        let root = Path::new("/x");
        assert_eq!(inside(root, "a/b.txt"), Some(root.join("a").join("b.txt")));
        assert_eq!(inside(root, "../evil"), None);
        assert_eq!(inside(root, "/etc/passwd"), None);
        assert_eq!(inside(root, "a\\..\\..\\evil"), None);
    }

    #[test]
    fn zip_and_7z_round_trip() {
        let dir = std::env::temp_dir().join("morph-test-archive");
        let _ = fs::remove_dir_all(&dir);
        let photos = dir.join("Photos");
        fs::create_dir_all(photos.join("vide")).unwrap();
        fs::write(photos.join("a.txt"), b"bonjour").unwrap();
        fs::write(dir.join("seul.txt"), vec![7u8; 100_000]).unwrap();
        for format in ["zip", "7z"] {
            let archive = create(&[photos.clone(), dir.join("seul.txt")], format, &mut |_| {}).unwrap();
            assert_eq!(archive, dir.join(format!("Photos.{format}")));
            let out = extract(&archive, &mut |_| {}).unwrap();
            // Deux éléments : rangés dans un dossier au nom de l'archive (« Photos (2) »).
            assert_eq!(out, dir.join("Photos (2)"));
            assert_eq!(fs::read(out.join("Photos").join("a.txt")).unwrap(), b"bonjour");
            assert!(out.join("Photos").join("vide").is_dir());
            assert_eq!(fs::read(out.join("seul.txt")).unwrap().len(), 100_000);
            fs::remove_dir_all(&out).unwrap();
            fs::remove_file(&archive).unwrap();
        }
        // Un seul dossier dans l'archive : sorti tel quel.
        let archive = create(&[photos.clone()], "zip", &mut |_| {}).unwrap();
        assert_eq!(extract(&archive, &mut |_| {}).unwrap(), dir.join("Photos (2)"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
