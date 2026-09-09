//! What this phase runs, what it skips and what it defers (93a, D15).
//!
//! The skip is decided by the scenario file and by no list held outside it: a
//! scenario says what it needs beyond the store binding, and a run whose
//! bindings do not provide it skips the scenario and says why. What the phase
//! *lists* is the acceptance table of proposal.md, and it is here because a
//! scenario the table lists that every configuration skips is a failure and
//! never a silent pass.

mod phase;

use flywheel_atoms::conformance::{Requirement, Scenario};
pub use phase::{ACCEPTED, DEFERRED, REAL_WORKSPACE};
use flywheel_scenario::conformance::{Profile, RunOptions};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}


/// Every scenario of the suite, named as its file is — which is how the phase's
/// acceptance table and this repository's tasks name them — with what it says it
/// requires.
fn scenarios() -> Vec<(String, Scenario)> {
    let dir = root().join("conformance/scenarios");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("the scenarios are readable").flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|x| x != "yaml") {
            continue;
        }
        let (scenario, _) =
            flywheel_atoms::conformance::load(&path).expect("every scenario file loads");
        let name = path
            .file_stem()
            .expect("a scenario file has a name")
            .to_string_lossy()
            .to_string();
        out.push((name, scenario));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn recorded_workspace_excludes_nine() {
    // Phase 1 records the line-and-place effects and scripts the sessions, so
    // it provides neither requirement (93a, 93b, D8).
    let options = RunOptions {
        profile: Profile::GitOnly,
        ..Default::default()
    };
    let provided = options.provides();
    assert!(
        provided.is_empty(),
        "this phase provides no requirement beyond the store binding (93a, 93b)"
    );

    let scenarios = scenarios();
    let skipped: Vec<&str> = scenarios
        .iter()
        .filter(|(_, s)| s.unmet(&provided).is_some())
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(
        skipped, REAL_WORKSPACE,
        "the skipped set is what the scenario files require and nothing else"
    );
    for name in REAL_WORKSPACE {
        let (_, scenario) = scenarios
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("{name} is a scenario of the suite"));
        assert_eq!(
            scenario.unmet(&provided),
            Some(Requirement::RealWorkspace),
            "{name} is skipped for the workspace it needs, and the reason says so"
        );
    }

    // The twenty-one the acceptance table lists are excluded by nothing: every
    // one applies to this store binding and requires nothing this phase does
    // not provide, so a run of them is a run and never a silent pass (D15).
    for name in ACCEPTED {
        let (_, scenario) = scenarios
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("{name} is a scenario of the suite"));
        assert_eq!(
            scenario.unmet(&provided),
            None,
            "{name} is in the acceptance table and would be skipped"
        );
        assert!(
            scenario.runs_on(options.profile.name()) || scenario.runs_on("stand-in"),
            "{name} is in the acceptance table and applies to no path this phase runs"
        );
    }

    // And the fourteen the phase defers are deferred for their own reasons —
    // the pane runner, the tracker, the ledger, the dispatcher, construction —
    // which is not a requirement a binding could provide. They carry none.
    for name in DEFERRED {
        let (_, scenario) = scenarios
            .iter()
            .find(|(n, _)| n == name)
            .unwrap_or_else(|| panic!("{name} is a scenario of the suite"));
        assert_eq!(
            scenario.unmet(&provided),
            None,
            "{name} is deferred for a reason of the phase's own, not for a binding's"
        );
        assert!(
            !ACCEPTED.contains(name),
            "{name} is both deferred and accepted"
        );
    }

    // Every scenario of the suite is in exactly one of the three.
    for (name, _) in &scenarios {
        let listed = ACCEPTED.contains(&name.as_str()) as usize
            + REAL_WORKSPACE.contains(&name.as_str()) as usize
            + DEFERRED.contains(&name.as_str()) as usize;
        assert_eq!(listed, 1, "{name} is in {listed} of the three sets");
    }
}

#[test]
fn the_skipped_set_is_read_from_the_data() {
    // A scenario that stopped requiring a real workspace would stop being
    // skipped, with no list here to keep in step: the runner reads `requires:`
    // and nothing else (D15).
    let mut scenario = flywheel_atoms::conformance::load(
        &root().join("conformance/scenarios/S14.yaml"),
    )
    .expect("S14 loads")
    .0;
    assert_eq!(scenario.unmet(&[]), Some(Requirement::RealWorkspace));
    assert_eq!(scenario.unmet(&[Requirement::RealWorkspace]), None);
    scenario.requires.clear();
    assert_eq!(scenario.unmet(&[]), None);
}

