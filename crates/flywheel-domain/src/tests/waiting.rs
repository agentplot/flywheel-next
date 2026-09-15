//! What waits for curation, as the signals tray and the status page list it:
//! grouped by capture, ordered by source and age, none discarded (118, S225).

use crate::signals::{self, Capture, Move, Signal};
use std::collections::BTreeMap;

fn a_capture(files: &mut BTreeMap<String, String>, key: &str, source: &str, event_at: &str, signals_held: usize) {
    let capture = Capture {
        key: key.into(),
        source: source.into(),
        event_at: event_at.into(),
        captured_by: "dana".into(),
        raw: format!("raw://{key}"),
    };
    let mut world = flywheel_atoms::testing::FakeWorld::new();
    signals::write_capture(&mut world, &capture).unwrap();
    for ordinal in 1..=signals_held as u64 {
        let signal = Signal {
            id: signals::signal_object(key, ordinal),
            capture: signals::object_of(key),
            kind: "question".into(),
            asserted_by: "dana".into(),
            assertion: format!("{key} says {ordinal}"),
            position: "whole".into(),
            ..Default::default()
        };
        signals::write_signal(&mut world, key, ordinal, &signal).unwrap();
    }
    files.extend(world.files);
}

/// Grouped by capture and ordered by source, then age, the oldest first; a
/// capture's tenth signal follows its ninth; a moved signal leaves the list and
/// a capture with nothing left leaves it too (118, 107).
#[test]
fn waiting_is_grouped_by_capture_and_ordered_by_source_and_age() {
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    a_capture(&mut files, "meeting/2026-09-02/weekly", "meeting", "2026-09-02", 10);
    a_capture(&mut files, "meeting/2026-08-30/standup", "meeting", "2026-08-30", 1);
    a_capture(&mut files, "folder/drop/abc", "folder", "2026-09-10", 1);

    let listed = signals::waiting(&files);
    let order: Vec<(&str, &str)> = listed.iter().map(|w| (w.source.as_str(), w.event_at.as_str())).collect();
    assert_eq!(order, vec![("folder", "2026-09-10"), ("meeting", "2026-08-30"), ("meeting", "2026-09-02")]);
    let weekly = &listed[2];
    assert_eq!(weekly.signals.len(), 10);
    assert_eq!(weekly.signals[8].assertion, "meeting/2026-09-02/weekly says 9");
    assert_eq!(weekly.signals[9].assertion, "meeting/2026-09-02/weekly says 10", "the tenth follows the ninth");

    let mut world = flywheel_atoms::testing::FakeWorld::new();
    world.files = files;
    let standup = signals::signal_object("meeting/2026-08-30/standup", 1);
    let moved = Move { signal: standup, target: "drop".into(), reason: "noise".into(), at: "2026-09-15".into() };
    signals::write_move(&mut world, &moved).unwrap();
    let listed = signals::waiting(&world.files);
    assert_eq!(listed.len(), 2, "a capture with nothing left unmoved still waits: {listed:?}");
    assert!(listed.iter().all(|w| w.capture != signals::object_of("meeting/2026-08-30/standup")));
}
