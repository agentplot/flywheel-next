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

/// A caller posting the same delivery twice — a monitor's webhook retrying —
/// captures once: the second call is acknowledged and writes nothing (137,
/// 111).
#[test]
fn a_delivery_captured_twice_is_one_capture() {
    let (mut store, mut world, defs) = a_store();
    let finding = || {
        Call::new("capture", "chuck", "page")
            .arg("text", json!("synthetic monitor: the gateway's 5xx rate held at 3.1% for 10 minutes"))
            .arg("source", json!("datadog"))
            .delivered("datadog-4417")
    };
    catalogue::call(&mut store, &mut world, &defs, &finding()).expect("the first delivery");
    let captures = |store: &FakeStore| {
        store
            .list_records(&flywheel_atoms::Scope::All)
            .unwrap()
            .into_iter()
            .filter(|o| o.machine == "capture")
            .map(|o| (o.id, o.record.get("source").cloned()))
            .collect::<Vec<_>>()
    };
    assert_eq!(captures(&store).len(), 1, "{:?}", captures(&store));
    assert_eq!(captures(&store)[0].1, Some(json!("datadog")));

    let writes = store.writes();
    catalogue::call(&mut store, &mut world, &defs, &finding()).expect("the same delivery again");
    assert_eq!(captures(&store).len(), 1, "a repeated delivery captured again: {:?}", captures(&store));
    assert_eq!(store.writes(), writes, "a repeated delivery wrote to the store");
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

    // And no tool of the catalogue edits one: `attach-signal` and `drop-signal`
    // write its move and `revive` clears it, which is the move changing and
    // never the signal (107, 113, 193).
    for tool in catalogue::catalogue() {
        assert!(
            !matches!(tool.name, "edit-signal" | "amend-signal" | "correct-signal"),
            "the catalogue carries `{}`",
            tool.name
        );
        if tool.args.contains(&"signal") {
            assert!(
                matches!(tool.name, "revive" | "attach-signal" | "drop-signal"),
                "`{}` takes a signal; only a tool that moves it or clears its move may (107)",
                tool.name
            );
        }
    }
}

// ------------------------------------------------ the operator's hand on a capture

/// A note typed in the capture box, and its one signal (19).
fn a_note(
    store: &mut FakeStore,
    world: &mut FakeWorld,
    defs: &flywheel_engine::Definitions,
    text: &str,
) -> (String, String) {
    let call = Call::new("capture", "chuck", "page")
        .arg("text", json!(text))
        .arg("source", json!("console"));
    let outcome = catalogue::call(store, world, defs, &call).expect("the capture is taken");
    let capture = outcome.journal.iter().find(|n| n.kind == "capture").expect("a capture").object.clone();
    let signal = signals::of_capture(store, &capture).unwrap().into_iter().next().expect("its signal");
    (capture, signal)
}

