//! A dictation skips the rail and takes the transition the decision would have
//! taken, with the same effects (4, 12).

use crate::testing as world;

use flywheel_atoms::{Records, StateStore};
use flywheel_domain::commands;
use flywheel_engine::{Definitions, Object, PlannedEffect};
use crate::catalogue::{self, Call};
use serde_json::json;

/// An item waiting on the operator, where the machine offers a decision whose
/// answers are `retry` and `drop` (41). The one object both halves start from.
fn a_stalled_item<S: StateStore>(store: &mut S, defs: &Definitions) {
    let at = commands::now(store).expect("a point");
    commands::put_new(
        store,
        defs,
        "bolt/atlas/plan-rows",
        "bolt",
        None,
        [("repository".to_string(), json!("atlas"))]
            .into_iter()
            .collect(),
        at,
    )
    .expect("the bolt");
    commands::put_new(
        store,
        defs,
        "unit/atlas/u",
        "unit",
        Some("bolt/atlas/plan-rows"),
        [
            ("type".to_string(), json!("default")),
            ("type_version".to_string(), json!(5)),
        ]
        .into_iter()
        .collect(),
        at,
    )
    .expect("the unit");
    commands::put_new(
        store,
        defs,
        ITEM,
        "work-item",
        Some("unit/atlas/u"),
        [
            ("ordinal".to_string(), json!(1)),
            ("type".to_string(), json!("default")),
            ("type_version".to_string(), json!(5)),
        ]
        .into_iter()
        .collect(),
        at,
    )
    .expect("the item");
    let mut item = store.get(ITEM).expect("a read").expect("the item");
    item.config.insert("life".into(), "stopped".into());
    // With its place standing, so the transition has a region to command (74).
    item.config.insert("place".into(), "place".into());
    flywheel_engine::initialise(defs, &mut item, at);
    let base = item.seq;
    store.put(ITEM, &item, base).expect("the item stopped");
}

const ITEM: &str = "work-item/atlas/u/1";

/// Tick until nothing more fires, collecting what moved and what was performed.
fn settle<S: StateStore + flywheel_engine::runtime::EvidenceSource>(
    store: &mut S,
    defs: &Definitions,
) -> (Vec<String>, Vec<String>) {
    let mut moves = vec![];
    let mut acts = vec![];
    for _ in 0..8 {
        let mut fired = 0;
        let ticked = commands::tick(
            store,
            defs,
            &flywheel_atoms::Scope::All,
            |_store: &mut _, object: &str, _region: &str, effect: &PlannedEffect| {
                acts.push(format!("{object} {}", effect.name));
                true
            },
            |_store: &mut _, f: &flywheel_engine::tick::Fired, _tail| {
                fired += 1;
                moves.push(format!("{} {} {}→{}", f.object, f.region, f.from, f.to));
            },
        )
        .expect("a tick");
        if ticked.transitions == 0 {
            break;
        }
    }
    (moves, acts)
}

/// What the object did, either way: the same transition, with the same effects
/// and the same commanded regions, and the act recorded like any response
/// (4, 12).
fn what_happened(answered: bool) -> (Vec<String>, Vec<String>, Object) {
    let mut store = flywheel_atoms::testing::FakeStore::default();
    let mut world = world::Files::new();
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    a_stalled_item(&mut store, &defs);

    let call = if answered {
        // The decision the machine offers, by the number the register gave it.
        let standing = commands::rail(&mut store, &defs).expect("the rail");
        let decision = standing
            .iter()
            .find(|d| d.object == ITEM && d.kind == "stalled")
            .expect("the item's decision stands");
        assert!(
            decision.answers.iter().any(|a| a == "drop"),
            "the decision offers {:?}",
            decision.answers
        );
        let number = decision.number.expect("the register numbered it");
        Call::new("answer", "chuck", "page")
            .arg("decision", json!(number))
            .arg("answer", json!("drop"))
    } else {
        // The same act, outside the decision entirely.
        Call::new("drop", "chuck", "chat").arg("object", json!(ITEM))
    };
    let outcome = catalogue::call(&mut store, &mut world, &defs, &call).expect("the call");

    // What happened to the item: the rest of the world was ticking either way,
    // and the claim is about the object the operator acted on.
    let (moves, acts) = settle(&mut store, &defs);
    let moves = moves.into_iter().filter(|m| m.starts_with(ITEM)).collect();
    let acts = acts.into_iter().filter(|a| a.starts_with(ITEM)).collect();
    let record = store
        .get(&format!("response/{}", outcome.id))
        .expect("a read")
        .expect("the act was recorded like any response");
    (moves, acts, record)
}

#[test]
fn dictation_takes_the_decisions_transition() {
    let (answered_moves, answered_acts, answered_record) = what_happened(true);
    let (dictated_moves, dictated_acts, dictated_record) = what_happened(false);

    let took = format!("{ITEM} life stopped→dropped");
    assert!(
        answered_moves.contains(&took),
        "answering the decision did not drop the item: {answered_moves:?}"
    );
    assert_eq!(
        dictated_moves, answered_moves,
        "the dictation took a different transition from the decision's"
    );
    assert_eq!(
        dictated_acts, answered_acts,
        "the dictation had different effects from the decision's"
    );
    // The `enter:` on the transition reaches the item's own regions either way,
    // so the dictation releases the place the decision's answer would have (74).
    assert!(
        dictated_acts.iter().any(|a| a.ends_with("remove_place")),
        "the dictation commanded no remove_place: {dictated_acts:?}"
    );

    // And the act is recorded like any response, naming the tool that made it
    // and who gave it (12, 153, I1).
    assert_eq!(
        dictated_record.record.get("tool").and_then(|v| v.as_str()),
        Some("drop")
    );
    assert_eq!(
        dictated_record.record.get("object").and_then(|v| v.as_str()),
        Some(ITEM)
    );
    assert_eq!(
        answered_record.record.get("tool").and_then(|v| v.as_str()),
        Some("answer")
    );
}

/// Start and stop are the pair clause 47 grants beyond undo-or-defer, and they
/// are dictations like the rest: one call, one response, on the service named.
#[test]
fn service_start_and_stop_are_dictations() {
    let mut store = flywheel_atoms::testing::FakeStore::default();
    let mut world = world::Files::new();
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    for tool in ["start", "stop"] {
        let call = Call::new(tool, "chuck", "page").arg("service", json!("service/atlas/web"));
        let outcome = catalogue::call(&mut store, &mut world, &defs, &call).expect("the call");
        let record = store
            .get(&format!("response/{}", outcome.id))
            .expect("a read")
            .unwrap_or_else(|| panic!("`{tool}` wrote no response"));
        assert_eq!(record.record.get("tool").and_then(|v| v.as_str()), Some(tool));
        assert_eq!(
            record.record.get("object").and_then(|v| v.as_str()),
            Some("service/atlas/web"),
            "`{tool}` named another object"
        );
    }
}
