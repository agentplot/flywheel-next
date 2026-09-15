//! The acceptance and contract sets: the whole binary measured against the
//! model's conformance suite, which is the third tier's subject whether or not
//! a process is started (168, D15, D17).

use super::phase;
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

/// Every scenario of the suite, played on the phase's one profile in one run,
/// and the gate over it: each row the acceptance table lists asserted through
/// the runner and passed, none skipped, found not applicable or missing from
/// the run (roadmap, phase gates; 168, 93a, D15; tasks 2.15 and 11.4; audit 3,
/// 18).
///
/// The set is enumerated from `conformance/scenarios/` and no list of what to
/// play is kept here: a scenario that joins the suite joins this run, and what
/// a run skips is decided by the scenario file alone. The rows the phase defers
/// are played too and reported with what they expected and what they got; one
/// that fails or does not validate gates nothing until its phase opens.
///
/// This one driver is what stood as three hard-coded lists — the rail
/// scenarios, the whole-set regressions and the host loop's — each of which
/// named a subset and left nine listed rows asserted by nothing.
#[test]
fn every_acceptance_row_passes() {
    // A scenario whose script reports through the command needs the binary,
    // and a test is not it (D8).
    std::env::set_var(
        flywheel_scenario::sessions::BINARY_ENV,
        flywheel_binary_beside_the_test(),
    );
    let options = RunOptions {
        definitions: Some(root().join("definitions")),
        profile: Profile::GitOnly,
        ..Default::default()
    };
    let report = conformance::run(&[conformance_dir().join("scenarios")], &options)
        .expect("the suite runs");
    let rendered = report.render();
    // The report is the run's output, the deferred rows in it.
    println!("{rendered}");

    // Every listed row is in the run, by the name its file has, and passed.
    let listed: Vec<String> = phase::ACCEPTED.iter().map(|s| s.to_string()).collect();
    let missing = conformance::every_listed_scenario_ran(&listed, &report);
    let did_not_play = report.listed_but_not_run();
    let failed: Vec<String> = report
        .outcomes
        .iter()
        .filter(|o| o.status == Status::Failed && phase::accepted(&o.row()))
        .map(|o| format!("{}:\n{}", o.line(), o.failures.join("\n")))
        .collect();
    let passed = report
        .outcomes
        .iter()
        .filter(|o| o.status == Status::Passed && phase::accepted(&o.row()))
        .count();
    assert!(
        missing.is_empty()
            && did_not_play.is_empty()
            && failed.is_empty()
            && passed == listed.len()
            && report.exit_code() == 0,
        "the gate is not green (exit {}): {passed} of {} listed rows passed\n\n\
         listed rows missing from the run: {missing:?}\n\
         listed rows that did not play: {did_not_play:?}\n\nfailed rows:\n{}\n\n{rendered}",
        report.exit_code(),
        listed.len(),
        failed.join("\n\n")
    );
    assert!(
        rendered.contains(&format!("gate      {0} of {0} listed rows passed\n", listed.len())),
        "the report says the gate held: {rendered}"
    );

    // Nothing played is left out of the report: every scenario of the suite has
    // its line, and each deferred row that did not pass is named beside the
    // gate with what it expected and what it got.
    let files = conformance::scenario_files(&conformance_dir().join("scenarios"))
        .expect("the suite is readable");
    assert_eq!(report.outcomes.len(), files.len(), "every scenario played: {rendered}");
    for o in report.deferred_not_passing() {
        assert!(rendered.contains(&o.line()), "{} has its line: {rendered}", o.row());
        for failure in &o.failures {
            assert!(rendered.contains(failure.as_str()), "{} says what it got: {rendered}", o.row());
        }
        let named = match o.status {
            Status::Failed => format!("{} failed", o.row()),
            _ => format!("{} does not validate", o.row()),
        };
        assert!(
            rendered.lines().any(|l| l.starts_with("deferred") && l.contains(&named)),
            "{named} is named beside the gate: {rendered}"
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