/// `make an intent`: the intent opens at once, named from the capture's first
/// words with the call as its approval and no decision of its own; the signal's
/// move attaches it there, and the intent's material proposes its first
/// elaboration from it on the next tick (12, 19a, 21).
#[test]
fn open_intent_opens_at_once_and_attaches_the_signal() {
    let (mut store, mut world, defs) = a_store();
    let (capture, signal) = a_note(&mut store, &mut world, &defs, "keep the row numbers on every page");

    let call = Call::new("open-intent", "chuck", "page").arg("capture", json!(capture));
    let outcome = catalogue::call(&mut store, &mut world, &defs, &call).expect("the intent opens");

    let intent = "intent/keep-row-numbers-every";
    let held = store.get(intent).unwrap().expect("the intent is named from the capture's words");
    assert_eq!(held.config.get("life").map(String::as_str), Some("open"), "the dictation skips proposed (12)");
    assert_eq!(held.config.get("life.open.material").map(String::as_str), Some("settled"));
    assert_eq!(held.record.get("approval"), Some(&json!(outcome.id)), "the call is its approval (I1)");
    assert_eq!(held.record.get("signals"), Some(&json!([signal.clone()])), "the intent cites the signal");
    let moved = signals::standing_move(&world, &signal).unwrap().expect("the signal moved");
    assert_eq!(moved.target, format!("attach {intent}"));
    let response = store.get(&format!("response/{}", outcome.id)).unwrap().expect("the call is recorded");
    assert_eq!(response.record.get("tool"), Some(&json!("open-intent")));
    let rail = flywheel_domain::commands::rail(&mut store, &defs).unwrap();
    assert!(rail.iter().all(|d| d.object != intent), "the intent raised a decision: {rail:?}");

    // The signal is the intent's pending material, which is what its open state
    // proposes the first elaboration from on the next tick (21,
    // `record-derived.yaml` intent.material_pending).
    assert_eq!(
        flywheel_domain::effects::pending_material(&store, intent).unwrap(),
        vec![signal.clone()],
        "the intent holds the signal as material to propose from"
    );
    let first = flywheel_domain::effects::propose_elaboration(&mut store, &defs, intent, "from-material", at(1))
        .expect("the elaboration is proposed");
    assert_eq!(store.get(&first).unwrap().expect("the elaboration").record.get("signals"), Some(&json!([signal])));

    // Its signal has moved, so the control has nothing left to move (107).
    assert!(catalogue::call(&mut store, &mut world, &defs, &call).is_err(), "a signal took a second move");
}

/// `add to bolt…`: the bolt is the one the operator picked, the unit is named
/// from the capture's own words, it stands approved on that bolt depending on
/// nothing, and the capture's signal routes to it — no second bolt is made and
/// no name is typed (34, 31, 12, 116, S224a).
#[test]
fn a_pick_puts_an_approved_unit_on_that_bolt_and_routes_the_signal() {
    let (mut store, world, defs) = a_store();
    let mut world = world.tracking("atlas");
    let bolt = "bolt/atlas/plan-rows";
    let at = flywheel_domain::commands::now(&store).expect("a point");
    let record = [("repository".to_string(), json!("atlas")), ("name".to_string(), json!("plan-rows"))];
    flywheel_domain::commands::put_new(&mut store, &defs, bolt, "bolt", None, record.into_iter().collect(), at)
        .expect("the open bolt");
    let (capture, signal) = a_note(&mut store, &mut world, &defs, "the rows lose their numbers on the second page");

    let call = Call::new("propose-unit", "chuck", "page")
        .arg("capture", json!(capture))
        .arg("bolt", json!(bolt));
    catalogue::call(&mut store, &mut world, &defs, &call).expect("the capture goes to the bolt");

    let of = |store: &FakeStore, machine: &str| -> Vec<flywheel_atoms::Object> {
        store.list_records(&flywheel_atoms::Scope::All).unwrap().into_iter().filter(|o| o.machine == machine).collect()
    };
    let bolts = of(&store, "bolt");
    assert_eq!(bolts.len(), 1, "a second bolt was made for a bolt already open: {:?}", bolts.iter().map(|b| &b.id).collect::<Vec<_>>());
    let units = of(&store, "unit");
    assert_eq!(units.len(), 1, "{:?}", units.iter().map(|u| &u.id).collect::<Vec<_>>());
    let unit = &units[0];
    assert_eq!(unit.id, "unit/atlas/rows-lose-their-numbers", "the unit is not named from the capture's first words");
    assert_eq!(unit.parent.as_deref(), Some(bolt), "the unit stands on another bolt");
    assert_eq!(unit.config.get("life").map(String::as_str), Some("approved"), "the unit is not approved by the call");
    assert_eq!(unit.record.get("target").and_then(|t| t.get("bolt")), Some(&json!(bolt)));
    assert!(unit.record.get("approval").is_some(), "the call is not the unit's approval (I1)");
    assert!(
        unit.record.get("depends_on").and_then(|v| v.as_array()).is_none_or(|on| on.is_empty()),
        "the unit waits on something (31)"
    );
    let moved = signals::standing_move(&world, &signal).unwrap().expect("the signal moved");
    assert_eq!(moved.target, format!("route {}", unit.id));
}

