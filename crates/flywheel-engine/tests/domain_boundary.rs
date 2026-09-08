//! The engine names nothing of the domain (86, 119, I13).
//!
//! The rule: over `crates/flywheel-engine/src/**` and `crates/flywheel-engine/
//! tests/**`, none of the seven object names of 86 appears as a whole word, no
//! name from `definitions/atoms.yaml` appears, and no instruction text is
//! carried. The engine does name rail, decision, response, host, lease, sink
//! and tick, because it owns the register, the decision derivation and the five
//! engine machines (model.md §2.5); and it names the five evidence atoms every
//! object has by virtue of being an object.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The five evidence names the engine provides for every object, which
/// model.md §2.5 leaves in the engine's own vocabulary.
const ENGINE_PROVIDED: &[&str] = &["state", "entered_at", "seq", "applied_responses", "now"];

/// The seven object names of 86.
const OBJECT_NAMES: &[&str] = &[
    "intent",
    "elaboration",
    "bolt",
    "unit",
    "work item",
    "claim",
    "verdict",
];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for sub in ["src", "tests"] {
        collect(&crate_dir().join(sub), &mut out);
    }
    out.sort();
    out
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            // The check states the names it forbids, so it names them itself.
            if p.file_name().is_some_and(|f| f == "domain_boundary.rs") {
                continue;
            }
            out.push(p);
        }
    }
}

fn atom_names() -> BTreeSet<String> {
    let path = crate_dir().join("../../definitions/atoms.yaml");
    let text = std::fs::read_to_string(path).expect("definitions/atoms.yaml is readable");
    let file: serde_yaml::Value = serde_yaml::from_str(&text).expect("atoms.yaml parses");
    let mut names = BTreeSet::new();
    for section in ["evidence", "effects"] {
        let map = file[section].as_mapping().expect("a mapping");
        for key in map.keys() {
            let name = key.as_str().expect("a name");
            if section == "evidence" && ENGINE_PROVIDED.contains(&name) {
                continue;
            }
            names.insert(name.to_string());
        }
    }
    names
}

/// `name` as a whole word: not preceded or followed by a word character.
fn names_it(line: &str, name: &str) -> bool {
    let bytes = line.as_bytes();
    let word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut from = 0;
    while let Some(at) = line[from..].find(name) {
        let start = from + at;
        let end = start + name.len();
        let before_ok = start == 0 || !word(bytes[start - 1]);
        let after_ok = end == bytes.len() || !word(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

fn offences(needles: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for file in rust_files() {
        let text = std::fs::read_to_string(&file).expect("a source file is readable");
        for (n, line) in text.lines().enumerate() {
            for needle in needles {
                if names_it(line, needle) {
                    out.push(format!(
                        "{}:{}: {needle}: {}",
                        file.file_name().unwrap().to_string_lossy(),
                        n + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    out
}

#[test]
fn domain_boundary() {
    let mut needles: Vec<String> = OBJECT_NAMES.iter().map(|s| s.to_string()).collect();
    needles.extend(atom_names());
    let found = offences(&needles);
    assert!(
        found.is_empty(),
        "the engine names the domain:\n{}",
        found.join("\n")
    );
}

#[test]
fn domain_boundary_fails_on_a_reintroduced_name() {
    // The check is only worth having if it catches a name put back.
    assert!(names_it("let x = obj.bolt;", "bolt"));
    assert!(names_it("// a claim", "claim"));
    assert!(names_it("prepare_place(&p)", "prepare_place"));
    // and only whole words
    assert!(!names_it("let bolted = 1;", "bolt"));
    assert!(!names_it("reunited", "unit"));
    assert!(!names_it("let (num, suffix) = s.split_at(n);", "unit"));
}

#[test]
fn no_instruction_text_is_carried() {
    // No engine behaviour depends on an instruction's wording, and no
    // instruction reaches the engine at all (119).
    let mut found = Vec::new();
    for file in rust_files() {
        let text = std::fs::read_to_string(&file).expect("a source file is readable");
        for (n, line) in text.lines().enumerate() {
            for needle in ["instructions/", "skills/", "include_str!", "include_dir!"] {
                if line.contains(needle) {
                    found.push(format!(
                        "{}:{}: {needle}",
                        file.file_name().unwrap().to_string_lossy(),
                        n + 1
                    ));
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "the engine carries instruction text:\n{}",
        found.join("\n")
    );
}
