//! What a session offered becomes one record pointing at its document, whatever
//! stands above the session (`record-derived.yaml` record_offers, 58, 62).

use chrono::{DateTime, TimeZone, Utc};
use flywheel_atoms::testing::{FakeStore, FakeWorld};
use flywheel_atoms::{Records, World};
use crate::report::{write_report, Report};
use crate::{commands, offers, signals};
use serde_json::json;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap()
}

fn offer(store: &mut FakeStore, session: &str, kind: &str, document: &str) {
    offer_scoped(store, session, kind, document, None);
}

fn offer_scoped(store: &mut FakeStore, session: &str, kind: &str, document: &str, scope: Option<&str>) {
    offer_at(store, session, kind, document, scope, None);
}

fn offer_at(store: &mut FakeStore, session: &str, kind: &str, document: &str, scope: Option<&str>, revision: Option<&str>) {
    write_report(
        store,
        session,
        "operator",
        now(),
        &Report::Offer {
            kind: kind.into(),
            document: document.into(),
            scope: scope.map(String::from),
            about: None,
            revision: revision.map(String::from),
        },
    )
    .unwrap();
}

/// A unit made of a chore offer keeps the revision its offer named beside the
/// document, since the record never holds the text and the document is read at
/// that revision wherever the chore is taken (62, `unit.yaml` record).
#[test]
fn a_chore_unit_keeps_its_offers_revision() {
    let mut store = FakeStore::default();
    let mut world = FakeWorld::new().tracking("atlas");
    let defs = crate::set::load().unwrap();
    commands::put_new(&mut store, &defs, "curation/main", "curation", None, Default::default(), now()).unwrap();
    let session = "session/curation/main/1";
    let document = "flywheel/curation/chores/agents-md.md";
    let revision = "4dacc66a0e5f1b2c3d4e5f60718293a4b5c6d7e8";
    offer_at(&mut store, session, "chore", document, Some("atlas"), Some(revision));

    let made = offers::record(&mut store, &mut world, &defs, session, "curation/main", now()).unwrap();
    assert_eq!(made, vec!["unit/atlas/chore-1".to_string()]);
    let unit = store.get(&made[0]).unwrap().expect("the chore unit");
    assert_eq!(unit.record.get("document"), Some(&json!(document)));
    assert_eq!(unit.record.get("revision"), Some(&json!(revision)));
}

/// An offer with neither an intent nor a bolt above the session is a signal
/// citing the path (62). Left unrecorded it stayed pending on every pass,
/// `record_offers` won the session's `working` region every time, and a
/// curation session that offered anything never reached its exit.
#[test]
fn an_offer_no_object_is_above_becomes_a_signal() {
    let mut store = FakeStore::default();
    let mut world = FakeWorld::new();
    let defs = crate::set::load().unwrap();
    commands::put_new(&mut store, &defs, "curation/main", "curation", None, Default::default(), now()).unwrap();
    let session = "session/curation/main/1";
    let finding = "flywheel/curation/findings/stale-claims.md";
    let chore = "flywheel/curation/chores/rename-tags.md";
    offer(&mut store, session, "finding", finding);
    // A chore naming no repository has nowhere to land off every bolt. The
    // command refuses it before writing; one on the thread anyway is refused
    // there, and is no signal (60, S231).
    offer(&mut store, session, "chore", chore);

    let made = offers::record(&mut store, &mut world, &defs, session, "curation/main", now()).unwrap();
    assert_eq!(made.len(), 1, "one record, for the finding: {made:?}");
    assert!(offers::pending(&store, session).unwrap().is_empty(), "nothing is left to stop the session");
    let thread = store.thread(session).unwrap();
    let refusal = thread.last().expect("the refusal is on the thread");
    assert_eq!(refusal.fields.get("refuses"), Some(&json!(format!("{session}#1"))));
    assert!(
        refusal.fields.get("refused").and_then(|v| v.as_str()).is_some_and(|r| r.contains("blueprints")),
        "the refusal names where a chore may land: {refusal:?}"
    );

    for (id, document) in made.iter().zip([finding]) {
        let signal = store.get(id).unwrap().expect("the signal is on record");
        assert_eq!(signal.machine, "signal");
        assert_eq!(signal.record.get("document"), Some(&json!(document)));
        assert_eq!(signal.record.get("sources").and_then(|s| s.as_array()).map(Vec::len), Some(1));
        // The record never holds the text: it says where the document is.
        assert_eq!(signal.record.get("assertion"), Some(&json!(document)));
        assert_eq!(signal.record.get("excerpt"), Some(&json!("")));

        // A signal carries its capture, and the offer is that capture: its
        // material is the document, and both are in the blueprints (111, 113).
        let capture = signal.parent.clone().expect("a signal is owned by a capture");
        let held = store.get(&capture).unwrap().expect("the capture is on record");
        assert_eq!(held.record.get("source"), Some(&json!(offers::SOURCE)));
        assert_eq!(held.record.get("raw"), Some(&json!(document)));
        let key = signals::key_of_capture(&store, &capture).unwrap();
        assert!(world.read_file(signals::BLUEPRINTS, &signals::capture_path(&key)).unwrap().is_some());
        assert!(world.read_file(signals::BLUEPRINTS, &signals::signal_path(&key, 1)).unwrap().is_some());
    }

    let again = offers::record(&mut store, &mut world, &defs, session, "curation/main", now()).unwrap();
    assert!(again.is_empty(), "a recorded offer is not recorded twice: {again:?}");
}

