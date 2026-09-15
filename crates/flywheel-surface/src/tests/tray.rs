//! The signals tray: what waits for curation, by capture, with run curation
//! now at its head (118, 19a, 110, S225, S231).

use crate::catalogue::{self, Call};
use crate::testing as world;
use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::Records;
use flywheel_domain::commands;
use flywheel_domain::signals::{self, Capture, Signal};
use flywheel_engine::Definitions;
use serde_json::json;

const ADDRESS: &str = "http://studio.tailnet.ts.net/willdan";

/// The page, with the tray's dock page as the drawer fetches it when the counter
/// opens it (310a, S235).
fn rendered(store: &mut FakeStore, world: &world::Files, defs: &Definitions) -> String {
    let read = crate::page::read(store, world, defs, ADDRESS, "chuck").expect("the page reads");
    crate::page::render(&read) + &crate::page::dock_page(&read, crate::page::tray::ID).expect("the tray's page")
}

/// The tray's surface in a rendered page.
fn tray_of(html: &str) -> String {
    let at = html.find("id=\"dock-tray\"").unwrap_or_else(|| panic!("no tray on the page: {html}"));
    html[at..].split("</article>").next().expect("the tray").to_string()
}

fn a_curation(store: &mut FakeStore, defs: &Definitions) {
    let at = commands::now(store).expect("a point");
    let record = [("threshold".to_string(), json!(12)), ("cadence".to_string(), json!("0 6 * * 1-5"))];
    commands::put_new(store, defs, "curation/willdan", "curation", None, record.into_iter().collect(), at)
        .expect("the curation");
}

/// A capture of material read before, with its signals, as the blueprints hold
/// it.
fn read_before(world: &mut world::Files, key: &str, source: &str, event_at: &str, said: &[&str]) {
    let capture = Capture {
        key: key.into(),
        source: source.into(),
        event_at: event_at.into(),
        captured_by: "dana".into(),
        raw: format!("raw://{key}"),
    };
    signals::write_capture(world, &capture).expect("the capture");
    for (at, words) in said.iter().enumerate() {
        let ordinal = at as u64 + 1;
        let signal = Signal {
            id: signals::signal_object(key, ordinal),
            capture: signals::object_of(key),
            kind: "question".into(),
            asserted_by: "dana".into(),
            assertion: words.to_string(),
            position: "whole".into(),
            ..Default::default()
        };
        signals::write_signal(world, key, ordinal, &signal).expect("the signal");
    }
}

/// Every signal curation has not read, grouped by capture and ordered by source
/// and age, each row carrying its controls; run curation now at the head beside
/// what would run it on its own; the count falls as a move is written, the head
/// shows curation reading while it runs, and with nothing waiting the tray says
/// what to do (118, 19a, 110, S214, S225).
#[test]
fn the_tray_groups_unmoved_signals_by_capture_source_and_age() {
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let mut store = FakeStore::default();
    let mut world = world::Files::new().tracking("atlas");
    a_curation(&mut store, &defs);
    read_before(&mut world, "meeting/2026-09-02/weekly", "meeting", "2026-09-02", &["the rows lose their numbers", "who owns the export?"]);
    read_before(&mut world, "meeting/2026-08-30/standup", "meeting", "2026-08-30", &["page two drops them again"]);
    read_before(&mut world, "folder/drop/abc", "folder", "2026-09-10", &["a transcript with nothing in it"]);
    let note = Call::new("capture", "chuck", "page").arg("text", json!("keep the numbers")).arg("source", json!("console"));
    catalogue::call(&mut store, &mut world, &defs, &note).expect("the note");

    let html = rendered(&mut store, &world, &defs);
    assert!(html.contains("href=\"#dock-tray\" data-waiting=\"5\""), "the counter opens the tray with five waiting");
    let tray = tray_of(&html);
    let at = |needle: &str| tray.find(needle).unwrap_or_else(|| panic!("the tray lacks `{needle}`: {tray}"));
    let groups = [
        at("data-source=\"console\""),
        at("data-capture=\"capture/folder-drop-abc\""),
        at("data-capture=\"capture/meeting-2026-08-30-standup\""),
        at("data-capture=\"capture/meeting-2026-09-02-weekly\""),
    ];
    assert!(groups.windows(2).all(|pair| pair[0] < pair[1]), "by source, then the oldest first: {groups:?}");
    let weekly = signals::signal_object("meeting/2026-09-02/weekly", 1);
    assert!(at(&format!("data-hand=\"{weekly}\"")) > groups[3], "a row carries its controls under its capture");
    assert!(tray.contains("<q>who owns the export?</q>"), "a row quotes its signal");
    assert!(tray.contains("action=\"/api/tools/curate\""), "run curation now is at the head");
    assert!(tray.contains(">run curation now<span class=\"k\">r</span>"));
    assert!(tray.contains("runs on its own at 12, or weekdays at 06:00"), "{tray}");
    assert!(commands::rail(&mut store, &defs).expect("the rail").is_empty(), "the tray asks nothing itself");

    // A move written takes its row away and the count down.
    let dropped = Call::new("drop-signal", "chuck", "page").arg("signal", json!(weekly));
    catalogue::call(&mut store, &mut world, &defs, &dropped).expect("dropped");
    let html = rendered(&mut store, &world, &defs);
    assert!(html.contains("data-waiting=\"4\""), "the count falls as a move is written");
    assert!(!tray_of(&html).contains(&format!("data-signal=\"{weekly}\"")));

    // While curation reads: the bar and its chip, and no second run now.
    let mut running = Records::get(&store, "curation/willdan").expect("a read").expect("the curation");
    running.config.insert("run".into(), "running".into());
    let base = running.seq;
    Records::put(&mut store, "curation/willdan", &running, base).expect("running");
    let tray = tray_of(&rendered(&mut store, &world, &defs));
    assert!(tray.contains("class=\"tray-bar\""), "no bar while curation reads");
    assert!(tray.contains("curation is reading them now"), "{tray}");
    assert!(!tray.contains("action=\"/api/tools/curate\""), "run now offered while curation runs");

    // Nothing waiting says what to do next.
    let (mut store, world) = (FakeStore::default(), world::Files::new());
    a_curation(&mut store, &defs);
    let html = rendered(&mut store, &world, &defs);
    assert!(html.contains("nothing waiting for curation"));
    assert!(tray_of(&html).contains("A note you type in the box above waits here"));
}

