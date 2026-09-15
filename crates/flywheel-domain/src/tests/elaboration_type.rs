//! An elaboration's type resolves: material names no type of its own, so a
//! proposal from attached material, and any proposal that names none, is
//! self-closing until the operator's `type <name>` names another, and the type
//! the record holds at the yes is the one its session starts under (27, 85a;
//! `intent.yaml` material, `elaboration.yaml` proposed and working).

use crate::{blueprints, commands, effects};
use chrono::{DateTime, TimeZone, Utc};
use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::Records;
use serde_json::json;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap()
}

/// An open intent holding one attached signal as material.
fn an_open_intent(store: &mut FakeStore, defs: &flywheel_engine::Definitions, id: &str) {
    let record = [("signals".to_string(), json!(["signal/rows-1"]))].into_iter().collect();
    commands::put_new(store, defs, id, "intent", None, record, now()).unwrap();
    let mut held = store.get(id).unwrap().unwrap();
    held.config.insert("life".into(), "open".into());
    flywheel_engine::initialise(defs, &mut held, now());
    let base = held.seq;
    store.put(id, &held, base).unwrap();
}

/// A proposal from the intent's attached material is self-closing, at the
/// version the registry holds, and so is one that names no type at all; the
/// type it holds is one the registry defines, so its yes is not refused (27,
/// 85a).
#[test]
fn a_proposal_from_attached_material_is_self_closing() {
    let defs = crate::set::load().unwrap();
    let mut store = FakeStore::default();
    an_open_intent(&mut store, &defs, "intent/rows");
    let proposed = effects::propose_elaboration(&mut store, &defs, "intent/rows", "from-material", now()).unwrap();
    let held = store.get(&proposed).unwrap().expect("the proposed elaboration");
    assert_eq!(held.record.get("type"), Some(&json!("self-closing")));
    assert_eq!(held.record.get("type_version"), Some(&json!(2)));
    assert!(blueprints::type_defined(&defs, held.record.get("type").and_then(|v| v.as_str())));

    an_open_intent(&mut store, &defs, "intent/exports");
    let untyped = effects::propose_elaboration(&mut store, &defs, "intent/exports", "", now()).unwrap();
    assert_eq!(store.get(&untyped).unwrap().unwrap().record.get("type"), Some(&json!("self-closing")));
}

/// `type <name>` before the yes names the type the elaboration runs: the record
/// holds the corrected name at its version, and the working state's machine is
/// that type's, which is what its session starts under (27, `elaboration.yaml`
/// working `machine: $type`).
#[test]
fn type_names_another_before_the_yes() {
    let defs = crate::set::load().unwrap();
    let mut store = FakeStore::default();
    an_open_intent(&mut store, &defs, "intent/rows");
    let proposed = effects::propose_elaboration(&mut store, &defs, "intent/rows", "from-material", now()).unwrap();
    effects::set_type(&mut store, &proposed, "type with-operator").unwrap();

    let held = store.get(&proposed).unwrap().unwrap();
    assert_eq!(held.record.get("type"), Some(&json!("with-operator")));
    assert_eq!(held.record.get("type_version"), Some(&json!(3)), "the version travels with the name");
    let working = defs
        .get("elaboration")
        .and_then(|m| m.regions.get("life"))
        .and_then(|r| r.states.get("working"))
        .expect("the elaboration's working state");
    let runs = flywheel_engine::tick::submachine(&defs, working, &held).expect("the type's machine");
    assert!(
        std::ptr::eq(runs, defs.get("with-operator").expect("with-operator is a type")),
        "the session would start under another type than the one named"
    );
    assert!(!std::ptr::eq(runs, defs.get("self-closing").unwrap()), "the proposal's own type still runs");
}
