//! Outil de test du moteur de conversion.
//!   morph-cli targets <fichiers…>   liste les formats proposés
//!   morph-cli <format> <fichiers…>  convertit chaque fichier vers <format>
//!   morph-cli compress <light|medium|strong> [--colors] [--target <Mo>] <fichiers…>
//!   morph-cli trim <début-fin,début-fin…> <fichiers…>  garde ces passages (en secondes)
//!   morph-cli merge <pdf…>           fusionne des PDF (nom Merged.pdf)
//!   morph-cli pdf <images…>          assemble plusieurs images en un PDF
//!   morph-cli archive-zip|archive-7z <fichiers ou dossiers…>  archive la sélection
//!   morph-cli extract <archives…>    extrait chaque archive
//!   morph-cli cutout [--cpu] [--best] <images ou vidéos…>  retire le fond (télécharge le
//!                                        modèle la première fois ; --best : modèle « qualité » ;
//!                                        --cpu : sans la carte graphique)

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use morph_lib::convert::{self, formats, output};
use morph_lib::edit::compress::{self, Level, Options, Outcome};
use morph_lib::edit::cutout;
use morph_lib::edit::trim::{self, Segment};

/// Détourage, avec le modèle et ONNX Runtime aux emplacements de l'application installée
/// (ou, en développement, ONNX Runtime de `src-tauri/onnxruntime/`).
fn cutout_cmd(files: &[String]) -> ExitCode {
    let data = if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    let Some(data) = data else {
        eprintln!("dossier de données introuvable");
        return ExitCode::FAILURE;
    };
    cutout::set_models_dir(data.join("com.underskydev.morph").join("models"));
    let beside_exe = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(cutout::RUNTIME_LIB)));
    let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("onnxruntime").join(Path::new(cutout::RUNTIME_LIB).file_name().unwrap());
    cutout::set_runtime(beside_exe.filter(|p| p.exists()).unwrap_or(dev));
    let (cpu, files) = match files.split_first() {
        Some((flag, rest)) if flag == "--cpu" => (true, rest),
        _ => (false, files),
    };
    cutout::set_gpu(!cpu);
    let (quality, files) = match files.split_first() {
        Some((flag, rest)) if flag == "--best" => (cutout::Quality::Best, rest),
        _ => (cutout::Quality::Fast, files),
    };

    let mut failed = false;
    for file in files {
        let mut last = -1;
        let mut report = |p: f32| {
            let pct = (p * 100.0) as i32;
            if pct != last {
                last = pct;
                print!("\r{file} → détourage : {pct:3}%");
                let _ = std::io::stdout().flush();
            }
        };
        match cutout::remove_background(Path::new(file), quality, &mut report) {
            Ok(out) => println!("\r{file} → {} ✓", out.display()),
            Err(e) => {
                failed = true;
                println!("\r{file} → détourage ✗\n{e}");
            }
        }
    }
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

/// pdfium : à côté de l'exécutable (application installée), sinon dans `src-tauri/pdfium/`.
fn set_pdfium() {
    let beside_exe = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(convert::pdf::LIBRARY)));
    let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("pdfium").join(Path::new(convert::pdf::LIBRARY).file_name().unwrap());
    convert::pdf::set_library(beside_exe.filter(|p| p.exists()).unwrap_or(dev));
}