/// Under a capture, an offer is that capture's next signal and no capture of
/// its own (113).
#[test]
fn an_offer_under_a_capture_is_that_captures_signal() {
    let mut store = FakeStore::default();
    let mut world = FakeWorld::new();
    let defs = crate::set::load().unwrap();
    let key = "meeting/2026-09-02/weekly";
    let capture = signals::object_of(key);
    commands::put_new(
        &mut store,
        &defs,
        &capture,
        "capture",
        None,
        [("event_key".to_string(), json!(key))].into_iter().collect(),
        now(),
    )
    .unwrap();
    signals::write_signal(&mut world, key, 1, &signals::Signal { id: signals::signal_object(key, 1), ..Default::default() }).unwrap();
    let session = "session/capture/meeting-2026-09-02-weekly/1";
    offer(&mut store, session, "finding", "flywheel/signals/drafts/weekly-2.md");

    let made = offers::record(&mut store, &mut world, &defs, session, &capture, now()).unwrap();
    assert_eq!(made, vec![signals::signal_object(key, 2)]);
    assert_eq!(store.get(&made[0]).unwrap().and_then(|s| s.parent), Some(capture.clone()));
    let captures = store
        .list_records(&flywheel_atoms::Scope::All)
        .unwrap()
        .into_iter()
        .filter(|o| o.machine == "capture")
        .count();
    assert_eq!(captures, 1, "the offer made no capture of its own");
}

/// The one record a signal offer makes: a signal citing the document, of a
/// capture of its own of source offer, and nothing left pending (62, 111).
fn assert_the_offer_is_a_signal(store: &FakeStore, made: &[String], session: &str, document: &str) {
    assert_eq!(made.len(), 1, "one record: {made:?}");
    let signal = store.get(&made[0]).unwrap().expect("the signal is on record");
    assert_eq!(signal.machine, "signal");
    assert_eq!(signal.record.get("document"), Some(&json!(document)));
    assert_eq!(signal.record.get("assertion"), Some(&json!(document)));
    let capture = signal.parent.clone().expect("a signal is owned by a capture");
    let held = store.get(&capture).unwrap().expect("the capture is on record");
    assert_eq!(held.record.get("source"), Some(&json!(offers::SOURCE)));
    assert!(offers::pending(store, session).unwrap().is_empty(), "nothing is left to stop the session");
}

/// A signal offered under a bolt is about neither the session's intent nor its
/// bolt, so it is a signal and no proposed unit on the bolt; its scope is not
/// read, so one naming no tracked repository is no refusal (58, 62, S231).
#[test]
fn a_signal_offered_under_a_bolt_is_a_signal() {
    let mut store = FakeStore::default();
    let mut world = FakeWorld::new();
    let defs = crate::set::load().unwrap();
    let repository = [("repository".to_string(), json!("atlas"))].into_iter().collect();
    commands::put_new(&mut store, &defs, "bolt/atlas-rows", "bolt", None, repository, now()).unwrap();
    commands::put_new(&mut store, &defs, "unit/atlas-rows/plan-rows", "unit", Some("bolt/atlas-rows"), Default::default(), now()).unwrap();
    let session = "session/unit/atlas-rows/plan-rows/fix/1";
    let document = "notes/provider-limits-elsewhere.md";
    offer_scoped(&mut store, session, "signal", document, Some("switchboard"));

    let made = offers::record(&mut store, &mut world, &defs, session, "unit/atlas-rows/plan-rows", now()).unwrap();
    assert_the_offer_is_a_signal(&store, &made, session, document);
    let units = store.list_records(&flywheel_atoms::Scope::Machine("unit".into())).unwrap();
    assert_eq!(units.len(), 1, "no unit is made on the bolt: {units:?}");
    assert!(
        store.thread(session).unwrap().iter().all(|e| !e.fields.contains_key("refuses")),
        "a signal's scope is not read, so nothing is refused"
    );
}

/// A signal offered under an intent is a signal and no proposed elaboration
/// there (58, 62, S231).
#[test]
fn a_signal_offered_under_an_intent_is_a_signal() {
    let mut store = FakeStore::default();
    let mut world = FakeWorld::new();
    let defs = crate::set::load().unwrap();
    commands::put_new(&mut store, &defs, "intent/atlas-provider-limits", "intent", None, Default::default(), now()).unwrap();
    let owner = "elaboration/atlas-provider-limits/research-1";
    commands::put_new(&mut store, &defs, owner, "elaboration", Some("intent/atlas-provider-limits"), Default::default(), now()).unwrap();
    let session = "session/elaboration/atlas-provider-limits/research-1/1";
    let document = "notes/billing-copy-drift.md";
    offer(&mut store, session, "signal", document);

    let made = offers::record(&mut store, &mut world, &defs, session, owner, now()).unwrap();
    assert_the_offer_is_a_signal(&store, &made, session, document);
    let elaborations = store.list_records(&flywheel_atoms::Scope::Machine("elaboration".into())).unwrap();
    assert_eq!(elaborations.len(), 1, "no elaboration is made on the intent: {elaborations:?}");
}

