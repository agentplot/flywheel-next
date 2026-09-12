//! A row the acceptance table lists is never silently skipped: a run that held
//! one and did not play it fails, naming the row (93a; tasks 2.15, 11.4; audit
//! 3).

use crate::conformance::{phase, Outcome, RunReport, Status};
use std::path::PathBuf;

fn outcome(name: &str, status: Status, reason: &str) -> Outcome {
    Outcome {
        scenario: name.to_string(),
        path: PathBuf::from(format!("conformance/scenarios/{name}.yaml")),
        status,
        failures: vec![],
        reason: Some(reason.to_string()),
        trace: None,
    }
}

fn report(outcomes: Vec<Outcome>) -> RunReport {
    RunReport {
        outcomes,
        ..Default::default()
    }
}

#[test]
fn a_skipped_row_fails_the_run() {
    assert!(phase::accepted("S18") && !phase::accepted("S14"), "the table as this test reads it");

    // A listed row skipped is a failure, and the report names the row.
    let skipped = report(vec![
        outcome("S01", Status::Passed, ""),
        outcome("S18", Status::Skipped, "requires real-sessions"),
    ]);
    assert_eq!(skipped.exit_code(), 1, "{}", skipped.render());
    let held = skipped.listed_but_not_run();
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(held[0].0, "S18");
    assert!(held[0].1.contains("real-sessions"), "{held:?}");

    // So is one the run found not applicable: the table lists it on this
    // phase's one profile.
    let not_applicable = report(vec![outcome("S13", Status::NotApplicable, "profiles: [other]")]);
    assert_eq!(not_applicable.exit_code(), 1);

    // A row the table does not list may be skipped for a stated requirement,
    // and the run is whole (93a).
    let unlisted = report(vec![
        outcome("S01", Status::Passed, ""),
        outcome("S14", Status::Skipped, "requires real-workspace"),
    ]);
    assert_eq!(unlisted.exit_code(), 0, "{}", unlisted.render());
    assert!(unlisted.listed_but_not_run().is_empty());

    // A failure outranks a skip, and an invalid file outranks both.
    let failed = report(vec![outcome("S02", Status::Failed, ""), outcome("S18", Status::Skipped, "")]);
    assert_eq!(failed.exit_code(), 1);
    let invalid = report(vec![outcome("S02", Status::Invalid, ""), outcome("S18", Status::Skipped, "")]);
    assert_eq!(invalid.exit_code(), 2);
}

/// A row declares the mode it needs beside `profiles:`, in its own file, and
/// the runner provides that mode rather than skipping the row: `requires:
/// [real-hosts]` is met under `--hosts real` and by no in-process run (D15,
/// 93a). The schema the model mirrors here admits the value once the model
/// does; until then no shipped row can declare it.
#[test]
fn a_row_declares_the_mode_it_needs() {
    use crate::conformance::{Profile, RunOptions};
    use flywheel_atoms::conformance::Requirement;

    let parsed: Vec<Requirement> = serde_yaml::from_str("[real-hosts, real-workspace]").unwrap();
    assert_eq!(parsed, vec![Requirement::RealHosts, Requirement::RealWorkspace]);
    assert!(Requirement::RealHosts.reason().contains("processes of their own"));

    let in_process = RunOptions { profile: Profile::GitOnly, ..Default::default() };
    assert!(in_process.provides().is_empty(), "an in-process run provides no real mode");
    let real = in_process.with_real_hosts();
    assert!(real.hosts_real);
    assert_eq!(real.provides(), vec![Requirement::RealHosts]);
    // The recorded workspace and the scripted sessions are what every run has,
    // so neither is ever provided and a row requiring them is skipped (11.4).
    assert!(!real.provides().contains(&Requirement::RealWorkspace));
}
