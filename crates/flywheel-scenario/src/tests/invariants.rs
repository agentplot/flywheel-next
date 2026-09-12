//! The invariants a scenario names are machine-checked over its trace, and a
//! trace that breaks one fails naming it (model.md §15; audit 4).

use crate::conformance::invariants::{self, View, HELD_ELSEWHERE, TRACE_CHECKED};
use crate::runner::{EffectRecord2, TickRecord, TransitionRecord};
use crate::store::Store;
use flywheel_atoms::conformance::GivenObject;
use flywheel_engine::Object;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn conformance_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance")
}

/// Every invariant any shipped scenario or contract row names.
fn named_by_the_suite() -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for dir in ["scenarios", "contract"] {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(conformance_dir().join(dir))
            .expect("the mirrored suite")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
            .collect();
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path).unwrap();
            let read: serde_json::Value = serde_yaml::from_str(&text).unwrap();
            let scenario = read["scenario"].as_str().unwrap_or_default().to_string();
            for name in read["invariants"].as_array().into_iter().flatten() {
                out.entry(name.as_str().unwrap_or_default().to_string())
                    .or_default()
                    .push(scenario.clone());
            }
        }
    }
    out
}

fn unit(id: &str, life: &str) -> GivenObject {
    GivenObject {
        id: id.into(),
        machine: "unit".into(),
        parent: None,
        state: [("life".to_string(), life.to_string())].into_iter().collect(),
        record: [("type".to_string(), json!("default"))].into_iter().collect(),
        applied_responses: vec![],
    }
}

fn tick(n: u64, transitions: Vec<TransitionRecord>, effects: Vec<EffectRecord2>) -> TickRecord {
    TickRecord {
        tick: n,
        at: crate::runner::epoch(),
        guards: vec![],
        transitions,
        effects,
        decisions: vec![],
        cost: Default::default(),
    }
}

fn moved(object: &str, from: &str, to: &str, response: Option<&str>) -> TransitionRecord {
    TransitionRecord {
        object: object.into(),
        region: "life".into(),
        from: from.into(),
        to: to.into(),
        response: response.map(String::from),
        reason: None,
        host: None,
    }
}

fn effect(name: &str, object: &str) -> EffectRecord2 {
    EffectRecord2 {
        effect_id: format!("{object}/{name}/1"),
        name: name.into(),
        object: object.into(),
        args: BTreeMap::new(),
        written: true,
        recalled: false,
    }
}

fn names(named: &[&str]) -> Vec<String> {
    named.iter().map(|n| n.to_string()).collect()
}

fn store_with(objects: Vec<Object>) -> Store {
    let mut store = Store::default();
    for object in objects {
        store.objects.insert(object.id.clone(), object);
    }
    store
}

