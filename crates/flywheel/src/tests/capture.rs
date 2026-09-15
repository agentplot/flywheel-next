//! The signals folder, read as captures already read: `flywheel capture signals
//! <dir>` over the layout a blueprints' `signals/` folder is written in (114,
//! 111, 215, D13); and the folder drop, `flywheel capture folder <dir>`. Over
//! the fake store and world, since what is proven is the reading of the folder
//! and not a repository (D17).

use chrono::{TimeZone, Utc};
use flywheel_atoms::testing::{FakeStore, FakeWorld};
use flywheel_atoms::Records;
use flywheel_domain::{adapters, signals};
use std::path::{Path, PathBuf};

const SIGNALS: [&str; 3] = [
    "2026-08-31-harbor-dashboard-review/01-berths-and-cranes-share-one-card",
    "2026-08-31-harbor-dashboard-review/02-tide-table-wants-a-week",
    "2026-09-04-yard-standup/01-gate-scanner-drops-at-shift-change",
];

/// A copy of the fixture folder, named `signals` as the blueprints name it, with
/// the `moves.rec` given beside it.
fn a_folder(test: &str, moves: Option<&str>) -> PathBuf {
    let base = std::env::temp_dir().join(format!("flywheel-capture-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let folder = base.join("signals");
    copy(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/signals"), &folder);
    if let Some(moves) = moves {
        std::fs::write(folder.join("moves.rec"), moves).unwrap();
    }
    folder
}

fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let into = to.join(path.file_name().unwrap());
        match path.is_dir() {
            true => copy(&path, &into),
            false => {
                std::fs::copy(&path, &into).unwrap();
            }
        }
    }
}

fn import(store: &mut FakeStore, world: &mut FakeWorld, folder: &Path) -> adapters::Enumerated {
    let defs = flywheel_domain::set::load().unwrap();
    let at = Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap();
    adapters::run(store, world, &defs, &format!("signals {}", folder.display()), "chuck", at).unwrap()
}

/// Each capture directory is a capture keeping its source, event date and raw
/// pointer, and each signal file a signal keeping its kind, who said it, its
/// subjects, the claims it argues with, its assertion, its excerpt verbatim and
/// where the excerpt sits; the capture machine reads them present, so no reader
/// is charged; and importing the folder again writes nothing (114, 111, 113,
/// 217e).
#[test]
fn capture_signals_reads_willdans_layout() {
    let folder = a_folder("reads", None);
    let (mut store, mut world) = (FakeStore::default(), FakeWorld::new());
    let first = import(&mut store, &mut world, &folder);
    assert_eq!(
        first.keys,
        vec![
            "signals/signals/2026-08-31-harbor-dashboard-review".to_string(),
            "signals/signals/2026-09-04-yard-standup".to_string(),
        ],
        "one capture per directory, the README and the loose files aside"
    );
    assert_eq!((first.captures_written, first.signals_written, first.moves_written), (2, 3, 0));

    let review = signals::read_capture(&world, &first.keys[0]).unwrap().expect("the review's capture");
    assert_eq!(review.source, "wispr-flow");
    assert_eq!(review.event_at, "2026-08-31");
    assert_eq!(review.raw, "signals/.raw/2026-08-31-harbor-dashboard-review.txt", "the pointer, never the material");
    let standup = signals::read_capture(&world, &first.keys[1]).unwrap().expect("the standup's capture");
    assert_eq!((standup.source.as_str(), standup.event_at.as_str()), ("teams", "2026-09-04"));

    let read = signals::signals_of(&world, &first.keys[0]).unwrap();
    assert_eq!(read.len(), 2);
    let berths = &read[0];
    assert_eq!(berths.kind, "question");
    assert_eq!(berths.asserted_by, "Rosa Lind");
    assert_eq!(berths.subject_tags, vec!["dashboard", "berths", "cranes"]);
    assert_eq!(berths.argues_with, vec!["books/harbor-kit/src/dashboard.md", "hk.catalog"]);
    assert_eq!(
        berths.assertion,
        "The dashboard puts a berth and the crane serving it on one card, and Rosa could not tell which of the two a status light belonged to."
    );
    assert_eq!(
        berths.excerpt,
        "\"that red light, is that the berth or the crane? because those are different people I call\"\n\n\"maybe they are two cards.\""
    );
    assert_eq!(berths.position, "lines 40-41, line 52");
    assert_eq!(read[1].kind, "ask");
    assert!(read[1].argues_with.is_empty());
    let scanner = &signals::signals_of(&world, &first.keys[1]).unwrap()[0];
    assert_eq!((scanner.kind.as_str(), scanner.position.as_str()), ("constraint", "0:04:12"));

    // The objects the engine ticks, beside the records a person reads; the
    // capture machine reads its signals present and is read at once, so no
    // reader is charged (114, `capture.yaml` reading.captured).
    let capture = signals::object_of(&first.keys[0]);
    let held = store.get(&capture).unwrap().expect("the capture's object");
    assert_eq!(held.record.get("source").and_then(|v| v.as_str()), Some("wispr-flow"));
    assert_eq!(signals::of_capture(&store, &capture).unwrap().len(), 2);
    assert_eq!(
        signals::evidence(&signals::Blueprints(&world), &capture, "capture.signals_present"),
        Some(serde_json::json!(true))
    );
    assert!(
        store.list_records(&flywheel_atoms::Scope::All).unwrap().iter().all(|o| !o.id.contains("session")),
        "a session was charged"
    );

    let writes = store.writes();
    let again = import(&mut store, &mut world, &folder);
    assert_eq!((again.captures_written, again.signals_written, again.moves_written), (0, 0, 0));
    assert_eq!(store.writes(), writes, "a second import wrote to the store");
    let _ = std::fs::remove_dir_all(folder.parent().unwrap());
}

/// A move in the folder's `moves.rec` by one of the six shipped words is the
/// signal's move; a move by any other word, `new-territory` among them, leaves
/// its signal unmoved for curation (107, 114, 118).
#[test]
fn a_move_by_an_unshipped_word_leaves_the_signal_unmoved() {
    let moves = format!(
        "# Curation moves — append-only.\n\n%rec: Move\n%type: Move enum attach challenge new-territory answered drop\n\n\
         Signal: {}\nMove: drop\nReason: said again in the standup\nDate: 2026-09-05\nBy: rosa\n\n\
         Signal: {}\nMove: new-territory\nTarget: harbor tides\nReason: nothing covers tides yet\nDate: 2026-09-05\nBy: rosa\n",
        SIGNALS[0], SIGNALS[1]
    );
    let folder = a_folder("moves", Some(&moves));
    let (mut store, mut world) = (FakeStore::default(), FakeWorld::new());
    let first = import(&mut store, &mut world, &folder);
    assert_eq!(first.moves_written, 1, "only the drop is a shipped move");

    let key = &first.keys[0];
    let (berths, tides) = (signals::signal_object(key, 1), signals::signal_object(key, 2));
    let dropped = signals::standing_move(&world, &berths).unwrap().expect("the drop stands");
    assert_eq!((dropped.target.as_str(), dropped.reason.as_str()), ("drop", "said again in the standup"));
    assert!(signals::standing_move(&world, &tides).unwrap().is_none(), "new-territory moved a signal");
    let unmoved: Vec<String> = signals::unmoved(&signals::Blueprints(&world)).into_iter().map(|s| s.id).collect();
    assert!(unmoved.contains(&tides) && !unmoved.contains(&berths), "{unmoved:?}");

    let again = import(&mut store, &mut world, &folder);
    assert_eq!(again.moves_written, 0, "a move already standing is carried once");
    let _ = std::fs::remove_dir_all(folder.parent().unwrap());
}

/// A file dropped in a folder is one capture with a pointer to the file and no
/// signals, keyed by the folder's name and what the file holds; its source is
/// due until it is captured, and a second enumeration writes nothing (111, 115,
/// 215).
#[test]
fn capture_folder_writes_a_capture_with_no_signals() {
    let base = std::env::temp_dir().join(format!("flywheel-capture-folder-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let folder = base.join("drops");
    std::fs::create_dir_all(folder.join("read")).unwrap();
    let transcript = folder.join("2026-09-02-willdan-weekly.vtt");
    std::fs::write(&transcript, "WEBVTT\n\n00:00:04.000 --> 00:00:09.000\nthe gate scanner drops at shift change\n").unwrap();
    std::fs::write(folder.join(".DS_Store"), "not a drop").unwrap();
    let (mut store, mut world) = (FakeStore::default(), FakeWorld::new());
    let defs = flywheel_domain::set::load().unwrap();
    let at = Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap();
    let command = format!("folder {}", folder.display());
    assert!(adapters::folder_due(&world, &folder.display().to_string()), "the drop is not due");

    let first = adapters::run(&mut store, &mut world, &defs, &command, "chuck", at).unwrap();
    let hash = format!("{:016x}", flywheel_atoms::atoms::fnv1a(&std::fs::read(&transcript).unwrap()));
    assert_eq!(first.keys, vec![format!("folder/drops/{hash}")], "one capture, the hidden file and the directory aside");
    assert_eq!((first.captures_written, first.signals_written, first.moves_written), (1, 0, 0));

    let capture = signals::read_capture(&world, &first.keys[0]).unwrap().expect("the capture");
    assert_eq!((capture.source.as_str(), capture.event_at.as_str()), ("folder", "2026-09-02"));
    assert_eq!(capture.captured_by, "chuck");
    assert_eq!(capture.raw, transcript.display().to_string(), "the pointer, never the material");
    assert!(signals::signals_of(&world, &first.keys[0]).unwrap().is_empty(), "the adapter read signals");
    let object = signals::object_of(&first.keys[0]);
    let held = store.get(&object).unwrap().expect("the capture's object");
    assert_eq!(held.record.get("source").and_then(|v| v.as_str()), Some("folder"));
    assert_eq!(
        signals::evidence(&signals::Blueprints(&world), &object, "capture.signals_present"),
        Some(serde_json::json!(false))
    );
    assert!(
        store.list_records(&flywheel_atoms::Scope::All).unwrap().iter().all(|o| !o.id.contains("session")),
        "the adapter charged a session"
    );
    assert!(!adapters::folder_due(&world, &folder.display().to_string()), "a captured drop is still due");

    let writes = store.writes();
    let again = adapters::run(&mut store, &mut world, &defs, &command, "chuck", at).unwrap();
    assert_eq!((again.captures_written, again.signals_written), (0, 0));
    assert_eq!(store.writes(), writes, "a second enumeration wrote to the store");
    let _ = std::fs::remove_dir_all(&base);
}
