//! The acceptance and contract sets: the whole binary measured against the
//! model's conformance suite, which is the third tier's subject whether or not
//! a process is started (168, D15, D17).

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

/// The scenarios group 6 admits, on the profile each names. `status.yaml` and
/// S20 run on the git-only profile because what they assert is the committed
/// file a reader with no host running finds (D12, 145, S20).
#[test]
fn the_host_loops_acceptance_scenarios_pass() {
    for (path, profile) in [
        ("scenarios/X05.yaml", conformance::Profile::GitOnly),
        ("scenarios/S29.yaml", conformance::Profile::GitOnly),
        ("contract/status.yaml", conformance::Profile::GitOnly),
        ("scenarios/S20.yaml", conformance::Profile::GitOnly),
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
            "{path} failed ({:?}, {:?}):\n{}",
            outcome.status,
            outcome.reason,
            outcome.failures.join("\n")
        );
    }
}
/// The fourteen contract files, against a local bare state repository. This is
/// the step that admits a profile: one scenario per
/// operation of B.1, per guarantee of B.2, and one for the binding itself,
/// over a toy machine that shares no atom with the flywheel (168, task 3.15).
#[test]
fn the_contract_set_admits_the_profile() {
    let contract = conformance_dir().join("contract");
    let files: Vec<PathBuf> = {
        let mut out: Vec<PathBuf> = std::fs::read_dir(&contract)
            .expect("the contract directory is readable")
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
            .collect();
        out.sort();
        out
    };
    assert_eq!(files.len(), 14, "the contract set is fourteen files: {files:?}");
    for profile in [conformance::Profile::GitOnly] {
        for path in &files {
            let options = RunOptions {
                definitions: Some(root().join("definitions")),
                profile,
                ..Default::default()
            };
            let outcome = conformance::run_one(path, &options);
            assert_eq!(
                outcome.status,
                Status::Passed,
                "{} failed on {:?} ({:?}):\n{}",
                path.display(),
                profile,
                outcome.reason,
                outcome.failures.join("\n")
            );
        }
    }
}
