//! A scenario is a directory, and what happens after its moment is a list of
//! actions (`design/flywheel-next/scenarios/storefront.md`). The first tier
//! (D17): what a scenario file says, over no run.

use crate::conformance::{self, Suite};
use flywheel_atoms::conformance::{Action, Scenario};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// A directory of this test's own, removed when it is done.
struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        // Under the repository rather than the system's temporary directory:
        // what this is about is a scenario kept outside `conformance/` and
        // still closed by the suite's schema, and the suite is found by walking
        // up from the scenario.
        let dir = root().join("target/flywheel-scenario-dir").join(format!(
            "{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the directory is made");
        Dir(dir)
    }

    fn write(&self, path: &str, body: &str) -> PathBuf {
        let at = self.0.join(path);
        std::fs::create_dir_all(at.parent().expect("a parent")).expect("the directory is made");
        std::fs::write(&at, body).expect("the file is written");
        at
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A scenario with one action of whatever the caller names.
fn with_action(action: &str) -> Scenario {
    let yaml = format!(
        r#"
scenario: T-actions
title: a scenario for the action vocabulary
profiles: [all]
satisfies: [94]
given: {{}}
when: [{{tick: {{}}}}]
then: {{}}
actions:
  - {action}
"#
    );
    serde_yaml::from_str(&yaml).expect("the yaml parses as a scenario")
}

/// An action is exactly one thing a real actor does: something arrives, the
/// operator answers, a session delivers and exits. Nothing sets state (125,
/// 193).
#[test]
fn an_action_is_one_actors_doing() {
    // The three, each parsed as itself.
    let arrived = with_action(r#"capture: {adapter: "flywheel capture meeting 2026-09-02-storefront-weekly.vtt", by: dana}"#);
    assert!(matches!(
        arrived.actions().expect("a capture is an action").as_slice(),
        [Action::Capture(_)]
    ));
    let answered = with_action(r#"response: {decision: "intent/declines/intent-proposed", answer: "yes", id: page/1}"#);
    assert!(matches!(
        answered.actions().expect("a response is an action").as_slice(),
        [Action::Response(_)]
    ));
    let delivered = with_action(
        r#"session: {object: "elaboration/declines/research", exit: done, deliver: {"research/declines.md": "flywheel/elaborations/declines.md"}}"#,
    );
    assert!(matches!(
        delivered.actions().expect("a delivery is an action").as_slice(),
        [Action::Session(_)]
    ));

    // Every other way a scenario could reach the state it wants is refused by
    // name. Each of these is a `when:` step, and each would be the scenario
    // standing in for machinery that does not work.
    for (written, why) in [
        (r#"evidence: {place.exists: {"*": true}}"#, "evidence"),
        (r#"files: {"a.md": "hello"}"#, "files"),
        (r#"direct: {do: store, object: "intent/x", set: {life: open}}"#, "direct"),
        (r#"script: {"session/1": [{exit: done}]}"#, "script"),
        (r#"clock: {advance: 30m}"#, "clock"),
        (r#"tick: {}"#, "tick"),
        (r#"host: {name: laptop, start: true}"#, "host"),
        (r#"restart: {}"#, "restart"),
    ] {
        let refused = with_action(written).actions().expect_err(
            "a scenario that writes state by any other means is refused, never accommodated",
        );
        let said = format!("{refused:#}");
        assert!(
            said.contains(why),
            "the refusal names `{why}` so the author knows what it was: {said}"
        );
        assert!(
            said.contains("capture") && said.contains("response") && said.contains("session"),
            "and names the three an action may be: {said}"
        );
    }

    // A capture arrives one of two ways, never both and never neither.
    with_action(r#"capture: {by: dana}"#)
        .actions()
        .expect_err("a capture that names neither an adapter nor its text is refused");
    with_action(r#"capture: {adapter: "flywheel capture meeting x.vtt", text: "and also this"}"#)
        .actions()
        .expect_err("a capture that names both is refused");
}

/// A scenario is a directory with `bundle/` beside it; a single file is the
/// same thing with nothing beside it, and both load the same way.
#[test]
fn a_scenario_directory_loads_like_a_file() {
    const BODY: &str = r#"
scenario: T-directory
title: a scenario kept as a directory
profiles: [all]
satisfies: [94]
given: {}
when: [{tick: {}}]
then: {}
actions:
  - capture: {text: "checkout fails on amex", by: dana}
tour: ["Someone types what they noticed."]
"#;
    let dir = Dir::new("directory");
    let as_file = dir.write("flat.yaml", BODY);
    let as_directory = dir.write("storefront/scenario.yaml", BODY);
    dir.write("storefront/bundle/research/declines.md", "# why cards decline\n");

    let (flat, _) = flywheel_atoms::conformance::load(&as_file).expect("the file loads");
    let (held, _) = flywheel_atoms::conformance::load(&dir.0.join("storefront"))
        .expect("the directory loads, named as the directory");
    assert_eq!(flat.scenario, held.scenario);
    assert_eq!(held.actions().expect("the actions parse").len(), 1);
    assert_eq!(held.tour_line(1), Some("Someone types what they noticed."));

    // The bundle is beside the scenario, and the flat file has none.
    let bundle = flywheel_atoms::conformance::bundle_of(&dir.0.join("storefront"))
        .expect("the directory has a bundle beside it");
    assert!(bundle.join("research/declines.md").is_file());
    assert_eq!(
        flywheel_atoms::conformance::bundle_of(&as_file),
        None,
        "a scenario file with no bundle beside it has none"
    );

    // The run names it the same way, and the bundle is not mistaken for a set
    // of scenarios.
    assert_eq!(
        conformance::scenario_files(&dir.0.join("storefront")).expect("the directory is one scenario"),
        vec![as_directory.clone()]
    );
    let mut every = conformance::scenario_files(&dir.0).expect("the directory is walked");
    every.sort();
    assert_eq!(
        every,
        vec![as_file, as_directory],
        "a directory holding scenario.yaml is one scenario and `bundle/` is none"
    );

    // And it is the model's schema that closes it, wherever it is kept: there
    // is one scenario mechanism and not two.
    let suite = Suite::for_scenario(&dir.0.join("storefront"))
        .expect("a scenario outside the suite still validates against it");
    assert_eq!(suite.root, root().join("conformance").canonicalize().expect("the suite resolves"));
    let (_, document) =
        flywheel_atoms::conformance::load(&dir.0.join("storefront")).expect("it loads");
    suite.validate(&document).expect("actions and tour are in the schema");
}

/// A tour is a line per action, or none at all: a tour that has drifted from
/// the actions tells the viewer about the wrong moment.
#[test]
fn a_tour_is_a_line_per_action() {
    let yaml = r#"
scenario: T-tour
title: a tour that has drifted
profiles: [all]
satisfies: [94]
given: {}
when: [{tick: {}}]
then: {}
actions:
  - capture: {text: "one", by: dana}
  - capture: {text: "two", by: dana}
tour: ["only one line"]
"#;
    let scenario: Scenario = serde_yaml::from_str(yaml).expect("the yaml parses");
    let said = format!(
        "{:#}",
        scenario.actions().expect_err("a short tour is refused")
    );
    assert!(said.contains("1 lines for 2 actions"), "{said}");
}