/// A chore offered off every bolt is a proposed chore unit of the repository
/// its scope names, folded with that repository's other shared-line chores into
/// one decision; no capture or signal is written, and a route naming the offer
/// names the unit it became (60, 62, 11, 116, S231).
#[test]
fn a_chore_offered_off_every_bolt_is_a_unit_of_its_repository() {
    let mut store = FakeStore::default();
    let mut world = FakeWorld::new().tracking("atlas");
    let defs = crate::set::load().unwrap();
    commands::put_new(&mut store, &defs, "curation/main", "curation", None, Default::default(), now()).unwrap();
    let session = "session/curation/main/1";
    let first = "flywheel/curation/chores/agents-md.md";
    let second = "flywheel/curation/chores/rename-ref.md";
    offer_scoped(&mut store, session, "chore", first, Some("atlas"));
    offer_scoped(&mut store, session, "chore", second, Some("atlas"));

    let made = offers::record(&mut store, &mut world, &defs, session, "curation/main", now()).unwrap();
    assert_eq!(made, vec!["unit/atlas/chore-1".to_string(), "unit/atlas/chore-2".to_string()]);
    for (id, document) in made.iter().zip([first, second]) {
        let unit = store.get(id).unwrap().expect("the chore unit is on record");
        assert_eq!(unit.machine, "unit");
        assert_eq!(unit.parent.as_deref(), Some("repository/atlas"), "it stands under its repository");
        assert_eq!(unit.config.get("life").map(String::as_str), Some("proposed"));
        for (field, value) in [
            ("type", json!("chore")),
            ("batch", json!("atlas")),
            ("repository", json!("atlas")),
            ("document", json!(document)),
        ] {
            assert_eq!(unit.record.get(field), Some(&value), "{id} {field}");
        }
        assert_eq!(unit.record.get("sources").and_then(|s| s.as_array()).map(Vec::len), Some(1));
    }
    let written = store
        .list_records(&flywheel_atoms::Scope::All)
        .unwrap()
        .into_iter()
        .filter(|o| matches!(o.machine.as_str(), "capture" | "signal"))
        .count();
    assert_eq!(written, 0, "a chore is no capture and no signal");
    assert!(world.under("flywheel/signals/").is_empty(), "nothing is written into the signals");
    assert!(offers::pending(&store, session).unwrap().is_empty());

    let standing = commands::rail(&mut store, &defs).unwrap();
    let chores: Vec<_> = standing.iter().filter(|d| d.kind == "unit-proposed").collect();
    assert_eq!(chores.len(), 1, "one decision for the repository's chores: {chores:?}");
    assert_eq!(chores[0].folds.len(), 2);

    let signal = signals::signal_object("page-1", 1);
    commands::put_new(&mut store, &defs, &signal, "signal", Some("capture/page-1"), Default::default(), now()).unwrap();
    let moved = signals::Move {
        signal: signal.clone(),
        target: format!("route {session}#0"),
        reason: "curation offered a chore for it".into(),
        at: now().to_rfc3339(),
    };
    signals::apply_move(&mut store, &mut world, &moved, now()).unwrap();
    assert_eq!(
        store.get(&signal).unwrap().unwrap().record.get("route"),
        Some(&json!("unit/atlas/chore-1")),
        "the route names the unit the offer became (116)"
    );
}

/// A chore of the blueprints stands under the instance with the blueprints
/// named as its repository, folds under that name, and every host covers it,
/// as every host clones the blueprints (123, 149, 205).
#[test]
fn a_blueprints_chore_stands_under_the_instance() {
    let mut store = FakeStore::default();
    let mut world = FakeWorld::new();
    let defs = crate::set::load().unwrap();
    commands::put_new(&mut store, &defs, "instance/willdan", "instance", None, Default::default(), now()).unwrap();
    commands::put_new(&mut store, &defs, "curation/main", "curation", None, Default::default(), now()).unwrap();
    let session = "session/curation/main/1";
    offer_scoped(&mut store, session, "chore", "flywheel/curation/chores/skill-typo.md", Some("blueprints"));

    let made = offers::record(&mut store, &mut world, &defs, session, "curation/main", now()).unwrap();
    assert_eq!(made, vec!["unit/blueprints/chore-1".to_string()]);
    let unit = store.get(&made[0]).unwrap().expect("the chore unit");
    assert_eq!(unit.parent.as_deref(), Some("instance/willdan"));
    assert_eq!(unit.record.get("repository"), Some(&json!("blueprints")));
    assert_eq!(unit.record.get("batch"), Some(&json!("blueprints")));

    let declaration = crate::derived::Declaration {
        repositories: vec!["atlas".into()],
        types: vec![],
        kinds: vec!["all".into()],
    };
    assert!(declaration.covers(&unit), "a host that declares its repositories still takes the blueprints' chores");
}
