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
    write_report(
        store,
        session,
        "operator",
        now(),
        &Report::Offer {
            kind: kind.into(),
            document: document.into(),
        },
    )
    .unwrap();
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
    offer(&mut store, session, "chore", chore);

    let made = offers::record(&mut store, &mut world, &defs, session, "curation/main", now()).unwrap();
    assert_eq!(made.len(), 2, "one record per offer: {made:?}");
    assert!(offers::pending(&store, session).unwrap().is_empty(), "nothing is left to stop the session");

    for (id, document) in made.iter().zip([finding, chore]) {
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
