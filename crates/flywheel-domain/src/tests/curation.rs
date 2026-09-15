//! Run curation now: the operator's dictation, which the curation machine's
//! idle takes to running at once whatever its threshold and cadence say, and
//! which, given while curation runs, is recorded unapplicable and reported
//! rather than kept in silence (110, 12, 6, 154, `curation.yaml`).

use crate::commands::{self, CallRecord};
use crate::derived::{self, Reading};
use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::Records;
use flywheel_engine::Definitions;
use serde_json::json;

const CURATION: &str = "curation/willdan";

fn a_curation(store: &mut FakeStore, defs: &Definitions) {
    let at = commands::now(store).unwrap();
    let record = [("threshold".to_string(), json!(12)), ("cadence".to_string(), json!(crate::cadence::DEFAULT))];
    commands::put_new(store, defs, CURATION, "curation", None, record.into_iter().collect(), at).unwrap();
}

/// One run now, as the catalogue's `curate` records it: a dictation naming the
/// instance's curation, answered `run` (193).
fn run_now(store: &mut FakeStore, defs: &Definitions, delivery: &str) -> String {
    let call = CallRecord {
        tool: "curate",
        decision: None,
        object: Some(CURATION),
        answer: "run",
        args: None,
        by: "chuck",
        delivery: "page",
        delivery_id: Some(delivery),
        proposed_by: None,
    };
    commands::record_call(store, defs, &call).unwrap().id
}

/// What a host answers before a tick reads it: one unmoved signal, the
/// threshold and no cadence due on the curation, and each response's reads as
/// the host derives them.
fn answer_the_reads(store: &mut FakeStore, defs: &Definitions) {
    store.given(CURATION, "curation.unmoved_count", json!(1));
    store.given(CURATION, "curation.threshold", json!(12));
    store.given(CURATION, "curation.cadence_due", json!(false));
    let reading = Reading::new("studio", commands::now(store).unwrap());
    for held in store.list_records(&flywheel_atoms::Scope::All).unwrap() {
        if held.machine != "response" {
            continue;
        }
        let named = held.record.get("object").and_then(|v| v.as_str()).and_then(|id| store.get(id).unwrap());
        let present = commands::dictation_takes(defs, &held, named.as_ref())
            .map(|takes| json!(takes))
            .or_else(|| derived::evidence(store, &reading, &held.id, "response.decision_present"))
            .unwrap();
        let applied = derived::evidence(store, &reading, &held.id, "response.applied").unwrap();
        store.given(&held.id, "response.decision_present", present);
        store.given(&held.id, "response.applied", applied);
    }
}

/// One tick with the reads answered, and what it moved.
fn tick(store: &mut FakeStore, defs: &Definitions) -> Vec<String> {
    answer_the_reads(store, defs);
    let mut moved = Vec::new();
    commands::tick(
        store,
        defs,
        &flywheel_atoms::Scope::All,
        |_, _, _, _| true,
        |_, fired, _| moved.push(format!("{} {}: {} → {}", fired.object, fired.region, fired.from, fired.to)),
    )
    .unwrap();
    moved
}

fn state(store: &FakeStore, object: &str, region: &str) -> String {
    store.get(object).unwrap().unwrap().config.get(region).cloned().unwrap_or_default()
}

/// Run now with one unmoved signal and no cadence due: the tick before it
/// leaves curation idle, and the one after takes it to running with the curator
/// session charged, the threshold and cadence as they were, and the response
/// applied (110, 12).
#[test]
fn curate_runs_curation_below_the_threshold() {
    let defs = crate::set::load().unwrap();
    let mut store = FakeStore::default();
    a_curation(&mut store, &defs);
    tick(&mut store, &defs);
    assert_eq!(state(&store, CURATION, "run"), "idle", "one signal and no cadence due charge nothing");

    let response = run_now(&mut store, &defs, "page-run-1");
    let moved = tick(&mut store, &defs);
    assert!(moved.iter().any(|m| m == &format!("{CURATION} run: idle → running")), "{moved:?}");
    let held = store.get(CURATION).unwrap().unwrap();
    assert!(held.config.contains_key("run.running.session"), "the curator session is charged: {:?}", held.config);
    assert!(held.applied_responses.contains(&response), "run now was not what moved it");
    assert_eq!(held.record.get("threshold"), Some(&json!(12)), "the threshold stands as it was");
    assert_eq!(held.record.get("cadence"), Some(&json!(crate::cadence::DEFAULT)), "the cadence stands as it was");

    tick(&mut store, &defs);
    assert_eq!(state(&store, &format!("response/{response}"), "life"), "applied");
}

/// Run now given while curation runs is no transition of the state it is in: it
/// is recorded unapplicable and reported under attention, and curation runs on
/// as it was (154, 6).
#[test]
fn curate_while_running_is_unapplicable() {
    let defs = crate::set::load().unwrap();
    let mut store = FakeStore::default();
    a_curation(&mut store, &defs);
    let first = run_now(&mut store, &defs, "page-run-1");
    tick(&mut store, &defs);
    assert_eq!(state(&store, CURATION, "run"), "running");

    let again = run_now(&mut store, &defs, "page-run-2");
    tick(&mut store, &defs);
    assert_eq!(state(&store, &format!("response/{again}"), "life"), "unapplicable");
    assert_eq!(state(&store, &format!("response/{first}"), "life"), "applied");
    let held = store.get(CURATION).unwrap().unwrap();
    assert_eq!(state(&store, CURATION, "run"), "running", "curation runs on as it was");
    assert!(!held.applied_responses.contains(&again), "the second run now was applied");

    let rail = commands::rail(&mut store, &defs).unwrap();
    assert!(
        rail.iter().any(|d| d.object == format!("response/{again}") && d.kind == "response-unapplicable"),
        "the unapplicable run now is not reported: {rail:?}"
    );
}
