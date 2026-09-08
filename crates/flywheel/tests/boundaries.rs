//! Where a dependency may live. The state store is the one door between the
//! data plane and durable, shared storage (125), so only the crate that
//! implements it may link a storage or git library; every other crate reaches
//! durable state through the contract's operations.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The crate that implements the state store. Phase 2 adds a second beside it.
const STATE_STORE_CRATES: &[&str] = &["flywheel-store-git", "flywheel-store-tracker"];

/// Libraries that reach durable storage or a git repository directly.
const STORAGE_LIBRARIES: &[&str] = &[
    "git2",
    "gix",
    "libgit2-sys",
    "gitoxide",
    "rusqlite",
    "libsqlite3-sys",
    "sqlx",
    "diesel",
    "sled",
    "redb",
    "rocksdb",
    "mongodb",
    "tokio-postgres",
    "postgres",
    "mysql",
    "redis",
];

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn crate_manifests() -> Vec<(String, PathBuf)> {
    let crates_dir = workspace_root().join("crates");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&crates_dir).expect("crates/ is readable").flatten() {
        let manifest = entry.path().join("Cargo.toml");
        if manifest.is_file() {
            out.push((
                entry.file_name().to_string_lossy().to_string(),
                manifest,
            ));
        }
    }
    out.sort();
    out
}

/// The dependency names a manifest declares, in any of its dependency tables.
fn dependencies(manifest: &Path) -> BTreeSet<String> {
    let text = std::fs::read_to_string(manifest).expect("a manifest is readable");
    let mut names = BTreeSet::new();
    let mut in_deps = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_deps = line.contains("dependencies");
            continue;
        }
        if !in_deps || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((name, _)) = line.split_once('=') {
            names.insert(name.trim().to_string());
        }
    }
    names
}

fn storage_dependencies(names: &BTreeSet<String>) -> Vec<String> {
    names
        .iter()
        .filter(|n| STORAGE_LIBRARIES.contains(&n.as_str()))
        .cloned()
        .collect()
}

#[test]
fn storage_dependency_boundary() {
    let mut offences = Vec::new();
    for (name, manifest) in crate_manifests() {
        if STATE_STORE_CRATES.contains(&name.as_str()) {
            continue;
        }
        for dep in storage_dependencies(&dependencies(&manifest)) {
            offences.push(format!("{name} depends on {dep}"));
        }
    }
    assert!(
        offences.is_empty(),
        "only the state store crate may link a storage library (125):\n{}",
        offences.join("\n")
    );
}

#[test]
fn storage_dependency_boundary_catches_a_dependency_added_elsewhere() {
    // The check is only worth having if it fires when the dependency moves.
    let mut names = BTreeSet::new();
    names.insert("serde".to_string());
    assert!(storage_dependencies(&names).is_empty());
    names.insert("git2".to_string());
    assert_eq!(storage_dependencies(&names), vec!["git2".to_string()]);
}

#[test]
fn the_atoms_crate_depends_on_the_engine_alone() {
    // `flywheel-atoms` holds the registries and the four traits and binds none
    // of them, so it needs no workspace crate but the engine (D1, D8).
    let manifest = workspace_root().join("crates/flywheel-atoms/Cargo.toml");
    let workspace_deps: Vec<String> = dependencies(&manifest)
        .into_iter()
        .filter(|d| d.starts_with("flywheel-"))
        .collect();
    assert_eq!(workspace_deps, vec!["flywheel-engine".to_string()]);
}
