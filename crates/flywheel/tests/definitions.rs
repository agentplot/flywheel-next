//! The set the binary carries, and who may override it (223, 224, D2).

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
