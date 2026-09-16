//! A seed covers what it seeds: a line a described state says is there is
//! there, so the work hanging off it can be placed (19.6, 125, 149). The first
//! tier (D17).

use crate::scenario;
use flywheel_engine::runtime::EvidenceSource;
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("workspace root")
}

fn storefront() -> flywheel_atoms::scenario::Scenario {
    scenario::load(&workspace().join("scenarios/storefront/scenario.yaml")).expect("the storefront scenario loads")
}

/// The storefront seeds an intent whose line is current, and nothing made that
/// line: the guard an elaboration's place waits on read it as absent, so the
/// elaboration got no place and the scenario could not run past it. The seed
/// makes what it says is there (19.6, 125, 149).
#[test]
fn a_seeded_intents_line_is_one_the_world_holds() {
    const INTENT: &str = "intent/storefront-mobile-speed";
    let defs = flywheel_engine::load::load_dir(&workspace().join("definitions")).expect("definitions load");
    let sc = storefront();
    let rt = scenario::seed(defs, &sc);

    // The state said the line was current, and the world holds it.
    let line = rt
        .store
        .world
        .lines
        .get(INTENT)
        .unwrap_or_else(|| panic!("the seed made no line for {INTENT}: {:?}", rt.store.world.lines.keys()));
    assert!(line.exists && !line.absent, "the seeded line is not there: {line:?}");

    // And the guard reads it as there, which is what a place off the intent's
    // line waits on and what was false before.
    assert_eq!(
        rt.store.evidence(INTENT, "line", "line.exists"),
        Some(serde_json::json!(true)),
        "the line a seeded intent says it has reads as absent"
    );
    assert_eq!(
        rt.store.evidence(INTENT, "line", "line.absent"),
        Some(serde_json::json!(false)),
        "the line a seeded intent says it has reads as absent"
    );
}

/// A line is a branch of a repository. A described state that says a line is
/// there when the instance tracks no repository to hold it is refused, saying
/// what would have to be tracked first (149, 205, 206).
#[test]
fn a_seeded_line_no_repository_holds_is_refused_with_what_is_missing() {
    let sc = storefront();

    // The storefront tracks storefront, so its own seed is held.
    scenario::lines_held(&sc, &["storefront".to_string()]).expect("a tracked repository holds its lines");

    // An instance tracking something else is told which repository is missing
    // and which object named it.
    let mut named = storefront();
    named.given.objects.push(flywheel_atoms::scenario::GivenObject {
        id: "bolt/shipping/rate-table".into(),
        machine: "bolt".into(),
        parent: None,
        record: [("repository".to_string(), serde_json::json!("shipping"))].into_iter().collect(),
        state: [("line".to_string(), "current".to_string())].into_iter().collect(),
        entered: None,
    });
    let refused = scenario::lines_held(&named, &["storefront".to_string()])
        .expect_err("a line no tracked repository holds is seeded")
        .to_string();
    assert!(refused.contains("shipping"), "the refusal does not name the repository: {refused}");
    assert!(
        refused.contains("bolt/shipping/rate-table"),
        "the refusal does not name what said so: {refused}"
    );
    assert!(refused.contains("flywheel.yaml"), "the refusal does not say what to do: {refused}");
}
