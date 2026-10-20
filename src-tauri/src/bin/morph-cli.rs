//! Outil de test du moteur de conversion.
//!   morph-cli targets <fichiers…>   liste les formats proposés
//!   morph-cli <format> <fichiers…>  convertit chaque fichier vers <format>
//!   morph-cli compress <light|medium|strong> [--colors] [--target <Mo>] <fichiers…>

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use morph_lib::convert::{self, formats};
use morph_lib::edit::compress::{self, Level, Options, Outcome};

fn main() -> ExitCode {
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

    let mut failed = false;
    for file in files {
        let mut last = -1;
        let mut report = |p: f32| {
            let pct = (p * 100.0) as i32;
            if pct != last {
                last = pct;
                print!("\r{file} → {command} : {pct:3}%");
                let _ = std::io::stdout().flush();
            }
        };
        match convert::convert(&PathBuf::from(file), command, &mut report) {
            Ok(out) => println!("\r{file} → {} ✓", out.display()),
            Err(e) => {
                failed = true;
                println!("\r{file} → {command} ✗\n{e}");
            }
        }
    }
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
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
