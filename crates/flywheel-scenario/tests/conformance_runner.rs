//! The runner's own rules: the closed vocabularies, the virtual clock, the
//! observation registry, the trace, the exit codes.

use flywheel_atoms::conformance::{Requirement, Scenario, Step};
use flywheel_scenario::conformance::{self, assertions, drive, trace, Profile, RunOptions, Status, Suite};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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

const MINIMAL: &str = r#"
scenario: T-test
title: a scenario for the runner's own tests
profiles: [all]
satisfies: [94]
given: {}
when: [{tick: {}}]
then: {}
"#;

// ---- 2.6 the runner's core

#[test]
fn the_runner_plays_s01_against_the_real_engine() {
    let options = RunOptions {
        definitions: Some(root().join("definitions")),
        ..Default::default()
    };
    let outcome = conformance::run_one(&conformance_dir().join("scenarios/S01.yaml"), &options);
    // Every clause of S01 holds: the approval, the place prepared and proven
    // current, the session started, and the register's numbers across the
    // restart in its own steps. The chain settles inside the ticks the `when`
    // list has, because a region's move is its siblings' to read on the next
    // tick and the scenario's steps are counted that way (model.md, the tick).
    assert_eq!(
        outcome.status,
        Status::Passed,
        "S01 failed:\n{}",
        outcome.failures.join("\n")
    );
}

// ---- 2.7 every `then` clause is evaluated

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

// ---- 2.8 the observation registry

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

// ---- 2.9 the trace

#[test]
fn trace_lists_all_six() {
    let options = RunOptions {
        definitions: Some(root().join("definitions")),
        trace: Some(std::env::temp_dir().join("flywheel-trace-test")),
        tracing: true,
        ..Default::default()
    };
    let _ = std::fs::remove_dir_all(options.trace_dir());
    let outcome = conformance::run_one(&conformance_dir().join("scenarios/S01.yaml"), &options);
    let json = outcome.trace.expect("a trace was written");
    assert!(json.to_string_lossy().ends_with(".trace.json"));
    let text = std::fs::read_to_string(&json).unwrap();
    let written: trace::Trace = serde_json::from_str(&text).unwrap();
    assert_eq!(
        written.lists(),
        vec!["ticks", "guards", "transitions", "effects", "decisions", "numbers"]
    );
    // and the trace actually holds each of them
    assert!(!written.ticks.is_empty(), "ticks");
    assert!(written.ticks.iter().any(|t| !t.guards.is_empty()), "guards");
    assert!(written.ticks.iter().any(|t| !t.transitions.is_empty()), "transitions");
    assert!(written.ticks.iter().any(|t| !t.effects.is_empty()), "effects");
    assert!(
        written.decisions_after.iter().any(|d| !d.is_empty()),
        "decisions"
    );
    assert!(
        written
            .decisions_after
            .iter()
            .flatten()
            .any(|d| d.number.is_some()),
        "numbers"
    );
}

#[test]
fn trace_md_is_rendered_from_trace_json() {
    let dir = std::env::temp_dir().join("flywheel-trace-render");
    let _ = std::fs::remove_dir_all(&dir);
    let options = RunOptions {
        definitions: Some(root().join("definitions")),
        trace: Some(dir.clone()),
        tracing: true,
        ..Default::default()
    };
    let outcome = conformance::run_one(&conformance_dir().join("scenarios/S01.yaml"), &options);
    let json = outcome.trace.expect("a trace was written");
    let md = json.with_extension("md");
    assert!(md.exists(), "the document a person reads is written beside it");

    // The document is rendered from the file, so changing the file changes the
    // document: they cannot diverge (95).
    let mut written: trace::Trace =
        serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
    written.title = "a different title".into();
    std::fs::write(&json, serde_json::to_string_pretty(&written).unwrap()).unwrap();
    trace::render_from(&json).unwrap();
    assert!(std::fs::read_to_string(&md).unwrap().contains("a different title"));

    // And it goes under the profile's own directory by default, never beside
    // the scenario files.
    let default = RunOptions::default().trace_dir();
    assert_eq!(default, PathBuf::from("target/flywheel-trace/git-only"));
}

// ---- 13.1 one profile

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

// ---- 2.10 the virtual clock

