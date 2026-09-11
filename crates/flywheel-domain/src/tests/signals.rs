//! Captures, signals and moves: the material under the machinery's prefix in
//! the blueprints, keyed by source event and never rewritten
//! (`signals/capture-and-curation`, 111, 113, 107, 203).

use chrono::{DateTime, Duration, TimeZone, Utc};
use flywheel_atoms::testing::{FakeStore, FakeWorld};
use flywheel_atoms::{Records, World};
use crate::commands;
use crate::signals::{self, Capture};
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
        crate::set::load().unwrap(),
    )
}

/// A source-event key cannot be one of the two names the layout keeps for
/// itself, so a capture can never be mistaken for a move (111, 203).
#[test]
fn a_key_never_shadows_the_layout() {
    let (_store, mut world, _defs) = a_store();
    for key in ["moves/s1", "captures/x"] {
        let refused = signals::write_capture(
            &mut world,
            &Capture {
                key: key.into(),
                ..Default::default()
            },
        )
        .expect_err("a key shadowing the layout");
        assert!(refused.to_string().contains("keeps for itself"), "{refused}");
    }
}
/// The signal and move formats are versioned and stable: a record written
/// under an earlier version is read as it stands, with no migration step, so
/// captures made before the flywheel existed read without conversion (114).
#[test]
fn older_signal_reads_unconverted() {
    let mut world = FakeWorld::new();

    // The formats say which version wrote them (114).
    assert_eq!(signals::CAPTURE_FORMAT, "flywheel-capture/1");
    assert_eq!(signals::SIGNAL_FORMAT, "flywheel-signal/1");
    assert_eq!(signals::MOVE_FORMAT, "flywheel-move/1");
    for format in [
        signals::CAPTURE_FORMAT,
        signals::SIGNAL_FORMAT,
        signals::MOVE_FORMAT,
    ] {
        let (name, version) = format.split_once('/').expect("a versioned format name");
        assert!(!name.is_empty() && version.parse::<u32>().is_ok(), "{format}");
    }

    // A record an earlier version wrote, put where this one would look for it.
    let older = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/older-signal.rec"),
    )
    .expect("the fixture");
    assert!(
        !older.contains("format:"),
        "the fixture is not in the earlier format"
    );
    let key = "inbox/2024-11-03";
    world
        .write_file(
            signals::BLUEPRINTS,
            &signals::signal_path(key, 1),
            older.as_bytes(),
            None,
        )
        .expect("the older record is placed");

    // It reads, as it stands: the fields it has are read and the ones it never
    // carried are simply absent. Nothing migrated it (114).
    let read = signals::signals_of(&world, key).expect("the older record reads");
    assert_eq!(read.len(), 1, "{read:?}");
    let signal = &read[0];
    assert_eq!(signal.id, "signal/inbox-2024-11-03-1");
    assert_eq!(signal.capture, "capture/inbox-2024-11-03");
    assert_eq!(signal.kind, "constraint");
    assert_eq!(signal.asserted_by, "dana");
    assert_eq!(
        signal.assertion,
        "only one host may write to a provider at a time"
    );
    assert_eq!(
        signal.excerpt,
        "both wrote to the same provider inside a minute"
    );
    // What the earlier version did not write is absent, not invented.
    assert_eq!(signal.position, "");
    assert!(signal.argues_with.is_empty());
    assert!(signal.subject_tags.is_empty());

    // And reading it wrote nothing back: no migration step touched the file
    // (114, 78).
    assert_eq!(
        world.writes_to(&signals::signal_path(key, 1)),
        1,
        "reading the older record rewrote it"
    );
    assert_eq!(
        String::from_utf8(
            world
                .read_file(signals::BLUEPRINTS, &signals::signal_path(key, 1))
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        older,
        "the older record was converted on read"
    );

    // A move written under an earlier version reads the same way.
    let older_move = "%rec: move\n\nsignal: signal/inbox-2024-11-03-1\ntarget: drop\nreason: superseded\n";
    world
        .write_file(
            signals::BLUEPRINTS,
            &signals::move_path("signal/inbox-2024-11-03-1"),
            older_move.as_bytes(),
            None,
        )
        .expect("the older move is placed");
    let moved = signals::standing_move(&world, "signal/inbox-2024-11-03-1")
        .expect("a read")
        .expect("the move");
    assert_eq!(moved.word(), "drop");
    assert_eq!(moved.reason, "superseded");
    assert_eq!(moved.at, "", "a date the earlier version never wrote");
}
/// Every signal has exactly one standing move — the signal id, the target, the
/// reason and the date — and curation sees only the signals that have none
/// (107).
#[test]
fn one_standing_move_per_signal() {
    let mut world = FakeWorld::new();
    let key = "meeting/2026-09-02/willdan-weekly";

    // Three signals from one transcript, none of them moved.
    for (n, assertion) in [
        (1u64, "only one host may write to a provider at a time"),
        (2, "the retry jitter is not in the spec"),
        (3, "the status copy says pending when it means queued"),
    ] {
        signals::write_signal(
            &mut world,
            key,
            n,
            &signals::Signal {
                id: signals::signal_object(key, n),
                capture: signals::object_of(key),
                kind: "constraint".into(),
                asserted_by: "dana".into(),
                assertion: assertion.into(),
                excerpt: assertion.into(),
                ..Default::default()
            },
        )
        .unwrap();
    }
    assert_eq!(signals::unmoved(&signals::Blueprints(&world)).len(), 3);

    // One move, with everything 107 asks of it.
    let first = signals::signal_object(key, 1);
    let attached = signals::Move {
        signal: first.clone(),
        target: "attach intent/atlas-provider-limits".into(),
        reason: "it is evidence on the open intent".into(),
        at: at(0).to_rfc3339(),
    };
    assert!(signals::write_move(&mut world, &attached).unwrap());
    let standing = signals::standing_move(&world, &first)
        .unwrap()
        .expect("the move");
    assert_eq!(standing, attached);
    assert_eq!(standing.word(), "attach");
    assert_eq!(standing.names(), "intent/atlas-provider-limits");

    // Exactly one: a second move on the same signal replaces the first rather
    // than standing beside it (107).
    let challenged = signals::Move {
        signal: first.clone(),
        target: "challenge providers/one-writer@3".into(),
        reason: "the operator said so".into(),
        at: at(5).to_rfc3339(),
    };
    assert!(signals::write_move(&mut world, &challenged).unwrap());
    assert_eq!(
        signals::standing_move(&world, &first).unwrap().unwrap(),
        challenged,
        "the replacement did not stand"
    );
    assert_eq!(
        signals::moves(&signals::Blueprints(&world)).len(),
        1,
        "a signal carries two moves"
    );

    // Curation sees only the unmoved ones, and never re-judges a moved one
    // (107).
    let files = signals::Blueprints(&world);
    let unmoved: Vec<String> = signals::unmoved(&files)
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(
        unmoved,
        vec![signals::signal_object(key, 2), signals::signal_object(key, 3)],
        "curation would re-judge a signal that has a move"
    );
    assert_eq!(
        signals::evidence(&files, &first, "signal.move"),
        Some(json!("challenge"))
    );
    assert_eq!(
        signals::evidence(&files, "curation/willdan", "curation.unmoved_count"),
        Some(json!(2))
    );

    // The operator's response is what replaces a move, and clearing one makes
    // the signal unmoved again so the next run clusters it (107, S24).
    signals::clear_move(&mut world, &first).unwrap();
    assert_eq!(signals::standing_move(&world, &first).unwrap(), None);
    let files = signals::Blueprints(&world);
    assert_eq!(signals::unmoved(&files).len(), 3);
    assert_eq!(
        signals::evidence(&files, &first, "signal.move"),
        Some(json!("none"))
    );

    // A move outside the six is no move at all (107).
    let refused = signals::write_move(
        &mut world,
        &signals::Move {
            signal: first,
            target: "done".into(),
            ..Default::default()
        },
    )
    .expect_err("a move outside the six");
    assert!(refused.to_string().contains("is no move"), "{refused}");
}
/// Every move has a stated consequence, and a challenge's is to record the
/// claim it argues with by name and version — and nothing of the ledger, which
/// belongs to the phase that has one (116, 101).
#[test]
fn challenge_records_claim_only() {
    let mut world = FakeWorld::new();
    let (mut store, _files, defs) = a_store();
    let key = "meeting/2026-09-02/willdan-weekly";
    let signal = signals::signal_object(key, 1);
    let at0 = at(0);

    // A signal, an open intent for it to attach to, and a proposed one to join.
    commands::put_new(&mut store, &defs, &signal, "signal", None, Default::default(), at0).unwrap();
    for intent in ["intent/atlas-provider-limits", "intent/retry-jitter"] {
        commands::put_new(&mut store, &defs, intent, "intent", None, Default::default(), at0)
            .unwrap();
    }

    // The challenge: the claim by name and version, on the signal (116).
    let applied = signals::apply_move(
        &mut store,
        &mut world,
        &signals::Move {
            signal: signal.clone(),
            target: "challenge providers/one-writer@3".into(),
            reason: "switchboard has its own client and nothing says only one writes".into(),
            at: at0.to_rfc3339(),
        },
        at0,
    )
    .unwrap();
    assert_eq!(applied.claim.as_deref(), Some("providers/one-writer"));
    assert_eq!(applied.claim_version, Some(3));
    let held = Records::get(&store, &signal).unwrap().unwrap();
    assert_eq!(
        held.record.get("argues_with").unwrap(),
        &json!(["providers/one-writer@3"]),
        "the signal does not record the claim it argues with"
    );

    // And nothing of the ledger was attempted: no cell was written, no ledger
    // file exists, and nothing was read from one (101, A.14 is phase 3).
    let cells: Vec<_> = Records::list_records(&store, &flywheel_atoms::Scope::All)
        .unwrap()
        .into_iter()
        .filter(|o| o.machine == "ledger-cell" || o.id.starts_with("cell/"))
        .collect();
    assert!(cells.is_empty(), "a challenge wrote a ledger cell: {cells:?}");
    assert!(
        world.list_files(signals::BLUEPRINTS, "flywheel/ledger/").unwrap().is_empty(),
        "a challenge wrote into the ledger"
    );

    // The other five have their own consequences (116).
    let cases = [
        ("attach intent/atlas-provider-limits", Some("intent/atlas-provider-limits")),
        ("join intent/retry-jitter", Some("intent/retry-jitter")),
        ("answered providers/one-writer@4", Some("providers/one-writer@4")),
        ("route offer/chore-1", Some("offer/chore-1")),
        ("drop", None),
    ];
    for (target, expected) in cases {
        let applied = signals::apply_move(
            &mut store,
            &mut world,
            &signals::Move {
                signal: signal.clone(),
                target: target.into(),
                reason: "curation said so".into(),
                at: at0.to_rfc3339(),
            },
            at0,
        )
        .unwrap();
        assert_eq!(applied.target.as_deref(), expected, "`{target}`");
        // One standing move, whatever came before it (107).
        assert_eq!(
            signals::standing_move(&world, &signal).unwrap().unwrap().target,
            target
        );
    }

    // Attach and join put the signal on the intent, which says how many it has
    // (109, 116).
    for intent in ["intent/atlas-provider-limits", "intent/retry-jitter"] {
        let held = Records::get(&store, intent).unwrap().unwrap();
        assert_eq!(held.record.get("signals").unwrap(), &json!([signal.clone()]));
        assert_eq!(held.record.get("signals_count").unwrap(), &json!(1));
    }

    // Still no ledger, after every move (101).
    assert!(world
        .list_files(signals::BLUEPRINTS, "flywheel/ledger/")
        .unwrap()
        .is_empty());
}
