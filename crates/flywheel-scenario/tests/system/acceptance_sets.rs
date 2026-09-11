//! The acceptance sets played against the real `flywheel` binary: the sessions
//! a scenario scripts are its processes, so these are the system tier (D17).

use flywheel_scenario::conformance::{self, Profile, RunOptions, Status};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn conformance_dir() -> PathBuf {
    root().join("conformance")
}

/// The `flywheel` binary beside the test binary. `cargo test -p
/// flywheel-scenario` does not build another package's binary, so build it when
/// it is not there.
fn flywheel_binary_beside_the_test() -> PathBuf {
    let deps = std::env::current_exe().expect("the test binary has a path");
    let target = deps.parent().and_then(|p| p.parent()).expect("target/debug");
    let binary = target.join("flywheel");
    if !binary.exists() {
        let out = std::process::Command::new(env!("CARGO"))
            .args(["build", "--quiet", "-p", "flywheel"])
            .current_dir(root())
            .output()
            .expect("building the flywheel binary");
        assert!(
            binary.exists(),
            "no flywheel binary at {}: {}",
            binary.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    binary
}

/// The scenarios group 10 admits: the rail derived, numbered and retracted, a
/// standing session offered rather than ended, a proposal that created nothing,
/// a thread closed on the operator's response, and — against a real state
/// repository — the rail identical after a restart and a slow host starting one
/// session (tasks 10.7–10.11).
#[test]
fn the_rail_acceptance_scenarios_pass() {
    // A scenario whose script reports through the command needs the binary,
    // and a test is not it (D8).
    std::env::set_var(
        flywheel_scenario::sessions::BINARY_ENV,
        flywheel_binary_beside_the_test(),
    );
    for (path, profile) in [
        ("scenarios/S01.yaml", conformance::Profile::GitOnly),
        ("scenarios/S02.yaml", conformance::Profile::GitOnly),
        ("scenarios/S04.yaml", conformance::Profile::GitOnly),
        ("scenarios/S07.yaml", conformance::Profile::GitOnly),
        ("scenarios/S05.yaml", conformance::Profile::GitOnly),
        ("scenarios/S06.yaml", conformance::Profile::GitOnly),
    ] {
        let options = RunOptions {
            definitions: Some(root().join("definitions")),
            profile,
            ..Default::default()
        };
        let outcome = conformance::run_one(&conformance_dir().join(path), &options);
        assert_eq!(
            outcome.status,
            Status::Passed,
            "{path} failed on {profile:?} ({:?}):\n{}",
            outcome.reason,
            outcome.failures.join("\n")
        );
    }
}
/// The three the whole-set run found, kept where a change that broke them
/// again would be caught: a decision on an object a tick created is numbered in
/// that tick on either profile (15, D15), a pane a scenario killed by its own
/// name is read as gone (196, X08), and a capture that stands with its proof
/// absent acts once per tick (73, 127, S21).
#[test]
fn the_whole_set_acceptance_scenarios_pass() {
    std::env::set_var(
        flywheel_scenario::sessions::BINARY_ENV,
        flywheel_binary_beside_the_test(),
    );
    for (path, profile) in [
        ("scenarios/S04.yaml", Profile::GitOnly),
        ("scenarios/X08.yaml", Profile::GitOnly),
        ("scenarios/S21.yaml", Profile::GitOnly),
        ("scenarios/S16.yaml", Profile::GitOnly),
    ] {
        let options = RunOptions {
            definitions: Some(root().join("definitions")),
            profile,
            ..Default::default()
        };
        let outcome = conformance::run_one(&conformance_dir().join(path), &options);
        assert_eq!(
            outcome.status,
            Status::Passed,
            "{path} failed on {profile:?} ({:?}):\n{}",
            outcome.reason,
            outcome.failures.join("\n")
        );
    }
}
