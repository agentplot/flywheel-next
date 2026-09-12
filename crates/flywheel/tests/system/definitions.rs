//! The set the binary carries, and who may override it (223, 224, D2).
//!
//! Every one of these shells out to the `flywheel` binary, so they belong to
//! the group gate rather than to the run a person makes every few minutes
//! (AGENTS.md — Gates).

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn run(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_flywheel"))
        .current_dir(root())
        .args(args)
        .output()
        .expect("the command runs");
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.success(), text)
}

/// A host runs the definitions in the binary, so the set it ran is provable
/// from the bytes. The directory load is the scenario runner's alone.
#[test]
fn host_refuses_definitions_override() {
    for args in [
        ["host", "doctor", "--definitions", "definitions"],
        ["host", "--definitions", "definitions", "doctor"],
    ] {
        let (ok, text) = run(&args);
        assert!(!ok, "the host must refuse an override: {text}");
        assert!(
            text.contains("refused") && text.contains("223"),
            "the refusal names the rule and the clause: {text}"
        );
    }
    let (ok, _) = run(&["host", "doctor"]);
    assert!(ok, "a host with no override runs");
}

/// The scenario runner takes the override: it is what the parity test and a
/// scenario naming its own machines are for (D2, D15).
#[test]
fn the_scenario_runner_takes_the_override() {
    let (ok, text) = run(&[
        "scenario",
        "run",
        "--definitions",
        "definitions",
        "conformance/contract/read.yaml",
    ]);
    assert!(ok, "the runner accepts --definitions: {text}");
}

/// `version --definitions` prints the set version and every core machine's
/// version (224).
#[test]
fn version_prints_the_set_and_every_core_machine() {
    let (ok, text) = run(&["version", "--definitions"]);
    assert!(ok, "{text}");
    assert!(text.contains("definition set"), "{text}");
    assert!(text.contains("instance"), "the core machines are listed: {text}");
    let listed = text.lines().filter(|l| l.contains("version ")).count();
    assert!(listed > 20, "every core machine is listed, not a few: {text}");
}

/// A host reads the instance's own types from a blueprints checkout, and a file
/// that would override a core machine is reported rather than swallowed (57,
/// 85, 223).
#[test]
fn the_host_reports_what_the_blueprints_offered_and_what_it_refused() {
    let (ok, text) = run(&[
        "host",
        "doctor",
        "--blueprints",
        "crates/flywheel-domain/tests/fixtures/blueprints",
    ]);
    assert!(ok, "{text}");
    assert!(text.contains("1 types of its own"), "{text}");
    assert!(!text.contains("refused:"), "nothing in the fixture is refused: {text}");

    let dir = std::env::temp_dir().join(format!("flywheel-doctor-{}", std::process::id()));
    let types = dir.join("flywheel/unit-types");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&types).unwrap();
    std::fs::write(
        types.join("instance@99.yaml"),
        "machine: instance\nversion: 99\nkind: object\ntier: core\nregions: {}\n",
    )
    .unwrap();
    let (ok, text) = run(&["host", "doctor", "--blueprints", &dir.to_string_lossy()]);
    assert!(ok, "a refusal is reported, not a crash: {text}");
    assert!(text.contains("refused:") && text.contains("instance"), "{text}");
    assert!(text.contains("223"), "the refusal names the rule: {text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Where the model lives on this machine: `FLYWHEEL_BLUEPRINTS` where it is
/// set, the sibling checkout AGENTS.md names otherwise. A gate on the model
/// that cannot find the model fails and says so; it never passes by skipping
/// (audit 18).
fn model() -> PathBuf {
    match std::env::var("FLYWHEEL_BLUEPRINTS") {
        Ok(set) => PathBuf::from(set),
        Err(_) => root().join("../../blueprints/main/design/flywheel-next/models/statechart"),
    }
}

/// Every file under a directory, relative to it, in order.
fn files_under(dir: &std::path::Path) -> Vec<PathBuf> {
    fn walk(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, base, out);
            } else if path.file_name().is_some_and(|n| n != ".DS_Store") {
                out.push(path.strip_prefix(base).unwrap().to_path_buf());
            }
        }
    }
    let mut out = vec![];
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// The byte-for-byte differences between a mirror and its source, named.
fn mirror_differences(mirror: &std::path::Path, source: &std::path::Path, skip: &[&str]) -> Vec<String> {
    let mut out = vec![];
    let ours: Vec<PathBuf> = files_under(mirror)
        .into_iter()
        .filter(|p| !skip.iter().any(|s| p.starts_with(s)))
        .collect();
    let theirs = files_under(source);
    for path in &ours {
        match std::fs::read(source.join(path)) {
            Ok(bytes) if bytes == std::fs::read(mirror.join(path)).unwrap() => {}
            Ok(_) => out.push(format!("{} differs from the model's", path.display())),
            Err(_) => out.push(format!("{} is not in the model", path.display())),
        }
    }
    for path in &theirs {
        if !ours.contains(path) {
            out.push(format!("the model's {} is not mirrored", path.display()));
        }
    }
    out
}

/// Requirement 83's checker — every element of a diagram tied to a definition,
/// every clause cited, every decision kind with one creating state — runs in a
/// gate, over the set this binary mirrors: the model's `check.py` passes in the
/// model tree, and `definitions/` and `conformance/` here are byte-for-byte
/// the model's `machines/`, `profiles/` and `conformance/`, so what it checked
/// is what this binary carries (83, AGENTS.md — where the design lives; audit
/// 7).
///
/// The checker resolves its paths from its own location — `../diagrams/*.svg`,
/// `../../../requirements.md` — so it runs where it lives and the mirror is
/// asserted equal to it, rather than the copy here being run against paths it
/// cannot find. It needs `pyyaml` and `jsonschema`, which `uv` supplies.
#[test]
fn the_models_checker_passes_over_the_set_this_binary_mirrors() {
    let model = model();
    assert!(
        model.join("machines/check.py").is_file(),
        "the model is not at {} (set FLYWHEEL_BLUEPRINTS to the statechart directory)",
        model.display()
    );
    let out = Command::new("uv")
        .args(["run", "--no-project", "--with", "pyyaml", "--with", "jsonschema", "python3", "check.py"])
        .current_dir(model.join("machines"))
        .output()
        .expect("uv runs the model's checker");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "check.py failed:\n{text}");
    assert!(!text.contains("FAIL"), "check.py reported a failure:\n{text}");
    // The diagram pass says nothing when every picture agrees with the
    // definitions, so what makes it a pass and not a no-op is that there were
    // pictures to check (83).
    let diagrams = files_under(&model.join("diagrams"))
        .into_iter()
        .filter(|p| p.extension().is_some_and(|x| x == "svg") && p.parent().is_some_and(|d| d.as_os_str().is_empty()))
        .count();
    assert!(diagrams > 0, "the model has no diagrams for the checker to tie to its definitions");

    let mut differences = mirror_differences(&root().join("definitions"), &model.join("machines"), &["profiles"]);
    differences.extend(mirror_differences(&root().join("definitions/profiles"), &model.join("profiles"), &[]));
    differences.extend(mirror_differences(&root().join("conformance"), &model.join("conformance"), &[]));
    assert!(
        differences.is_empty(),
        "the mirror is not the model; copy from the model, never edit here (AGENTS.md):\n{}",
        differences.join("\n")
    );
}