#[test]
fn clock_moves_only_on_clock_and_tick() {
    let options = RunOptions {
        definitions: Some(root().join("definitions")),
        ..Default::default()
    };
    let suite = suite();
    let yaml = MINIMAL.replace(
        "when: [{tick: {}}]",
        "when: [{evidence: {}}, {clock: {advance: 6m}}, {evidence: {}}, {tick: {}}]",
    );
    let (scenario, _) = scenario_from(&yaml);
    let run = drive::play(
        &scenario,
        &conformance_dir().join("scenarios/T-test.yaml"),
        &suite,
        &options,
    )
    .expect("the scenario plays");
    // Two evidence steps moved nothing; the clock step moved 6m and the tick
    // one interval.
    let expected = drive::start_of_time() + chrono::Duration::minutes(6) + options.interval;
    assert_eq!(run.runtime.store.now, expected);
}

#[test]
fn wall_clock_never_reaches_a_guard() {
    // Two runs of one scenario are the same run, whatever the wall clock says.
    let options = RunOptions {
        definitions: Some(root().join("definitions")),
        ..Default::default()
    };
    let path = conformance_dir().join("scenarios/S01.yaml");
    let (scenario, _) = flywheel_atoms::conformance::load(&path).unwrap();
    let suite = suite();
    let first = drive::play(&scenario, &path, &suite, &options).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let second = drive::play(&scenario, &path, &suite, &options).unwrap();
    assert_eq!(first.runtime.store.now, second.runtime.store.now);
    assert_eq!(
        serde_json::to_string(&first.ticks).unwrap(),
        serde_json::to_string(&second.ticks).unwrap(),
        "the same scenario twice is the same run"
    );
    // S01 carries seven tick steps, and nothing else moves the clock.
    assert_eq!(first.runtime.store.now, drive::start_of_time() + options.interval * 7);
}

