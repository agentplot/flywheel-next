//! What rides on another decision's card rather than standing on its own
//! (10, `intent.yaml` proposed, `elaboration.yaml` proposed).

use flywheel_engine::runtime::Register;
use flywheel_engine::Object;
use std::collections::BTreeMap;

fn object(defs: &flywheel_engine::Definitions, id: &str, machine: &str, parent: Option<&str>) -> Object {
    let mut held = Object {
        id: id.into(),
        machine: machine.into(),
        parent: parent.map(String::from),
        config: Default::default(),
        entered_at: Default::default(),
        record: Default::default(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    };
    flywheel_engine::initialise(defs, &mut held, chrono::Utc::now());
    held
}

/// An elaboration proposed against an intent that is itself still proposed is
/// part of that intent's decision, and takes no card and no number of its own
/// (10).
///
/// Requirement 10's first decision kind is "a proposed intent, with its
/// proposed elaborations". The machines say the same thing in two places — the
/// intent's `proposed` doc reads "a decision with its proposed elaborations
/// folded in", the elaboration's "folded into the intent's decision when the
/// intent is itself proposed" — and the evidence atom
/// `elaboration.shown_with_parent` is the predicate. The rail numbered it
/// anyway, so a curation that proposed a subject and the work to understand it
/// put two cards where the operator has one thing to decide, and the second
/// was not theirs to answer: until the intent is a subject at all there is
/// nothing to say about how to understand it.
#[test]
fn a_proposed_elaboration_rides_on_its_proposed_intents_card() {
    let defs = crate::set::load().expect("the shipped set");
    let mut objects: BTreeMap<String, Object> = BTreeMap::new();

    // A proposed intent, and the elaboration the machinery proposed with it.
    let intent = object(&defs, "intent/atlas-declines", "intent", None);
    assert_eq!(intent.top_state(), Some("proposed"));
    let elaboration = object(
        &defs,
        "elaboration/atlas-declines/proposed-1",
        "elaboration",
        Some("intent/atlas-declines"),
    );
    assert_eq!(elaboration.top_state(), Some("proposed"));
    objects.insert(intent.id.clone(), intent.clone());
    objects.insert(elaboration.id.clone(), elaboration.clone());

    // The engine derives both, because a decision is a live state carrying one
    // and it knows no object names. That is right and is not the whole rule.
    let raw = flywheel_engine::rail::derive(&defs, &objects, &Register::default());
    assert_eq!(raw.len(), 2, "{raw:?}");

    // The rail a reader sees carries one card, and it names the elaboration it
    // was proposed with.
    let standing = crate::rail::standing(&defs, &objects, &Register::default());
    assert_eq!(standing.len(), 1, "{standing:?}");
    assert_eq!(standing[0].object, "intent/atlas-declines");
    assert_eq!(standing[0].kind, "intent-proposed");
    assert!(
        standing[0].folds.iter().any(|id| id == &elaboration.id),
        "the card names what rides on it: {:?}",
        standing[0].folds
    );

    // Once the intent is open there is something to decide about how to
    // understand it, and the elaboration stands on its own.
    let mut open = intent.clone();
    open.config.insert("life".into(), "open".into());
    objects.insert(open.id.clone(), open);
    let standing = crate::rail::standing(&defs, &objects, &Register::default());
    let kinds: Vec<&str> = standing.iter().map(|d| d.kind.as_str()).collect();
    assert_eq!(kinds, vec!["elaboration-proposed"], "{standing:?}");
    assert_eq!(standing[0].object, elaboration.id);
}

/// An elaboration under an intent that is not proposed is a decision of its
/// own wherever the elaboration sits, and the fold never reaches past the one
/// case the model names.
#[test]
fn nothing_else_is_folded_onto_a_parent() {
    let defs = crate::set::load().expect("the shipped set");
    let mut objects: BTreeMap<String, Object> = BTreeMap::new();

    // An intent already open, gaining material: the elaboration the machinery
    // proposes for it is the operator's to answer and is numbered like any
    // other (21). This is the shape `S8` reaches.
    let mut intent = object(&defs, "intent/atlas-provider-limits", "intent", None);
    intent.config.insert("life".into(), "open".into());
    let elaboration = object(
        &defs,
        "elaboration/atlas-provider-limits/proposed-1",
        "elaboration",
        Some("intent/atlas-provider-limits"),
    );
    objects.insert(intent.id.clone(), intent);
    objects.insert(elaboration.id.clone(), elaboration.clone());

    let standing = crate::rail::standing(&defs, &objects, &Register::default());
    assert_eq!(standing.len(), 1, "{standing:?}");
    assert_eq!(standing[0].object, elaboration.id);
    assert_eq!(standing[0].kind, "elaboration-proposed");
}
