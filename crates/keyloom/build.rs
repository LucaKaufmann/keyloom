//! Embeds `data/keycodes.json` and `data/guides/*.md` (spec 07). Missing files embed as empty, so
//! the crate builds before the data exists.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let out = std::env::var("OUT_DIR").unwrap();
    let data = Path::new(&manifest).join("data");
    println!("cargo:rerun-if-changed=data");

    let keycodes = std::fs::read_to_string(data.join("keycodes.json")).unwrap_or_else(|_| "[]".into());
    std::fs::write(Path::new(&out).join("keycodes.json"), keycodes).unwrap();

    let mut guides: Vec<_> = std::fs::read_dir(data.join("guides"))
        .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "md")).collect())
        .unwrap_or_default();
    guides.sort();
    let mut code = String::from("pub const GUIDES: &[(&str, &str)] = &[\n");
    for path in guides {
        println!("cargo:rerun-if-changed={}", path.display());
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        writeln!(code, "    ({name:?}, include_str!({:?})),", path.display().to_string()).unwrap();
    }
    code.push_str("];\n");
    std::fs::write(Path::new(&out).join("guides.rs"), code).unwrap();
}
