//! `scenarios/storefront/`: the authored scenario, played through its actions
//! against the real engine (19.5).
//!
//! Third tier by subject and by what it needs: a session's exit goes through
//! the reporting command, which is the real binary, and what the scenario
//! proves is the product carrying a real arc rather than one crate's own code
//! (D17, 93).

use flywheel_scenario::conformance::{self, drive, Profile, RunOptions, Status, Suite};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// The `flywheel` binary beside the test binary, built if it is not there.
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

/// `scenarios/storefront/` plays through its authored actions against the real
/// engine, and the acceptance set admits it as a scenario like any other.
///
/// It is the proof that the mechanism carries a real arc: four capture sources
/// arrive, a reader delivers what it found, twelve unmoved signals charge a
/// curation, the operator opens the intent it proposed, and the work is
/// understood before anything is planned — research that closes itself and
/// delivers a document, the finding it offers, a prototype that comes back
/// with a question and carries on when the question is answered. Every step of
/// it the machinery's own, with nothing set (19, 21, 24, 25, 58, 68, 70, 110,
/// 111, 112, 115, 116, 118, 187, 215).
#[test]
fn storefront_runs_through_its_authored_actions() {
    // A scenario whose session reports through the command needs the binary,
    // and a test is not it (D8, 93).
    std::env::set_var(
        flywheel_scenario::sessions::BINARY_ENV,
        flywheel_binary_beside_the_test(),
    );
    let path = root().join("scenarios/storefront");
    // The set the binary carries: a demo of the product runs the product's own
    // definitions, which is what a host runs (223, 224, D2).
    let options = RunOptions::default();
    let outcome = conformance::run_one(&path, &options);
    assert_eq!(
        outcome.status,
        Status::Passed,
        "storefront failed: {}\n{}",
        outcome.reason.clone().unwrap_or_default(),
        outcome.failures.join("\n")
    );
    assert_eq!(outcome.scenario, "storefront");

    // It is a scenario like any other: closed by the suite's own schema, on the
    // profile every scenario runs on, requiring nothing the phase does not
    // provide, and found by the same walk that finds the acceptance set.
    let suite = Suite::for_scenario(&path).expect("the suite closing it");
    let (scenario, document) = flywheel_atoms::conformance::load(&path).expect("it loads");
    suite.validate(&document).expect("it is valid against schema.json");
    suite.check_names(&scenario, &path).expect("every name it uses is bound");
    assert!(scenario.runs_on(Profile::GitOnly.name()));
    assert_eq!(scenario.unmet(&drive::provided()), None);
    let found = conformance::scenario_files(&root().join("scenarios"))
        .expect("the directory is walked");
    assert!(
        found.contains(&path.join("scenario.yaml")),
        "the run finds it the way it finds any scenario: {found:?}"
    );
    assert!(
        !found.iter().any(|p| p.components().any(|c| c.as_os_str() == "bundle")),
        "and its bundle is artifacts, not scenarios: {found:?}"
    );

    // And it carries the three optional parts a demo adds, all of them read
    // from the one file: the actions, the tour copy, and the bundle beside it.
    let actions = scenario.actions().expect("the actions parse");
    assert_eq!(actions.len(), 19, "stages 1 to 3 of the arc, and the thinnest construction");
    assert_eq!(scenario.tour.len(), actions.len(), "a line of copy per action");
    let bundle = flywheel_atoms::conformance::bundle_of(&path).expect("a bundle beside it");
    assert!(bundle.join("meeting/2026-09-02-storefront-weekly.vtt").is_file());
    // And the artifacts stage 3's sessions deliver are hand-authored files in
    // it, not text the runner invents: a document read inline and a page
    // opened at its own address (`scenarios/storefront.md`, the bundle).
    for artifact in [
        "research/declines.md",
        "research/retry-schedule.md",
        "prototype/retry-report.html",
    ] {
        assert!(bundle.join(artifact).is_file(), "the bundle holds {artifact}");
    }
}
