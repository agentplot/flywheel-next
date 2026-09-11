//! The capture and the signal as the catalogue's tools write them: one capture
//! per source event, and a signal no tool rewrites
//! (111, 113, 193, 203).

use chrono::{DateTime, Duration, TimeZone, Utc};
use flywheel_atoms::testing::{FakeStore, FakeWorld};
use flywheel_atoms::{Records, World};
use crate::catalogue::{self, Call};
use flywheel_domain::signals::{self, Capture};
use serde_json::json;

fn at(minute: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap() + Duration::minutes(minute)
}

/// A store, a world and the set behind them. No repository: what these assert
/// is the record and the material, and neither is git's (D17).
fn a_store() -> (FakeStore, FakeWorld, flywheel_engine::Definitions) {
    (
        FakeStore::default(),
        FakeWorld::new(),
        flywheel_domain::set::load().unwrap(),
    )
}

/// A capture is one source event: it carries its provenance and a pointer to
/// the raw material, it lives under the machinery's prefix in the blueprints,
/// and capturing the same event twice yields one (111, 203).
#[test]
fn capture_keyed_once() {
    let (mut store, mut world, defs) = a_store();

    let capture = Capture {
        key: "meeting/2026-09-02/willdan-weekly".into(),
        source: "meeting".into(),
        event_at: at(0).to_rfc3339(),
        captured_by: "chuck".into(),
        // A pointer, not the transcript: the raw material stays outside every
        // repository and the capture cites it (111).
        raw: "raw://meetings/2026-09-02-willdan-weekly.vtt".into(),
    };
    assert!(
        signals::write_capture(&mut world, &capture).unwrap(),
        "the first capture wrote nothing"
    );

    // Under the machinery's prefix, in the blueprints, and nowhere else (203).
    let path = signals::capture_path(&capture.key);
    assert_eq!(
        path,
        "flywheel/signals/captures/meeting/2026-09-02/willdan-weekly.rec"
    );
    assert!(path.starts_with("flywheel/"), "outside the prefix: {path}");
    let written: Vec<String> = world
        .files
        .keys()
        .filter(|p| !p.starts_with("flywheel/"))
        .cloned()
        .collect();
    assert!(written.is_empty(), "written outside the prefix: {written:?}");

    // Its provenance and its pointer, read back as they stand.
    let held = signals::read_capture(&world, &capture.key)
        .unwrap()
        .expect("the capture");
    assert_eq!(held, capture);
    let body = String::from_utf8(
        world
            .read_file(signals::BLUEPRINTS, &path)
            .unwrap()
            .expect("the record"),
    )
    .unwrap();
    assert!(body.contains(signals::CAPTURE_FORMAT), "{body}");
    // The record cites the raw material and does not hold it (111).
    assert!(body.contains("raw://meetings/"), "{body}");
    assert!(
        !body.contains("WEBVTT") && body.lines().count() < 12,
        "the record holds the material rather than a pointer to it: {body}"
    );

    // The same source event again: one capture, and the second write reports
    // itself as no write at all (111, 127).
    let again = Capture {
        captured_by: "someone else".into(),
        event_at: at(60 * 24).to_rfc3339(),
        ..capture.clone()
    };
    assert!(
        !signals::write_capture(&mut world, &again).unwrap(),
        "the same source event was captured twice"
    );
    assert_eq!(signals::captures(&world).unwrap().len(), 1);
    assert_eq!(
        signals::read_capture(&world, &capture.key).unwrap().unwrap(),
        capture,
        "the second capture overwrote the first"
    );

    // And through the tool the page's box and the chat forward both call: one
    // keyed capture, and the delivery recorded once (111, 193).
    let call = Call::new("capture", "chuck", "chat")
        .keyed("message/1421")
        .delivered("chat-1421")
        .arg("text", json!("discord message link 1421"))
        .arg("source", json!("forwarded-message"));
    catalogue::call(&mut store, &mut world, &defs, &call).unwrap();
    catalogue::call(&mut store, &mut world, &defs, &call).unwrap();
    assert_eq!(
        signals::captures(&world).unwrap().len(),
        2,
        "the same forwarded message made two captures"
    );
    let forwarded = signals::read_capture(&world, "message/1421")
        .unwrap()
        .expect("the forward's capture");
    assert_eq!(forwarded.source, "forwarded-message");
    assert_eq!(forwarded.captured_by, "chuck");
    assert_eq!(forwarded.raw, "discord message link 1421");
    // One object too, for the engine to tick: a key names its source first and
    // then the event, and an object id names one object, so the separators are
    // flattened (S21, S22).
    assert_eq!(signals::object_of("message/1421"), "capture/message-1421");
    assert!(
        Records::get(&store, "capture/message-1421")
            .unwrap()
            .is_some(),
        "the capture object was not written"
    );
}
/// A signal carries what it asserts and is immutable once written: no tool
/// edits one, and its record is never rewritten in history. A correction is a
/// new signal or a change of move (113, 193).
#[test]
fn signal_is_never_rewritten() {
    let mut world = FakeWorld::new();

    let signal = signals::Signal {
        id: "signal/meeting-2026-09-02-willdan-weekly".into(),
        capture: "capture/meeting-2026-09-02-willdan-weekly".into(),
        // Its kind, from the small fixed set (113).
        kind: "constraint".into(),
        asserted_by: "dana".into(),
        subject_tags: vec!["providers".into(), "leases".into()],
        assertion: "only one host may write to a provider at a time".into(),
        excerpt: "That is the one-writer claim, isn't it.".into(),
        position: "00:04:29.400".into(),
        argues_with: vec!["providers/one-writer@3".into()],
    };
    let key = "meeting/2026-09-02/willdan-weekly";
    assert!(signals::write_signal(&mut world, key, 1, &signal).unwrap());

    // Everything 113 asks for is in the record, and it reads back whole.
    let read = signals::signals_of(&world, key).unwrap();
    assert_eq!(read, vec![signal.clone()], "the record lost a field");
    assert!(
        signals::KINDS.contains(&signal.kind.as_str()),
        "a kind outside the fixed set (113)"
    );

    let path = signals::signal_path(key, 1);
    assert_eq!(
        world.writes_to(&path),
        1,
        "the signal's record took more than one commit"
    );

    // A second write of the same signal, saying something else, is not made:
    // the record already there stands (113).
    let corrected = signals::Signal {
        assertion: "two hosts may write, actually".into(),
        ..signal.clone()
    };
    assert!(
        !signals::write_signal(&mut world, key, 1, &corrected).unwrap(),
        "a signal was rewritten"
    );
    assert_eq!(signals::signals_of(&world, key).unwrap(), vec![signal]);
    assert_eq!(
        world.writes_to(&path),
        1,
        "the signal's record was rewritten in history (113, 167)"
    );

    // And no tool of the catalogue edits one: `revive` clears a move, which is
    // the move changing and never the signal (107, 113, 193).
    for tool in catalogue::catalogue() {
        assert!(
            !matches!(tool.name, "edit-signal" | "amend-signal" | "correct-signal"),
            "the catalogue carries `{}`",
            tool.name
        );
        if tool.args.contains(&"signal") {
            assert_eq!(
                tool.name, "revive",
                "`{}` takes a signal; only `revive` may, and it clears the move (107)",
                tool.name
            );
        }
    }
}
