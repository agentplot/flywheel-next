//! A capture with no signals, read by a capture-reader session: the order it is
//! handed, and what its delivery becomes (111, 113, 115; context.yaml
//! sessions.capture-reader).

use crate::report::{deliver, write_report, Report};
use crate::{changes, commands, offers, order, signals};
use chrono::{DateTime, TimeZone, Utc};
use flywheel_atoms::testing::{FakeStore, FakeWorld};
use flywheel_atoms::Records;
use flywheel_engine::Definitions;
use serde_json::json;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap()
}

const KEY: &str = "folder/drops/5f1d2c3b4a596877";

/// A transcript dropped in a folder, captured with no signals, beside a
/// standing specification a signal may argue with.
fn a_dropped_transcript(store: &mut FakeStore, world: &mut FakeWorld, defs: &Definitions) -> signals::Capture {
    let capture = signals::Capture {
        key: KEY.into(),
        source: "folder".into(),
        event_at: "2026-09-03".into(),
        captured_by: "chuck".into(),
        raw: "/drops/2026-09-03-viewpoint-sds-connector-design.txt".into(),
    };
    signals::write_capture(world, &capture).unwrap();
    let record = [
        ("source", json!(capture.source)),
        ("event_key", json!(capture.key)),
        ("event_at", json!(capture.event_at)),
        ("captured_by", json!(capture.captured_by)),
        ("raw", json!(capture.raw)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    commands::put_new(store, defs, &signals::object_of(KEY), "capture", None, record, now()).unwrap();
    world.files.insert(
        "openspec/specs/connectors/spec.md".into(),
        "## Purpose\n\n### Requirement: A connector names its stream\n\nA connector SHALL…\n".into(),
    );
    capture
}

/// The reader's order carries the capture's provenance and its pointer, the
/// standing claims by name, and the one file it delivers its signals in with
/// the fields a signal takes (111, 113; context.yaml sessions.capture-reader).
#[test]
fn a_readers_order_carries_the_capture_the_claims_and_the_signal_format() {
    let (mut store, mut world) = (FakeStore::default(), FakeWorld::new());
    let defs = crate::set::load().unwrap();
    let capture = a_dropped_transcript(&mut store, &mut world, &defs);
    let inputs = order::Reading {
        capture,
        claims: changes::standing_claims(&signals::Blueprints(&world)),
        tags: None,
    };
    let rendered = order::capture_reading(&inputs);
    for shown in [
        "- source: folder",
        "- event: folder/drops/5f1d2c3b4a596877",
        "- said at: 2026-09-03",
        "- the material: /drops/2026-09-03-viewpoint-sds-connector-design.txt",
        "- `connectors/a-connector-names-its-stream` · A connector names its stream · openspec/specs/connectors/spec.md",
        "The instance keeps no vocabulary yet",
        ".flywheel/deliverables/signal.rec",
        "Kind: constraint | ask | question | commitment | reaction",
        "Position: <where the excerpt sits",
        "--deliverable signal",
    ] {
        assert!(rendered.contains(shown), "the order lacks `{shown}`: {rendered}");
    }
}

/// A reader's delivery is parsed when its offers are recorded: each signal the
/// schema admits is its capture's next signal, excerpt verbatim, and one with no
/// position in its material is refused on the thread; the capture then holds
/// its signals, and a second pass writes nothing (113, 115, 80, 127).
#[test]
fn a_readers_delivery_writes_the_captures_signals() {
    let (mut store, mut world) = (FakeStore::default(), FakeWorld::new());
    let defs = crate::set::load().unwrap();
    a_dropped_transcript(&mut store, &mut world, &defs);
    let capture = signals::object_of(KEY);
    let session = format!("{capture}/reading/1");
    let delivered = "Kind: constraint\nSaid-by: Dana\nSubjects: connectors streams\n\
         Assertion: A connector takes one stream and never two.\n\
         Excerpt: \"one stream per connector,\n+ or we lose ordering\"\nPosition: lines 40-41\n\
         Argues-with: connectors/a-connector-names-its-stream\n\n\
         Kind: question\nSaid-by: Sam\nSubjects: naming\nAssertion: Sam asked who names a connector.\n\
         Excerpt: \"who names these?\"\nPosition: 0:12:04\nArgues-with:\n\n\
         Kind: ask\nSaid-by: Sam\nAssertion: Someone should write the connector rules down.\n\
         Excerpt: \"write it down\"\nPosition:\n";
    deliver(&mut store, &session, "capture-reader", now(), "signal", delivered).unwrap();
    let exit = Report::Exit {
        kind: "done".into(),
        deliverables: vec!["signal".into()],
        question: None,
        text: None,
    };
    write_report(&mut store, &session, "capture-reader", now(), &exit).unwrap();

    let made = offers::record(&mut store, &mut world, &defs, &session, &capture, now()).unwrap();
    let refused: Vec<String> = store
        .thread(&session)
        .unwrap()
        .iter()
        .filter(|e| e.kind == "refusal")
        .map(|e| e.fields.get("reason").and_then(|v| v.as_str()).unwrap_or_default().to_string())
        .collect();
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(refused[0].contains("no position"), "{refused:?}");
    assert_eq!(made, vec![signals::signal_object(KEY, 1), signals::signal_object(KEY, 2)]);

    let read = signals::signals_of(&world, KEY).unwrap();
    assert_eq!(read.len(), 2, "{read:?}");
    assert_eq!((read[0].kind.as_str(), read[0].asserted_by.as_str()), ("constraint", "Dana"));
    assert_eq!(read[0].assertion, "A connector takes one stream and never two.");
    assert_eq!(read[0].excerpt, "\"one stream per connector,\nor we lose ordering\"");
    assert_eq!(read[0].position, "lines 40-41");
    assert_eq!(read[0].subject_tags, vec!["connectors", "streams"]);
    assert_eq!(read[0].argues_with, vec!["connectors/a-connector-names-its-stream"]);
    assert_eq!((read[1].kind.as_str(), read[1].position.as_str()), ("question", "0:12:04"));
    assert!(read[1].argues_with.is_empty());
    assert_eq!(
        signals::evidence(&signals::Blueprints(&world), &capture, "capture.signals_present"),
        Some(json!(true))
    );
    let held = store.get(&signals::signal_object(KEY, 1)).unwrap().expect("the signal's object");
    assert_eq!(held.parent.as_deref(), Some(capture.as_str()));

    let writes = store.writes();
    offers::record(&mut store, &mut world, &defs, &session, &capture, now()).unwrap();
    assert_eq!(store.writes(), writes, "a second pass wrote again");
    assert_eq!(signals::signals_of(&world, KEY).unwrap().len(), 2);
}