/// A capture of several signals — a folder imported, a transcript read — waits
/// in the tray: the board draws notes, and Recently done lists the notes taken,
/// so an import changes nothing on the page but the counter and the tray
/// (S13, S225, S9).
#[test]
fn a_capture_of_several_signals_waits_in_the_tray_not_on_the_board() {
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let mut store = FakeStore::default();
    let mut world = world::Files::new();
    let key = "signals/signals/2026-09-02-weekly";
    read_before(&mut world, key, "wispr-flow", "2026-09-02", &["the rows lose their numbers", "who owns the export?"]);
    let at = commands::now(&store).expect("a point");
    let capture = signals::object_of(key);
    let fields = [("source".to_string(), json!("wispr-flow")), ("event_key".to_string(), json!(key))];
    commands::put_new(&mut store, &defs, &capture, "capture", None, fields.into_iter().collect(), at).expect("its object");
    for ordinal in [1, 2] {
        let id = signals::signal_object(key, ordinal);
        commands::put_new(&mut store, &defs, &id, "signal", Some(&capture), Default::default(), at).expect("a signal's object");
    }
    let note = Call::new("capture", "chuck", "page").arg("text", json!("keep the numbers")).arg("source", json!("console"));
    let outcome = catalogue::call(&mut store, &mut world, &defs, &note).expect("the note");
    let noted = outcome.journal.iter().find(|n| n.kind == "capture").expect("a capture").object.clone();

    let html = rendered(&mut store, &world, &defs);
    let board = html.split("id=\"dock\"").next().expect("the page before its dock");
    assert!(board.contains(&format!("data-hand=\"{noted}\"")), "the note is on the board");
    assert!(!board.contains(&format!("id=\"{capture}\"")), "the import is drawn on the board");
    let since = html.split("<ul class=\"since\">").nth(1).and_then(|s| s.split("</ul>").next()).expect("recently done");
    assert!(since.contains(&format!("#dock-{noted}")), "the note taken is listed");
    assert!(!since.contains(&format!("#dock-{capture}")), "the import is listed as captured");
    assert!(tray_of(&html).contains(&format!("data-capture=\"{capture}\"")), "the import waits in the tray");
}

/// A finding a session offered with nothing above it is a row like any other:
/// its quote the document's path, its source offer, its line the session that
/// offered it, with the four controls and no decision; build now on it names
/// its chore from the path's words (S231, 62, S217).
#[test]
fn a_sessions_offer_is_a_tray_row_quoting_its_path() {
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let mut store = FakeStore::default();
    let mut world = world::Files::new().tracking("atlas");
    let session = "curation/willdan/1";
    let path = "openspec/changes/rows/findings/retry-schedule.md";
    // As `record_offers` writes a finding with nothing above its session.
    let key = format!("offer/{session}/1");
    let capture = Capture {
        key: key.clone(),
        source: "offer".into(),
        event_at: "2026-09-15T09:00:00+00:00".into(),
        captured_by: session.into(),
        raw: path.into(),
    };
    signals::write_capture(&mut world, &capture).expect("the capture");
    let at = commands::now(&store).expect("a point");
    let capture_id = signals::object_of(&key);
    let fields = [("source".to_string(), json!("offer")), ("event_key".to_string(), json!(key))];
    commands::put_new(&mut store, &defs, &capture_id, "capture", None, fields.into_iter().collect(), at).expect("its object");
    let signal = Signal {
        id: signals::signal_object(&key, 1),
        capture: capture_id.clone(),
        kind: "ask".into(),
        asserted_by: session.into(),
        assertion: path.into(),
        position: "whole".into(),
        ..Default::default()
    };
    signals::write_signal(&mut world, &key, 1, &signal).expect("the signal");
    commands::put_new(&mut store, &defs, &signal.id, "signal", Some(&capture_id), signal.fields(), at).expect("its object");

    let html = rendered(&mut store, &world, &defs);
    let tray = tray_of(&html);
    assert!(tray.contains("data-source=\"offer\""), "{tray}");
    assert!(tray.contains(&format!("offered by {session}")), "{tray}");
    assert!(tray.contains(&format!("<q>{path}</q>")), "the row quotes the path: {tray}");
    assert!(tray.contains(&format!("offer · offered by {session}")), "{tray}");
    assert!(tray.contains(&format!("data-hand=\"{}\"", signal.id)), "the row carries the four controls");
    assert!(commands::rail(&mut store, &defs).expect("the rail").is_empty(), "an offer raised a decision");

    let build = Call::new("propose-unit", "chuck", "page").arg("capture", json!(signal.id)).arg("repository", json!("atlas"));
    catalogue::call(&mut store, &mut world, &defs, &build).expect("built");
    let named = format!("unit/atlas/{}", signals::name_from_words(path));
    assert!(store.get(&named).expect("a read").is_some(), "build now names its chore from the path: {named}");
}