// ---- 2.11 the host step's closed vocabulary

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
fn no_host_step_runs_as_local() {
    let options = RunOptions {
        definitions: Some(root().join("definitions")),
        ..Default::default()
    };
    let (scenario, _) = scenario_from(MINIMAL);
    let run = drive::play(
        &scenario,
        &conformance_dir().join("scenarios/T-test.yaml"),
        &suite(),
        &options,
    )
    .unwrap();
    assert_eq!(run.runtime.store.me(), drive::DEFAULT_HOST);

    // `host: {name: none}` means no host is acting.
    let yaml = MINIMAL.replace("when: [{tick: {}}]", "when: [{host: {name: none}}]");
    let (scenario, _) = scenario_from(&yaml);
    let run = drive::play(
        &scenario,
        &conformance_dir().join("scenarios/T-test.yaml"),
        &suite(),
        &options,
    )
    .unwrap();
    assert_eq!(run.runtime.store.acting_host, None);
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

// ---- 2.12 the direct step's discriminator

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

// ---- 2.13 fixtures

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

// ---- 2.14 the decision the response names

#[test]
fn decision_id_resolves_through_the_register() {
    // S01 answers by the id form and again by the same delivery; both land.
    let options = RunOptions {
        definitions: Some(root().join("definitions")),
        ..Default::default()
    };
    let path = conformance_dir().join("scenarios/S01.yaml");
    let (scenario, _) = flywheel_atoms::conformance::load(&path).unwrap();
    let run = drive::play(&scenario, &path, &suite(), &options).unwrap();
    let object = run
        .runtime
        .store
        .objects
        .get("elaboration/atlas-provider-limits/research-1")
        .unwrap();
    assert_eq!(object.applied_responses, vec!["discord/1001".to_string()]);
    // The register gave it a number, and the id form found it through that.
    assert!(!run.runtime.store.decision_numbers.is_empty());
}

#[test]
fn decision_naming_nothing_standing_fails() {
    let options = RunOptions {
        definitions: Some(root().join("definitions")),
        ..Default::default()
    };
    let yaml = MINIMAL.replace(
        "when: [{tick: {}}]",
        r#"when: [{tick: {}}, {response: {decision: "intent/nothing/never-raised", answer: "yes", id: "page/1"}}]"#,
    );
    let (scenario, _) = scenario_from(&yaml);
    let outcome = drive::play(
        &scenario,
        &conformance_dir().join("scenarios/T-test.yaml"),
        &suite(),
        &options,
    );
    let err = match outcome {
        Ok(_) => panic!("a response naming no standing decision must fail the scenario"),
        Err(e) => format!("{e:#}"),
    };
    assert!(err.contains("stands nowhere"), "{err}");
}

// ---- 2.15 `requires:` is data

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

// ---- 2.16 the report and the exit codes

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
fn failure_line_names_clauses_and_trace() {
    let options = RunOptions {
        definitions: Some(root().join("definitions")),
        trace: Some(std::env::temp_dir().join("flywheel-trace-fail")),
        tracing: true,
        ..Default::default()
    };
    // S01 with one transition it never makes.
    let text = std::fs::read_to_string(conformance_dir().join("scenarios/S01.yaml")).unwrap();
    let text = text.replace("to: working}", "to: nowhere}");
    let path = std::env::temp_dir().join("S01-broken.yaml");
    std::fs::write(&path, &text).unwrap();
    // The scenario must sit under the conformance root to find the suite.
    let staged = conformance_dir().join("scenarios/.S01-broken.yaml");
    std::fs::write(&staged, &text).unwrap();
    let outcome = conformance::run_one(&staged, &options);
    let _ = std::fs::remove_file(&staged);

    assert_eq!(outcome.status, Status::Failed);
    let line = outcome.failures.join("\n");
    assert!(line.contains("FAIL S1"), "{line}");
    assert!(line.contains("1, 6, 13, 24, 314"), "the clause numbers: {line}");
    assert!(line.contains("I1, I2"), "the invariants: {line}");
    assert!(line.contains("expected"), "{line}");
    assert!(line.contains("actual"), "{line}");
    assert!(line.contains("trace"), "the path to the trace: {line}");

    // The run prints one line per scenario, then a summary.
    let report = conformance::RunReport {
        outcomes: vec![outcome],
        ..Default::default()
    };
    let rendered = report.render();
    assert!(rendered.starts_with("FAIL"));
    assert!(rendered.contains("0 passed · 1 failed"));
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

// ---- 6.3, 6.6, 6.17, 6.18: the group's own acceptance, in the gate

/// The scenarios group 6 admits, on the profile each names. `status.yaml` and
/// S20 run on the git-only profile because what they assert is the committed
/// file a reader with no host running finds (D12, 145, S20).
#[test]
#[ignore = "group gate: cargo test --workspace -- --include-ignored"]
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
#[ignore = "group gate: cargo test --workspace -- --include-ignored"]
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

/// The `flywheel` binary beside the test binary. `cargo test -p
/// flywheel-scenario` does not build another package's binary, so build it
/// when it is not there; the runner itself is the binary and needs none of
/// this.
fn flywheel_binary_beside_the_test() -> PathBuf {
    let deps = std::env::current_exe().expect("the test binary has a path");
    let target = deps.parent().and_then(|p| p.parent()).expect("target/debug");
    let binary = target.join("flywheel");
    if !binary.exists() {
        let out = std::process::Command::new(env!("CARGO"))
            .args(["build", "--quiet", "-p", "flywheel"])
            .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
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
#[ignore = "group gate: cargo test --workspace -- --include-ignored"]
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

// ---- 11.5 the machine files' hash, in the record and against the directory

/// A run records the hash of the machine files it ran and compares it with
/// `definitions/`, the mirror of the model. What passed against a directory
/// the repository does not hold proved nothing about the model, so the profile
/// is refused with exit 3 and no scenario plays (168, D2).
#[test]
fn run_record_hash_matches_definitions() {
    let options = RunOptions {
        definitions: Some(root().join("definitions")),
        ..Default::default()
    };
    let report = conformance::run(
        &[conformance_dir().join("contract/read.yaml")],
        &options,
    )
    .expect("the run is made");
    assert_eq!(
        report.record.profile, "git-only",
        "the record names the binding the run bound"
    );
    assert_eq!(
        Some(report.record.definitions_hash),
        report.record.repository_hash,
        "the machine files that ran are definitions/ on disk"
    );
    assert!(report.record.hash_matches_repository());
    assert!(report.refusal.is_none(), "{:?}", report.refusal);
    assert_ne!(report.exit_code(), 3);
    assert!(!report.ran().is_empty(), "the scenario played");

    // The same files with one byte more are a different set, and the run that
    // loads them is refused before anything plays.
    let edited = std::env::temp_dir().join(format!(
        "flywheel-definitions-{}-{}",
        std::process::id(),
        "edited"
    ));
    let _ = std::fs::remove_dir_all(&edited);
    copy_tree(&root().join("definitions"), &edited);
    let atoms = edited.join("atoms.yaml");
    let mut text = std::fs::read_to_string(&atoms).expect("the atoms file is readable");
    text.push_str("\n# a hand edit no rebuild saw\n");
    std::fs::write(&atoms, text).expect("the copy is writable");

    let report = conformance::run(
        &[conformance_dir().join("contract/read.yaml")],
        &RunOptions {
            definitions: Some(edited.clone()),
            ..Default::default()
        },
    )
    .expect("the run is made");
    assert_ne!(
        Some(report.record.definitions_hash),
        report.record.repository_hash,
        "an edited directory is a different set"
    );
    assert!(!report.record.hash_matches_repository());
    assert_eq!(report.exit_code(), 3, "an unmatched hash refuses the profile");
    assert!(report.outcomes.is_empty(), "no scenario played");
    let rendered = report.render();
    assert!(rendered.contains("REFUSED"), "{rendered}");
    assert!(rendered.contains("168"), "the clause: {rendered}");
    let _ = std::fs::remove_dir_all(&edited);
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).expect("the directory is readable").flatten() {
        let path = entry.path();
        let target = to.join(path.file_name().expect("a name"));
        if path.is_dir() {
            std::fs::create_dir_all(&target).expect("the copy is made");
            copy_tree(&path, &target);
        } else {
            std::fs::create_dir_all(to).expect("the copy is made");
            std::fs::copy(&path, &target).expect("the file copies");
        }
    }
}

// ---- 11.8a the run's own record, through the store

/// Every run writes a record through the store the run bound: the profile, the
/// definitions hash, the scenarios that ran, the subset skipped with its reason
/// and the failures (79–82, 93a, 167, D15).
#[test]
#[ignore = "group gate: cargo test --workspace -- --include-ignored"]
fn run_record_names_ran_and_skipped() {
    for profile in [Profile::GitOnly] {
        let options = RunOptions {
            profile,
            definitions: Some(root().join("definitions")),
            ..Default::default()
        };
        // Two that run and one the phase's bindings do not provide for (93a).
        let report = conformance::run(
            &[
                conformance_dir().join("scenarios/S01.yaml"),
                conformance_dir().join("scenarios/S07.yaml"),
                conformance_dir().join("scenarios/S14.yaml"),
            ],
            &options,
        )
        .expect("the run is made");

        assert_eq!(report.record.profile, profile.name());
        assert_eq!(
            report.record.ran,
            vec!["S1".to_string(), "S7".to_string()],
            "the scenarios that ran are named, in the order they did"
        );
        assert_eq!(
            report.record.skipped,
            vec![(
                "S14".to_string(),
                Requirement::RealWorkspace.reason().to_string()
            )],
            "the subset skipped, each with the reason it was (93a)"
        );

        // And it is a record, read back from the store rather than from the
        // struct that wrote it (167).
        let entries = &report.recorded;
        assert!(!entries.is_empty(), "the record was written through the store");
        let binding = entries
            .iter()
            .find(|e| e.kind == "binding")
            .expect("the record says what the run bound");
        assert_eq!(binding.object, profile.name());
        let field = |name: &str| {
            binding
                .fields
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        };
        assert_eq!(
            field("definitions_hash"),
            format!("{:016x}", report.record.definitions_hash),
            "the definitions hash is in the record (168, D2)"
        );
        assert_eq!(field("definitions_match_repository"), "true");
        assert!(field("ran").contains("S7"));
        assert!(field("skipped").contains("S14"));

        let skipped: Vec<&flywheel_domain::records::RunEntry> =
            entries.iter().filter(|e| e.kind == "skipped").collect();
        assert_eq!(skipped.len(), 1, "one entry per scenario skipped");
        assert_eq!(skipped[0].object, "S14");
        assert_eq!(skipped[0].reason, Requirement::RealWorkspace.reason());

        // A failure reaches the record as a problem with the machinery, which
        // is reported and never filed as work (81).
        let mut failed = report.record.clone();
        failed.failures.push("FAIL T · step 1 · effects".into());
        let entries = failed.entries(drive::start_of_time());
        assert!(entries
            .iter()
            .any(|e| e.kind == "problem" && e.reason.starts_with("FAIL T")));
    }
}

// ---- 11.9 the whole phase-1 set

/// The three the whole-set run found, kept where a change that broke them
/// again would be caught: a decision on an object a tick created is numbered in
/// that tick on either profile (15, D15), a pane a scenario killed by its own
/// name is read as gone (196, X08), and a capture that stands with its proof
/// absent acts once per tick (73, 127, S21).
#[test]
#[ignore = "group gate: cargo test --workspace -- --include-ignored"]
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
