//! The set the binary carries is the set the repository holds (D2, 223, 224).

use std::path::{Path, PathBuf};

fn definitions_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../definitions")
}

/// The parity test D2 names: what was compiled in equals what is on disk. A
/// hand edit to `definitions/` that no rebuild picked up fails here rather
/// than silently becoming the machine that ran.
#[test]
fn embedded_set_matches_definitions_dir() {
    let on_disk = flywheel_domain::set::digest_of_dir(&definitions_dir())
        .expect("definitions/ is readable");
    assert_eq!(
        flywheel_domain::set::digest(),
        on_disk,
        "the embedded set is not definitions/ on disk; rebuild after editing it"
    );
}

/// An edit the build did not see is caught: the digest is over the bytes, so
/// one byte more in one file is a different set.
#[test]
fn an_unbuilt_edit_is_a_different_set() {
    let dir = std::env::temp_dir().join(format!("flywheel-set-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut copied = 0usize;
    for entry in walk(&definitions_dir()) {
        let relative = entry.strip_prefix(definitions_dir()).unwrap().to_path_buf();
        let target = dir.join(&relative);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(&entry, &target).unwrap();
        copied += 1;
    }
    assert!(copied > 0, "definitions/ holds files");
    assert_eq!(
        flywheel_domain::set::digest_of_dir(&dir).unwrap(),
        flywheel_domain::set::digest(),
        "a faithful copy is the same set"
    );
    let edited = dir.join("atoms.yaml");
    let mut text = std::fs::read_to_string(&edited).unwrap();
    text.push_str("\n# a hand edit\n");
    std::fs::write(&edited, text).unwrap();
    assert_ne!(
        flywheel_domain::set::digest_of_dir(&dir).unwrap(),
        flywheel_domain::set::digest(),
        "an edited directory is a different set"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every core machine carries a version, and the set carries one of its own
/// (224, 208).
#[test]
fn every_core_machine_carries_a_version() {
    let versions = flywheel_domain::set::versions().expect("the embedded set parses");
    assert!(versions.len() > 20, "the set holds the core machines");
    assert!(versions.iter().all(|(_, v)| *v >= 1));
    assert!(versions.iter().any(|(name, _)| name == "instance"));
    assert!(!flywheel_domain::set::SET_VERSION.is_empty());
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out
}
