//! An answer no transition takes is reported, never silently kept (6, 129,
//! `engine/response.yaml` v4, `atoms.yaml` response.taken).
//!
//! A decision offers only answers its state's transitions name, which the
//! model's `check.py` enforces, so an answer that is not taken is one from
//! outside the decision's list, or one whose object has moved on since the
//! card was drawn. Either way it comes back once under attention.

use crate::commands::answer_taken;
use flywheel_engine::{Definitions, Object};
use serde_json::json;
use std::collections::BTreeMap;

fn defs() -> Definitions {
    crate::set::load().expect("the shipped set loads")
}

/// A unit at one state of its life, as the store holds it.
fn unit(defs: &Definitions, life: &str) -> Object {
    let mut object = Object {
        id: "unit/atlas/rows".to_string(),
        machine: "unit".to_string(),
        parent: Some("bolt/atlas/plan-rows".to_string()),
        config: Default::default(),
        entered_at: Default::default(),
        record: [("repository".to_string(), json!("atlas")), ("type".to_string(), json!("chore"))]
            .into_iter()
            .collect::<BTreeMap<String, serde_json::Value>>(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    };
    flywheel_engine::initialise(defs, &mut object, chrono::Utc::now());
    object.config.insert("life".to_string(), life.to_string());
    object
}

/// One answer to a numbered decision, as the catalogue records it.
fn answer(id: &str, object: &str, said: &str) -> Object {
    Object {
        id: format!("response/{id}"),
        machine: "response".to_string(),
        parent: None,
        config: Default::default(),
        entered_at: Default::default(),
        record: [
            ("decision".to_string(), json!(412)),
            ("object".to_string(), json!(object)),
            ("answer".to_string(), json!(said)),
        ]
        .into_iter()
        .collect::<BTreeMap<String, serde_json::Value>>(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    }
}

/// An answer the object's active states name is taken; one no transition of
/// them names is not, and the response machine sends it to `unapplicable`,
/// where it is reported once under attention (6, 129).
#[test]
fn an_answer_no_transition_takes_is_unapplicable() {
    let defs = defs();
    let proposed = unit(&defs, "proposed");

    // `drop` is what a proposed unit's card offers, and unit@5 has the
    // transition that takes it (S232).
    assert_eq!(
        answer_taken(&defs, &answer("page-1", &proposed.id, "drop"), Some(&proposed)),
        Some(true),
        "a proposed unit takes the drop its card offers"
    );
    assert_eq!(
        answer_taken(&defs, &answer("page-2", &proposed.id, "yes"), Some(&proposed)),
        Some(true)
    );

    // An answer from outside the decision's list: recorded, never applied, and
    // so never silently kept.
    assert_eq!(
        answer_taken(&defs, &answer("page-3", &proposed.id, "sideways"), Some(&proposed)),
        Some(false),
        "an answer no transition of the object's states names is unapplicable"
    );

    // The state the response machine sends it to reports it and asks nothing
    // of the operator but an acknowledgement (6, 129).
    let response = defs.get("response").expect("the response machine");
    let unapplicable = response
        .regions
        .values()
        .find_map(|region| region.states.get("unapplicable"))
        .expect("the unapplicable state");
    let decision = unapplicable.decision.as_ref().expect("it is a decision");
    assert_eq!(decision.kind, "response-unapplicable");
    assert_eq!(decision.group, "attention");
}

/// `applied` is read before `taken`, so a response the object took on the same
/// pass is never judged against the state it left (6, 129).
#[test]
fn a_response_applied_on_this_pass_is_not_judged_against_the_state_it_left() {
    let defs = defs();

    // The drop was applied and the unit is now dropped, which takes nothing.
    let dropped = unit(&defs, "dropped");
    assert_eq!(
        answer_taken(&defs, &answer("page-1", &dropped.id, "drop"), Some(&dropped)),
        Some(false),
        "read against the state it left, the applied answer would look untaken"
    );

    // Which is why the machine reads `applied` first: the order of the
    // transitions out of `given` is what keeps that response out of
    // `unapplicable` (`engine/response.yaml` v4).
    let response = defs.get("response").expect("the response machine");
    let given = response
        .regions
        .values()
        .find_map(|region| region.states.get("given"))
        .expect("the given state");
    let reads: Vec<String> = given
        .transitions
        .iter()
        .map(|t| format!("{:?}", t.when))
        .collect();
    let applied_at = reads
        .iter()
        .position(|guard| guard.contains("response.applied"))
        .expect("the given state reads response.applied");
    let taken_at = reads
        .iter()
        .position(|guard| guard.contains("response.taken"))
        .expect("the given state reads response.taken");
    assert!(
        applied_at < taken_at,
        "applied is read first, else a response applied on this pass is judged \
         against the state it left: {reads:?}"
    );
}
