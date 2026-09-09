//! Rebuild when the set does.
//!
//! `include_dir!` compiles `definitions/` into the binary but tells cargo
//! nothing about it, so an edit to the set would otherwise leave a stale binary
//! describing an older one. Naming every file here keeps the build current;
//! the parity test `embedded_set_matches_definitions_dir` is what catches a set
//! edited after a binary was built (D2).

use std::path::Path;

fn main() {
    let definitions = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../definitions");
    println!("cargo:rerun-if-changed={}", definitions.display());
    watch(&definitions);
}

fn watch(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        println!("cargo:rerun-if-changed={}", path.display());
        if path.is_dir() {
            watch(&path);
        }
    }
}
