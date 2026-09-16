//! The answers a standing decision offers are the ones that apply to the object
//! it stands on, and not every answer its machine names (27, 188, S226,
//! `elaboration.yaml` proposed). The first tier (D17).

use flywheel_engine::runtime::Register;
use flywheel_engine::Object;
use std::collections::BTreeMap;

/// An elaboration standing on a card of its own: the intent it hangs under is
/// named and is not among the objects, so nothing folds it onto a proposed
/// intent's card (10).
fn elaboration(defs: &flywheel_engine::Definitions, covers: &[&str]) -> Object {
    let mut held = Object {
        id: "elaboration/atlas-provider-limits/proposed-1".into(),
        machine: "elaboration".into(),
        parent: Some("intent/atlas-provider-limits".into()),
        config: Default::default(),
        entered_at: Default::default(),
        record: Default::default(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    };
    if !covers.is_empty() {
        let named: Vec<String> = covers.iter().map(|i| i.to_string()).collect();
        held.record.insert("covers".into(), serde_json::json!(named));
    }
    flywheel_engine::initialise(defs, &mut held, chrono::Utc::now());
    assert_eq!(held.top_state(), Some("proposed"));
    held
}

/// What the one standing decision offers.
fn offered(defs: &flywheel_engine::Definitions, held: Object) -> Vec<String> {
    let mut objects: BTreeMap<String, Object> = BTreeMap::new();
    objects.insert(held.id.clone(), held);
    let standing = crate::rail::standing(defs, &objects, &Register::default());
    assert_eq!(standing.len(), 1, "one decision stands: {standing:?}");
    standing[0].answers.clone()
}

fn a_pick(answer: &String) -> bool {
    answer.starts_with("pick ")
}

fn a_per_intent_drop(answer: &String) -> bool {
    answer.starts_with('<') && answer.ends_with(": drop")
}

/// An elaboration covering one intent has nothing to pick and no intent to take
/// out of a gathering, so its decision offers neither.
///
/// The page drew the right controls already; the decision carried the machine's
/// whole list, and the chat's controls and the numbered reply grammar read that
/// list — so an answer the operator could never see on the card was one they
/// could still send (188, S226).
#[test]
fn an_elaboration_of_one_intent_offers_no_pick() {
    let defs = crate::set::load().expect("the shipped set");
    let offered = offered(&defs, elaboration(&defs, &[]));

    for answer in ["yes", "drop", "type <name>"] {
        assert!(
            offered.iter().any(|held| held == answer),
            "an elaboration still takes `{answer}`: {offered:?}"
        );
    }
    assert!(
        !offered.iter().any(a_pick),
        "it offers a pick over the one intent it covers: {offered:?}"
    );
    assert!(
        !offered.iter().any(a_per_intent_drop),
        "it offers an intent's own drop with one intent covered: {offered:?}"
    );
}

/// A gathering covers several intents, and both answers apply to it (188).
#[test]
fn a_gathering_offers_the_pick_and_the_per_intent_drop() {
    let defs = crate::set::load().expect("the shipped set");
    let held = elaboration(
        &defs,
        &["intent/atlas-provider-limits", "intent/atlas-rate-caps"],
    );
    let offered = offered(&defs, held);

    assert!(
        offered.iter().any(a_pick),
        "a gathering offers no pick: {offered:?}"
    );
    assert!(
        offered.iter().any(a_per_intent_drop),
        "a gathering offers no per-intent drop: {offered:?}"
    );
}
