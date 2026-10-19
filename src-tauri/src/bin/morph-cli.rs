//! Outil de test du moteur de conversion.
//!   morph-cli targets <fichiers…>   liste les formats proposés
//!   morph-cli <format> <fichiers…>  convertit chaque fichier vers <format>

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use morph_lib::convert::{self, formats};

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
