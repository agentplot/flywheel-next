//! The runner's own rules, over no repository and no scenario run: the closed
//! vocabularies, the observation registry, the fixture paths, the exit codes
//! and what `requires:` reads. The first tier (D17).

use crate::conformance::{self, Profile, RunOptions, Status, Suite};
use crate::conformance::assertions;
use flywheel_atoms::conformance::{Requirement, Scenario, Step};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The smallest well-formed scenario: what a test about one rule varies.
const MINIMAL: &str = r#"
scenario: T-test
title: a scenario for the runner's own tests
profiles: [all]
satisfies: [94]
given: {}
when: [{tick: {}}]
then: {}
"#;

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn conformance_dir() -> PathBuf {
    root().join("conformance")
}

fn suite() -> Suite {
    Suite::open(&conformance_dir()).expect("the conformance suite opens")
}

fn scenario_from(yaml: &str) -> (Scenario, serde_json::Value) {
    let document: serde_json::Value = serde_yaml::from_str(yaml).expect("the yaml parses");
    let scenario: Scenario =
        serde_json::from_value(document.clone()).expect("the document is a scenario");
    (scenario, document)
}

#[test]
fn expect_keys_are_exhaustive() {
    // Every key the schema allows under `then` is a key the runner evaluates.
    // A key added to the schema and not here is what this catches.
    let schema: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(conformance_dir().join("schema.json")).unwrap())
            .unwrap();
    let allowed: Vec<&str> = schema["properties"]["then"]["properties"]
        .as_object()
        .expect("then has properties")
        .keys()
        .map(|k| k.as_str())
        .collect();
    for key in &allowed {
        assert!(
            assertions::THEN_KEYS.contains(key),
            "the schema allows `then.{key}` and the runner evaluates no such key"
        );
    }
    for key in assertions::THEN_KEYS {
        assert!(
            allowed.contains(key),
            "the runner evaluates `then.{key}`, which the schema does not allow"
        );
    }
}
#[test]
fn unbound_observation_is_an_error() {
    let suite = suite();
    let yaml = MINIMAL.replace("then: {}", "then: {state_store: {invented_key: true}}");
    let (scenario, _) = scenario_from(&yaml);
    let err = suite
        .check_names(&scenario, Path::new("conformance/scenarios/T-test.yaml"))
        .expect_err("a key observations.yaml does not bind is an error, never a silent pass");
    assert!(err.to_string().contains("invented_key"));

    // A bound key a profile the scenario runs on does not answer is the same
    // error. `items_in_tracker` is the tracker's alone, and this scenario runs
    // on every profile.
    let yaml = MINIMAL.replace("then: {}", "then: {state_store: {items_in_tracker: []}}");
    let (scenario, _) = scenario_from(&yaml);
    let err = suite
        .check_names(&scenario, Path::new("conformance/scenarios/T-test.yaml"))
        .expect_err("a key the git-only profile does not answer is an error");
    assert!(err.to_string().contains("git-only profile does not answer"));
}
#[test]
fn observations_are_parameterized() {
    // A key is host-agnostic: an observation about one host is keyed by host in
    // its value, never by a host's name in the key.
    let hosts = ["mac_mini", "mac-mini", "studio", "willdan"];
    for key in suite().observations.observations.keys() {
        for host in hosts {
            assert!(
                !key.contains(host),
                "observation `{key}` names a host; observations are keyed by host in the value"
            );
        }
    }
}
/// The run with no live service is the git-only profile against a bare
/// repository on the same computer, and it is the only one this phase binds
/// (92). A scenario that says `all` runs on it; the tracker is named in the
/// schema and is phase 2's to bind.
#[test]
fn git_only_is_the_only_profile() {
    assert_eq!(RunOptions::default().profile, Profile::GitOnly);
    assert_eq!(Profile::parse("git-only").unwrap(), Profile::GitOnly);
    for refused in ["stand-in", "tracker", "in-memory"] {
        let err = Profile::parse(refused)
            .expect_err("this release binds one profile and refuses the rest by name");
        assert!(err.to_string().contains(refused), "{err}");
    }
    // A scenario whose `profiles:` says `all` runs on it, and the run's trace
    // goes under the profile's own directory (D15).
    let (scenario, _) = scenario_from(MINIMAL);
    assert!(scenario.runs_on(Profile::GitOnly.name()));
    assert_eq!(
        RunOptions::default().trace_dir(),
        PathBuf::from("target/flywheel-trace/git-only")
    );
}
#[test]
fn host_step_vocabulary_is_closed() {
    // At most one of start, lose, disconnect, return.
    let two: BTreeMap<String, serde_json::Value> = serde_json::from_str(
        r#"{"host": {"name": "mac-mini", "lose": true, "return": true}}"#,
    )
    .unwrap();
    let step = Step::parse(&two).expect("it parses as a host step");
    let Step::Host(host) = step else { panic!("a host step") };
    let err = host
        .transition()
        .expect_err("two transitions on one host step is an error");
    assert!(err.to_string().contains("at most one"));

    // And the schema refuses it too, so it never reaches the runner.
    let yaml = MINIMAL.replace(
        "when: [{tick: {}}]",
        "when: [{host: {name: mac-mini, lose: true, return: true}}]",
    );
    let (_, document) = scenario_from(&yaml);
    assert!(
        suite().validate(&document).is_err(),
        "the schema closes the host step's vocabulary"
    );

    // A word outside the set is not a host step at all.
    let yaml = MINIMAL.replace("when: [{tick: {}}]", "when: [{host: {name: a, vanish: true}}]");
    let (_, document) = scenario_from(&yaml);
    assert!(suite().validate(&document).is_err());
}
#[test]
fn bypass_lease_refused_outside_in_process() {
    // A hook is honoured only when the runner holds the engine itself.
    assert!(RunOptions::default().honours_hooks());
    assert!(!RunOptions {
        hosts_real: true,
        ..Default::default()
    }
    .honours_hooks());

    // And a hook declared outside `contract/` is an error, because it forces a
    // race the machinery is built to prevent.
    let yaml = MINIMAL.replace("satisfies: [94]", "satisfies: [94]\nhooks: [bypass_lease]");
    let (scenario, _) = scenario_from(&yaml);
    let err = suite()
        .check_names(&scenario, Path::new("conformance/scenarios/T-test.yaml"))
        .expect_err("a hook outside contract/ is an error");
    assert!(err.to_string().contains("outside contract/"));
    // Under contract/ it is allowed.
    assert!(suite()
        .check_names(&scenario, Path::new("conformance/contract/single-writer.yaml"))
        .is_ok());
}
#[test]
fn direct_arms_require_their_fields() {
    // Each arm carries its own required fields.
    let good = r#"{"direct": {"do": "board", "issue": "42", "column": "Ready"}}"#;
    let step: BTreeMap<String, serde_json::Value> = serde_json::from_str(good).unwrap();
    assert!(matches!(Step::parse(&step), Ok(Step::Direct(_))));

    // A board move with no column is not a board move.
    let missing = r#"{"direct": {"do": "board", "issue": "42"}}"#;
    let step: BTreeMap<String, serde_json::Value> = serde_json::from_str(missing).unwrap();
    assert!(Step::parse(&step).is_err(), "a missing required field fails");

    // A mistyped key fails rather than being ignored.
    let typo = r#"{"direct": {"do": "adapter", "commandd": "flywheel capture x"}}"#;
    let step: BTreeMap<String, serde_json::Value> = serde_json::from_str(typo).unwrap();
    assert!(Step::parse(&step).is_err(), "a mistyped key fails");

    // And the schema says so first, with exit 2 behind it.
    let yaml = MINIMAL.replace(
        "when: [{tick: {}}]",
        "when: [{direct: {do: adapter, commandd: x}}]",
    );
    let (_, document) = scenario_from(&yaml);
    assert!(suite().validate(&document).is_err());
}
#[test]
fn fixture_paths_resolve_under_fixtures() {
    let suite = suite();
    assert_eq!(suite.fixtures(), conformance_dir().join("fixtures"));

    // A path naming a file there is materialized.
    let bytes = suite
        .resolve_fixture("meeting/2026-09-02-willdan-weekly.vtt", "")
        .expect("the fixture resolves");
    assert!(!bytes.is_empty());

    // A value that is not a fixture path is inline content for its key.
    let bytes = suite
        .resolve_fixture("notes/one-liner.md", "a one-line file needs no file of its own")
        .unwrap();
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "a one-line file needs no file of its own"
    );

    // A path resolving to neither is invalid.
    let err = suite
        .resolve_fixture("meeting/no-such-transcript.vtt", "")
        .expect_err("a path naming nothing is invalid");
    assert!(err.to_string().contains("resolves to no fixture"));
}
#[test]
fn requires_is_read_from_the_data() {
    // The skip is decided by the scenario file, and by no list held outside it.
    let options = RunOptions {
        definitions: Some(root().join("definitions")),
        ..Default::default()
    };
    assert!(
        options.provides().is_empty(),
        "phase 1 records the workspace and scripts the sessions (93a, 93b)"
    );

    let yaml = MINIMAL.replace("satisfies: [94]", "satisfies: [94]\nrequires: [real-workspace]");
    let (scenario, _) = scenario_from(&yaml);
    assert_eq!(
        scenario.unmet(&options.provides()),
        Some(Requirement::RealWorkspace)
    );
    assert_eq!(scenario.unmet(&[Requirement::RealWorkspace]), None);

    // Every scenario the suite skips is skipped for a stated requirement, read
    // out of the scenario file itself. Nothing is played to find that out: the
    // skip is decided before a scenario runs, which is the whole claim.
    let mut skipped: Vec<(String, Requirement)> = Vec::new();
    for file in conformance::scenario_files(&conformance_dir()).expect("the suite is readable") {
        let (scenario, _) = flywheel_atoms::conformance::load(&file).expect("a scenario loads");
        if let Some(unmet) = scenario.unmet(&options.provides()) {
            assert!(
                unmet.reason().contains("(93a)") || unmet.reason().contains("(93)"),
                "{} would be skipped without a stated requirement",
                scenario.scenario
            );
            skipped.push((scenario.scenario.clone(), unmet));
        }
    }
    assert!(!skipped.is_empty(), "the suite states requirements this phase does not provide");

    // A scenario the acceptance table lists that every configuration skips is a
    // failure, never a silent pass.
    let report = conformance::RunReport {
        outcomes: skipped
            .iter()
            .map(|(name, unmet)| conformance::Outcome {
                scenario: name.clone(),
                path: PathBuf::new(),
                status: Status::Skipped,
                failures: vec![],
                reason: Some(unmet.reason().to_string()),
                trace: None,
            })
            .collect(),
        ..Default::default()
    };
    let missing = conformance::every_listed_scenario_ran(&["S14".into()], &report);
    assert_eq!(missing, vec!["S14".to_string()]);
}
#[test]
fn exit_codes_are_four() {
    use conformance::{Outcome, RunReport};
    let outcome = |status| Outcome {
        scenario: "T".into(),
        path: PathBuf::new(),
        status,
        failures: vec![],
        reason: None,
        trace: None,
    };
    let code = |statuses: Vec<Status>| {
        RunReport {
            outcomes: statuses.into_iter().map(outcome).collect(),
            ..Default::default()
        }
        .exit_code()
    };
    assert_eq!(code(vec![Status::Passed, Status::Skipped]), 0);
    assert_eq!(code(vec![Status::Passed, Status::Failed]), 1);
    assert_eq!(code(vec![Status::Failed, Status::Invalid]), 2);
    assert_eq!(code(vec![Status::Invalid, Status::Refused]), 3);
}
#[test]
fn a_scenario_that_does_not_apply_is_not_a_skip() {
    // `profiles:` is the store binding and nothing else; the recorded-versus-
    // real axis is `requires:`.
    let options = RunOptions {
        profile: Profile::GitOnly,
        definitions: Some(root().join("definitions")),
        ..Default::default()
    };
    let (scenario, _) = scenario_from(&MINIMAL.replace("profiles: [all]", "profiles: [tracker]"));
    assert!(!scenario.runs_on(options.profile.name()));
    assert!(scenario.runs_on("tracker"));
}
/// The suite's own files sit beside the scenarios and are not ones: reading
/// `observations.yaml` as a scenario made every directory run report one
/// invalid file that no scenario named (D15).
#[test]
fn the_registry_is_not_a_scenario() {
    let files = conformance::scenario_files(&conformance_dir()).expect("the suite is readable");
    assert!(!files.is_empty());
    assert!(
        !files.iter().any(|p| p.file_name().is_some_and(|f| f == "observations.yaml")),
        "the observation registry is not a scenario"
    );
    assert!(
        files.iter().any(|p| p.file_name().is_some_and(|f| f == "S01.yaml")),
        "and the scenarios are still found"
    );
}