fn main() -> ExitCode {
    set_pdfium();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, files)) = args.split_first().filter(|(_, f)| !f.is_empty()) else {
        eprintln!("usage : morph-cli targets <fichiers…> | morph-cli <format> <fichiers…>");
        return ExitCode::from(2);
    };

    if command == "targets" {
        let paths: Vec<&Path> = files.iter().map(Path::new).collect();
        println!("{}", serde_json::to_string_pretty(&formats::menu_for(&paths)).unwrap());
        return ExitCode::SUCCESS;
    }

    if command == "compress" {
        return compress_cmd(files);
    }
    if command == "trim" {
        return trim_cmd(files);
    }
    if command == "cutout" {
        return cutout_cmd(files);
    }
    if command == convert::archive::EXTRACT {
        let mut failed = false;
        for file in files {
            match convert::archive::extract(Path::new(file), &mut |_| {}) {
                Ok(out) => println!("{file} → {} ✓", out.display()),
                Err(e) => {
                    failed = true;
                    println!("{file} ✗\n{e}");
                }
            }
        }
        return if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS };
    }
    if convert::is_batch(command, files) {
        let inputs: Vec<PathBuf> = files.iter().map(PathBuf::from).collect();
        return match convert::run_batch(&inputs, command, &mut |_| {}) {
            Ok(out) => {
                println!("{} fichiers → {} ✓", files.len(), out.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                println!("{command} ✗\n{e}");
                ExitCode::FAILURE
            }
        };
    }

    // Lot d'images : plusieurs à la fois, comme dans l'application (sans pourcentage, qui
    // s'entremêlerait) ; sinon un par un avec la progression.
    let paths: Vec<&Path> = files.iter().map(Path::new).collect();
    let workers = convert::parallelism(command, &paths);
    let failed = AtomicBool::new(false);
    let next = AtomicUsize::new(0);
    let run = |progress: bool| loop {
        let i = next.fetch_add(1, Ordering::SeqCst);
        let Some(file) = files.get(i) else { break };
        let mut last = -1;
        let mut report = |p: f32| {
            let pct = (p * 100.0) as i32;
            if progress && pct != last {
                last = pct;
                print!("\r{file} → {command} : {pct:3}%");
                let _ = std::io::stdout().flush();
            }
        };
        // Contexte de sortie propre à chaque fichier : deux conversions simultanées ne
        // visent jamais le même nom (« photo.png » et « photo.jpg » → « photo.webp »).
        let result = output::scoped(output::Scope::default(), || convert::convert(&PathBuf::from(file), command, &mut report));
        match result {
            Ok(out) => println!("\r{file} → {} ✓", out.display()),
            Err(e) => {
                failed.store(true, Ordering::SeqCst);
                println!("\r{file} → {command} ✗\n{e}");
            }
        }
    };
    if workers > 1 {
        std::thread::scope(|s| {
            for _ in 0..workers {
                s.spawn(|| run(false));
            }
        });
    } else {
        run(true);
    }
    if failed.load(Ordering::SeqCst) { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

fn compress_cmd(args: &[String]) -> ExitCode {
    let level = match args[0].as_str() {
        "light" => Level::Light,
        "medium" => Level::Medium,
        "strong" => Level::Strong,
        other => {
            eprintln!("niveau inconnu : {other} (light, medium ou strong)");
            return ExitCode::from(2);
        }
    };
    let mut opts = Options { level, reduce_colors: false, target_mb: None };
    let mut files = Vec::new();
    let mut rest = args[1..].iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--colors" => opts.reduce_colors = true,
            "--target" => opts.target_mb = rest.next().and_then(|v| v.parse().ok()),
            _ => files.push(arg),
        }
    }

    let mut failed = false;
    for file in files {
        let mut last = -1;
        let mut report = |p: f32| {
            let pct = (p * 100.0) as i32;
            if pct != last {
                last = pct;
                print!("\r{file} : {pct:3}%");
                let _ = std::io::stdout().flush();
            }
        };
        match compress::compress(Path::new(file), &opts, &mut report) {
            Ok(Outcome::Saved { output, before, after }) => {
                println!("\r{file} → {} ✓ {before} → {after} octets", output.display())
            }
            Ok(Outcome::NoGain { before }) => println!("\r{file} : déjà optimisé ({before} octets), rien d'enregistré"),
            Err(e) => {
                failed = true;
                println!("\r{file} ✗\n{e}");
            }
        }
    }
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

fn trim_cmd(args: &[String]) -> ExitCode {
    let segments: Option<Vec<Segment>> = args[0]
        .split(',')
        .map(|range| {
            let (start, end) = range.split_once('-')?;
            Some(Segment { start: start.parse().ok()?, end: end.parse().ok()? })
        })
        .collect();
    let Some(segments) = segments else {
        eprintln!("passages invalides : {} (ex : 0-12.5,20-31)", args[0]);
        return ExitCode::from(2);
    };

    let mut failed = false;
    for file in &args[1..] {
        match trim::trim(Path::new(file), &segments, 1.0, &mut |_| {}) {
            Ok(out) => println!("{file} → {} ✓", out.display()),
            Err(e) => {
                failed = true;
                println!("{file} ✗\n{e}");
            }
        }
    }
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
