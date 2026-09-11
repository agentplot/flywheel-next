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

// ---- 11.7: the 390px driver is a test dependency and of one crate alone

/// Libraries that drive a browser. The 390px pass needs one; the binary a host
/// runs must not carry one (314, D15).
const BROWSER_LIBRARIES: &[&str] = &[
    "headless_chrome",
    "fantoccini",
    "thirtyfour",
    "chromiumoxide",
    "playwright",
    "webdriver",
];

/// The crate whose tests hold the driver.
const DRIVER_CRATE: &str = "flywheel-scenario";

/// The dependency names a manifest declares, by the table they are in.
fn dependencies_by_table(manifest: &Path) -> Vec<(String, String)> {
    let text = std::fs::read_to_string(manifest).expect("a manifest is readable");
    let mut out = Vec::new();
    let mut table = String::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            table = line.trim_matches(['[', ']'].as_slice()).to_string();
            continue;
        }
        if !table.contains("dependencies") || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((name, _)) = line.split_once('=') {
            out.push((table.clone(), name.trim().to_string()));
        }
    }
    out
}

#[test]
fn driver_is_no_shipped_dependency() {
    let mut held = Vec::new();
    for (name, manifest) in crate_manifests() {
        for (table, dependency) in dependencies_by_table(&manifest) {
            if BROWSER_LIBRARIES.contains(&dependency.as_str()) {
                held.push((name.clone(), table, dependency));
            }
        }
    }
    assert!(
        !held.is_empty(),
        "the 390px pass needs a browser to drive, and no crate declares one (314)"
    );
    for (crate_name, table, dependency) in &held {
        assert_eq!(
            crate_name, DRIVER_CRATE,
            "`{dependency}` is declared by {crate_name}; the driver is {DRIVER_CRATE}'s alone (D15)"
        );
        assert_eq!(
            table, "dev-dependencies",
            "`{dependency}` is a `{table}` of {crate_name}; nothing the binary carries links a \
             browser (314, D15)"
        );
    }
}

/// The check is only worth having if it fires when the dependency moves.
#[test]
fn driver_boundary_catches_a_browser_in_a_shipped_table() {
    let manifest = workspace_root().join("crates/flywheel-scenario/Cargo.toml");
    let tables: Vec<String> = dependencies_by_table(&manifest)
        .into_iter()
        .filter(|(_, name)| BROWSER_LIBRARIES.contains(&name.as_str()))
        .map(|(table, _)| table)
        .collect();
    assert_eq!(
        tables,
        vec!["dev-dependencies".to_string()],
        "the reader tells a dev table from a shipped one"
    );
    assert!(
        dependencies_by_table(&workspace_root().join("crates/flywheel/Cargo.toml"))
            .iter()
            .all(|(_, name)| !BROWSER_LIBRARIES.contains(&name.as_str())),
        "the binary links no browser"
    );
}
