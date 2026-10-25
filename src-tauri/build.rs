use std::path::Path;

fn main() {
    locales();
    tauri_build::build()
}

/// Liste des catalogues de langue (`../locales/*.json`), intégrés au binaire par `i18n.rs`.
fn locales() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../locales");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("dossier locales introuvable")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    entries.sort();
    let items: Vec<String> = entries
        .iter()
        .map(|p| {
            let code = p.file_stem().unwrap().to_string_lossy();
            format!("({code:?}, include_str!({:?}))", p.display().to_string())
        })
        .collect();
    let code = format!("static CATALOGS: &[(&str, &str)] = &[{}];\n", items.join(", "));
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("locales.rs");
    std::fs::write(out, code).unwrap();
}