/// Every I-number the shipped suite names is either a predicate over the trace
/// or held by a named gate elsewhere — never a string in a failure message and
/// nothing more — and a trace that breaks a named one fails the scenario
/// naming the invariant it broke (I1–I16; audit 4).
#[test]
fn an_invariant_named_by_a_scenario_is_checked() {
    let named = named_by_the_suite();
    assert!(!named.is_empty(), "the suite names no invariants");
    let checked: BTreeSet<&str> = TRACE_CHECKED.iter().map(|(n, _)| *n).collect();
    let elsewhere: BTreeSet<&str> = HELD_ELSEWHERE.iter().map(|(n, _)| *n).collect();
    assert!(checked.is_disjoint(&elsewhere), "an invariant is in both lists");
    for (name, scenarios) in &named {
        assert!(
            checked.contains(name.as_str()) || elsewhere.contains(name.as_str()),
            "{scenarios:?} name `{name}`, which is neither checked over the trace nor held by a named gate"
        );
        assert!(invariants::statement(name).is_some(), "`{name}` has no statement");
    }
    // The model states sixteen, and every one of them is accounted for.
    for n in 1..=16 {
        let name = format!("I{n}");
        assert!(
            checked.contains(name.as_str()) || elsewhere.contains(name.as_str()),
            "`{name}` is accounted for nowhere"
        );
    }

    let defs = flywheel_domain::set::load().expect("the embedded definitions");

    // I1 broken: an effect on a unit still in `proposed`.
    let given = vec![unit("unit/atlas/u", "proposed")];
    let store = store_with(vec![]);
    let broken = vec![tick(1, vec![], vec![effect("create_items", "unit/atlas/u")])];
    let failures = invariants::check(
        &names(&["I1"]),
        &View { given: &given, ticks: &broken, lease_log: &[], decisions_after: &[], store: &store, defs: &defs },
    );
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].clause, "invariant I1");
    assert!(failures[0].actual.contains("before approved"), "{}", failures[0].actual);
    assert!(failures[0].expected.contains("approved"), "the statement is what was expected");

    // I1 held: approved by a response, and the effect after it.
    let mut approved = Object {
        id: "unit/atlas/u".into(),
        machine: "unit".into(),
        parent: None,
        config: [("life".to_string(), "approved".to_string())].into_iter().collect(),
        entered_at: Default::default(),
        record: Default::default(),
        counters: Default::default(),
        applied_responses: vec!["page-7".into()],
        seq: 1,
        created: 0,
    };
    let response = Object {
        id: "response/page-7".into(),
        machine: "response".into(),
        parent: None,
        config: Default::default(),
        entered_at: Default::default(),
        record: [("delivery_id".to_string(), json!("page-7"))].into_iter().collect(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 1,
        created: 0,
    };
    let store = store_with(vec![approved.clone(), response]);
    let held = vec![tick(
        1,
        vec![moved("unit/atlas/u", "proposed", "approved", Some("page-7"))],
        vec![effect("create_items", "unit/atlas/u")],
    )];
    let failures = invariants::check(
        &names(&["I1", "I2", "I12"]),
        &View { given: &given, ticks: &held, lease_log: &[], decisions_after: &[], store: &store, defs: &defs },
    );
    assert!(failures.is_empty(), "{failures:?}");

    // I1 broken the other way: approved by nothing.
    let by_nothing = vec![tick(1, vec![moved("unit/atlas/u", "proposed", "approved", None)], vec![])];
    let failures = invariants::check(
        &names(&["I1"]),
        &View { given: &given, ticks: &by_nothing, lease_log: &[], decisions_after: &[], store: &store, defs: &defs },
    );
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert!(failures[0].actual.contains("no response"), "{}", failures[0].actual);

    // I2 broken: the object moved by a response its applied_responses lacks.
    approved.applied_responses.clear();
    let store = store_with(vec![approved]);
    let failures = invariants::check(
        &names(&["I2"]),
        &View { given: &given, ticks: &held, lease_log: &[], decisions_after: &[], store: &store, defs: &defs },
    );
    assert!(
        failures.iter().any(|f| f.clause == "invariant I2" && f.actual.contains("applied_responses")),
        "{failures:?}"
    );

    // I12 broken: an effect no atom names.
    let unnamed = vec![tick(1, vec![], vec![effect("frobnicate", "unit/atlas/u")])];
    let failures = invariants::check(
        &names(&["I12"]),
        &View { given: &given, ticks: &unnamed, lease_log: &[], decisions_after: &[], store: &store, defs: &defs },
    );
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].clause, "invariant I12");
    assert!(failures[0].actual.contains("frobnicate"));

    // I8 broken: a bolt made for a chore.
    let chore = GivenObject {
        record: [("type".to_string(), json!("chore"))].into_iter().collect(),
        ..unit("unit/atlas/c", "approved")
    };
    let made = vec![tick(1, vec![], vec![effect("create_bolt", "unit/atlas/c")])];
    let failures = invariants::check(
        &names(&["I8"]),
        &View { given: &[chore], ticks: &made, lease_log: &[], decisions_after: &[], store: &store, defs: &defs },
    );
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].clause, "invariant I8");

    // I6 broken: a response ended a session directly.
    let session = GivenObject {
        id: "session/atlas/s".into(),
        machine: "session".into(),
        parent: None,
        state: [("life".to_string(), "alive".to_string())].into_iter().collect(),
        record: Default::default(),
        applied_responses: vec![],
    };
    let ended = vec![tick(1, vec![moved("session/atlas/s", "alive", "ended", Some("page-9"))], vec![])];
    let failures = invariants::check(
        &names(&["I6"]),
        &View { given: &[session], ticks: &ended, lease_log: &[], decisions_after: &[], store: &store, defs: &defs },
    );
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].clause, "invariant I6");

    // One held elsewhere is not failed here, and one the model never stated is
    // an error rather than a silent pass.
    let failures = invariants::check(
        &names(&["I15", "I99"]),
        &View { given: &given, ticks: &held, lease_log: &[], decisions_after: &[], store: &store, defs: &defs },
    );
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].clause, "invariant I99");
}
