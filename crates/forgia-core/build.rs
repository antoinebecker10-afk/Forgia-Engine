//! Génère le pack de définitions embarqué pour la cible web (story-695).
//!
//! wasm n'a pas de filesystem : chaque lecteur `std::fs` de genome tombe en
//! défauts SILENCIEUX (équipement désactivé → avatar absent, 0 cluster
//! champignons → re-spawn chaque frame, etc. — constaté 2026-08-11). Ce script
//! liste assets/genomes/**.toml + assets/registry/*.ron et émet un tableau
//! statique `(clé, include_str!)` consommé par `forgia_core::def_io` sur wasm.
//!
//! Les CONTENUS sont trackés par rustc via include_str! (dep-info) ; ce script
//! ne re-tourne que si la LISTE de fichiers change (rerun-if-changed dossiers).
//! Sur cible native le fichier généré n'est jamais compilé (module cfg wasm) :
//! zéro poids binaire, zéro rebuild sur édition de TOML.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn collect(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, ext, out);
        } else if path.extension().and_then(|x| x.to_str()) == Some(ext) {
            out.push(path);
        }
    }
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let assets = manifest.join("../../assets");
    let mut files = Vec::new();
    collect(&assets.join("genomes"), "toml", &mut files);
    collect(&assets.join("registry"), "ron", &mut files);
    files.sort();

    let mut src = String::from(
        "/// (chemin relatif à assets/, contenu) — généré par forgia-core/build.rs.\n\
         pub static WEB_DEFS: &[(&str, &str)] = &[\n",
    );
    for file in &files {
        let key = file
            .strip_prefix(&assets)
            .expect("fichier collecté hors assets/")
            .to_string_lossy()
            .replace('\\', "/");
        // Chemin absolu en slashes (PAS canonicalize : le préfixe UNC \\?\ de
        // Windows est indigeste pour include_str!).
        let abs = file.to_string_lossy().replace('\\', "/");
        writeln!(src, "    ({key:?}, include_str!({abs:?})),").unwrap();
    }
    src.push_str("];\n");

    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out.join("web_defs.rs"), src).expect("écriture web_defs.rs");
    println!("cargo:rerun-if-changed=../../assets/genomes");
    println!("cargo:rerun-if-changed=../../assets/registry");
}