/// A bolt whose close is offered or held is listed and adding to it takes the
/// offer back: new work arrived, so the close cannot be answered as it was
/// posed (S224a, `bolt.yaml` close).
#[test]
fn adding_to_a_bolt_whose_close_is_offered_takes_the_offer_back() {
    let (mut store, world, defs) = a_store();
    let mut world = world.tracking("atlas");
    let bolt = "bolt/atlas/plan-rows";
    let at = flywheel_domain::commands::now(&store).expect("a point");
    let record = [("repository".to_string(), json!("atlas")), ("name".to_string(), json!("plan-rows"))];
    flywheel_domain::commands::put_new(&mut store, &defs, bolt, "bolt", None, record.into_iter().collect(), at)
        .expect("the open bolt");
    let close = |store: &mut FakeStore, state: &str| {
        let mut held: flywheel_atoms::Object = Records::get(store, bolt).unwrap().unwrap();
        held.config.insert("life.open.close".into(), state.into());
        let base = held.seq;
        Records::put(store, bolt, &held, base).unwrap();
    };
    for offered in ["offered", "held"] {
        close(&mut store, offered);
        assert!(
            flywheel_domain::commands::rail(&mut store, &defs).unwrap().iter().any(|d| d.object == bolt) || offered == "held",
            "the close is not standing to be taken back"
        );
        let (capture, _) = a_note(&mut store, &mut world, &defs, &format!("one more thing while it is {offered}"));
        let call = Call::new("propose-unit", "chuck", "page").arg("capture", json!(capture)).arg("bolt", json!(bolt));
        catalogue::call(&mut store, &mut world, &defs, &call).expect("the capture goes to the bolt");

        let held: flywheel_atoms::Object = Records::get(&store, bolt).unwrap().unwrap();
        assert_eq!(
            held.config.get("life.open.close").map(String::as_str),
            Some("not-offered"),
            "the close offer stands after work arrived on the bolt (S224a)"
        );
        let rail = flywheel_domain::commands::rail(&mut store, &defs).unwrap();
        assert!(!rail.iter().any(|d| d.object == bolt && d.kind == "bolt-close"), "the close is still on the rail");
    }
}

/// `attach to…`: the signal's move attaches it to the open intent picked, which
/// cites it as material to propose from; an intent that is not open is refused
/// naming the open ones, and the same delivery twice is one call (19a, 21, 116,
/// 137).
#[test]
fn attach_signal_moves_it_onto_an_open_intent() {
    let (mut store, mut world, defs) = a_store();
    let (first, _) = a_note(&mut store, &mut world, &defs, "keep the row numbers on every page");
    let opened = Call::new("open-intent", "chuck", "page").arg("capture", json!(first));
    catalogue::call(&mut store, &mut world, &defs, &opened).expect("the intent opens");
    let intent = "intent/keep-row-numbers-every";
    let (_, signal) = a_note(&mut store, &mut world, &defs, "page two drops the numbers again");

    let nowhere = Call::new("attach-signal", "chuck", "page")
        .arg("signal", json!(signal))
        .arg("intent", json!("intent/nowhere"));
    let refused = catalogue::call(&mut store, &mut world, &defs, &nowhere).expect_err("no such intent is open");
    assert!(refused.to_string().contains(intent), "the refusal names the open intents: {refused}");
    assert!(signals::standing_move(&world, &signal).unwrap().is_none(), "a refused attach moved the signal");

    let call = Call::new("attach-signal", "chuck", "page")
        .arg("signal", json!(signal))
        .arg("intent", json!(intent))
        .delivered("tap-attach");
    let outcome = catalogue::call(&mut store, &mut world, &defs, &call).expect("the signal attaches");
    let moved = signals::standing_move(&world, &signal).unwrap().expect("the signal moved");
    assert_eq!(moved.target, format!("attach {intent}"));
    assert!(
        flywheel_domain::effects::pending_material(&store, intent).unwrap().contains(&signal),
        "the intent holds it as material to propose from (21)"
    );
    let response = store.get(&format!("response/{}", outcome.id)).unwrap().expect("the call is recorded");
    assert_eq!(response.record.get("tool"), Some(&json!("attach-signal")));
    assert_eq!(response.record.get("object"), Some(&json!(signal)));

    let again = catalogue::call(&mut store, &mut world, &defs, &call).expect("the same delivery is acknowledged");
    assert!(matches!(again.outcome, flywheel_atoms::Received::AlreadyApplied { .. }), "{:?}", again.outcome);
}

/// `drop`: the signal's move is drop with the call as its reason, it leaves what
/// curation reads, and `revive` clears the move so it is unmoved again (19a,
/// 107, S24).
#[test]
fn drop_signal_is_cleared_by_revive() {
    let (mut store, mut world, defs) = a_store();
    let (_, signal) = a_note(&mut store, &mut world, &defs, "a note for nobody");

    let dropped = Call::new("drop-signal", "chuck", "page").arg("signal", json!(signal));
    let outcome = catalogue::call(&mut store, &mut world, &defs, &dropped).expect("the signal drops");
    let moved = signals::standing_move(&world, &signal).unwrap().expect("the signal moved");
    assert_eq!(moved.target, "drop");
    assert!(moved.reason.contains(&outcome.id), "the call is its reason: {}", moved.reason);
    let unmoved = |world: &FakeWorld| -> Vec<String> {
        signals::unmoved(&signals::Blueprints(world)).into_iter().map(|s| s.id).collect()
    };
    assert!(!unmoved(&world).contains(&signal), "a dropped signal is still what curation reads");

    let revived = Call::new("revive", "chuck", "page").arg("signal", json!(signal));
    let outcome = catalogue::call(&mut store, &mut world, &defs, &revived).expect("the signal is revived");
    assert!(signals::standing_move(&world, &signal).unwrap().is_none(), "the move was not cleared");
    assert!(unmoved(&world).contains(&signal), "a revived signal is unmoved again (107)");
    assert!(store.get(&signal).unwrap().expect("the signal").record.get("move").is_none());
    let response = store.get(&format!("response/{}", outcome.id)).unwrap().expect("the call is recorded");
    assert_eq!(response.record.get("tool"), Some(&json!("revive")));

    // Unmoved again, it can be dropped again.
    catalogue::call(&mut store, &mut world, &defs, &dropped).expect("the signal drops again");
}

/// `build now` routes the capture's signal to the unit it made, and a capture
/// whose signal has moved makes no unit (34, 107, 116, S217).
#[test]
fn propose_unit_from_a_capture_routes_its_signal() {
    let (mut store, mut world, defs) = a_store();
    let (capture, signal) = a_note(&mut store, &mut world, &defs, "the rows lose their numbers on the second page");

    let call = Call::new("propose-unit", "chuck", "page")
        .arg("bolt", json!("bolt/atlas/plan-rows"))
        .arg("capture", json!(capture));
    catalogue::call(&mut store, &mut world, &defs, &call).expect("the unit is built");
    let unit = "unit/atlas/plan-rows";
    assert!(store.get(unit).unwrap().is_some(), "the unit stands");
    let moved = signals::standing_move(&world, &signal).unwrap().expect("the signal moved");
    assert_eq!(moved.target, format!("route {unit}"), "the route names the unit (116)");
    assert_eq!(store.get(&signal).unwrap().expect("the signal").record.get("route"), Some(&json!(unit)));

    let again = Call::new("propose-unit", "chuck", "page")
        .arg("bolt", json!("bolt/atlas/other-rows"))
        .arg("capture", json!(capture));
    assert!(catalogue::call(&mut store, &mut world, &defs, &again).is_err(), "a moved signal was built twice");
    assert!(store.get("unit/atlas/other-rows").unwrap().is_none(), "a refused build made a unit");
}
