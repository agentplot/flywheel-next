//! Every list shows its first fifty rows with its count and a `more` that
//! fetches the next fifty; the rail is never paged (310a, S235, 15, S9).

use crate::testing as world;
use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::{CommitRef, Records};
use flywheel_domain::commands;
use flywheel_domain::signals::{self, Capture, Signal};
use flywheel_engine::Definitions;
use serde_json::json;

const ADDRESS: &str = "http://studio.tailnet.ts.net/willdan";
const BOLT: &str = "bolt/atlas/rows-lose-numbers";
const MEETING: &str = "meeting/2026-09-02/weekly";

fn defs() -> Definitions {
    flywheel_domain::set::load().expect("the embedded definitions")
}

/// One capture of `how_many` signals nothing has moved, as the blueprints hold
/// them.
fn signals_waiting(world: &mut world::Files, how_many: u64) {
    let capture = Capture {
        key: MEETING.into(),
        source: "meeting".into(),
        event_at: "2026-09-02".into(),
        captured_by: "dana".into(),
        raw: format!("raw://{MEETING}"),
    };
    signals::write_capture(world, &capture).expect("the capture");
    for ordinal in 1..=how_many {
        let signal = Signal {
            id: signals::signal_object(MEETING, ordinal),
            capture: signals::object_of(MEETING),
            kind: "question".into(),
            asserted_by: "dana".into(),
            assertion: format!("point {ordinal} of the weekly"),
            position: "whole".into(),
            ..Default::default()
        };
        signals::write_signal(world, MEETING, ordinal, &signal).expect("the signal");
    }
}

fn a_bolt(store: &mut FakeStore, defs: &Definitions, id: &str) {
    let at = commands::now(store).expect("a point");
    let record = [("repository".to_string(), json!("atlas"))].into_iter().collect();
    commands::put_new(store, defs, id, "bolt", None, record, at).expect("the bolt");
}

fn commits(how_many: usize) -> Vec<CommitRef> {
    (0..how_many)
        .map(|n| CommitRef {
            hash: format!("c{n:06}"),
            subject: format!("commit {n}"),
            author: "session".into(),
            at: "2026-09-14T12:30:00Z".into(),
        })
        .collect()
}

fn read(store: &mut FakeStore, world: &world::Files, defs: &Definitions) -> crate::page::Read {
    crate::page::read(store, world, defs, ADDRESS, "chuck").expect("the page reads")
}

/// A list past fifty rows shows fifty, says how many there are, and carries the
/// `more` that fetches from the fiftieth; at fifty or fewer it shows them all
/// and no `more` — over the signals tray and a bolt's commits (310a, S235).
#[test]
fn a_list_past_fifty_shows_fifty_its_count_and_more() {
    let defs = defs();
    let mut store = FakeStore::default();
    let mut world = world::Files::new();
    signals_waiting(&mut world, 60);
    a_bolt(&mut store, &defs, BOLT);
    let mut read = read(&mut store, &world, &defs);
    read.commits.insert(BOLT.into(), commits(60));

    let tray = crate::page::dock_page(&read, crate::page::tray::ID).expect("the tray");
    assert_eq!(tray.matches("class=\"quote tray-row\"").count(), 50, "the tray shows fifty rows");
    assert!(tray.contains("<span class=\"shown\">50 of 60</span>"), "the tray says how many wait: {tray}");
    assert!(
        tray.contains("class=\"more\" data-list=\"rows\" data-object=\"tray\" data-from=\"50\""),
        "the tray's more fetches from the fiftieth row: {tray}"
    );

    let bolt = crate::page::dock_page(&read, BOLT).expect("the bolt's page");
    assert_eq!(bolt.matches("<tr><td class=\"mono\">").count(), 50, "the bolt's page shows fifty commits");
    assert!(bolt.contains("<span class=\"shown\">50 of 60</span>"), "{bolt}");
    assert!(
        bolt.contains(&format!("<tr class=\"more\" data-list=\"commits\" data-object=\"{BOLT}\" data-from=\"50\">")),
        "the commits' more stands as a row of the table: {bolt}"
    );

    // Fifty or fewer: every row, and nothing to fetch.
    read.commits.insert(BOLT.into(), commits(50));
    let bolt = crate::page::dock_page(&read, BOLT).expect("the bolt's page");
    assert_eq!(bolt.matches("<tr><td class=\"mono\">").count(), 50);
    assert!(!bolt.contains("class=\"more\""), "fifty rows need no more: {bolt}");
}

/// A `more` fetches the next fifty rows and only those, with the next `more`
/// under them while rows remain (310a, S235).
#[test]
fn more_fetches_the_next_fifty() {
    let defs = defs();
    let mut store = FakeStore::default();
    let mut world = world::Files::new();
    signals_waiting(&mut world, 120);
    a_bolt(&mut store, &defs, BOLT);
    let mut read = read(&mut store, &world, &defs);
    read.commits.insert(BOLT.into(), commits(120));
    let row_of = |ordinal: u64| format!("data-signal=\"{}\"", signals::signal_object(MEETING, ordinal));

    let next = crate::page::list_part(&read, Some(crate::page::tray::ID), "rows", 50).expect("the tray's rows");
    assert_eq!(next.matches("class=\"quote tray-row\"").count(), 50, "{next}");
    assert!(next.contains(&row_of(51)) && next.contains(&row_of(100)), "the next fifty are the 51st to the 100th: {next}");
    assert!(!next.contains(&row_of(50)), "a row already shown is sent again");
    assert!(next.contains("data-capture=\"capture/meeting-2026-09-02-weekly\""), "the rows carry on under their capture");
    assert!(next.contains("<span class=\"shown\">100 of 120</span>") && next.contains("data-from=\"100\""), "{next}");
    let last = crate::page::list_part(&read, Some(crate::page::tray::ID), "rows", 100).expect("the last rows");
    assert_eq!(last.matches("class=\"quote tray-row\"").count(), 20);
    assert!(!last.contains("class=\"more\""), "nothing is left to fetch: {last}");

    let next = crate::page::list_part(&read, Some(BOLT), "commits", 50).expect("the bolt's commits");
    assert_eq!(next.matches("<tr><td class=\"mono\">").count(), 50);
    assert!(next.contains(">c000050<") && next.contains(">c000099<") && !next.contains(">c000049<"), "{next}");
    assert!(next.contains("<span class=\"shown\">100 of 120</span>"), "{next}");
    assert_eq!(crate::page::list_part(&read, Some(BOLT), "nothing", 50), None, "a list the page has not");
}

/// The rail is never paged: every decision is in its count, so sixty stand on it
/// whole, while the board past fifty is a list like any other (15, 310a, S235).
#[test]
fn the_rail_is_never_paged() {
    let defs = defs();
    let mut store = FakeStore::default();
    for n in 0..60 {
        let id = format!("bolt/atlas/close-{n:02}");
        a_bolt(&mut store, &defs, &id);
        let mut bolt = Records::get(&store, &id).expect("a read").expect("the bolt");
        bolt.config.insert("life".into(), "open".into());
        bolt.config.insert("life.open.close".into(), "offered".into());
        let base = bolt.seq;
        Records::put(&mut store, &id, &bolt, base).expect("its close offered");
    }
    commands::rail(&mut store, &defs).expect("the rail derives");
    let read = read(&mut store, &world::Files::new(), &defs);
    assert_eq!(read.decisions.len(), 60, "sixty decisions stand");
    let html = crate::page::render(&read);
    let (rail, board) = html.split_at(html.find("id=\"board\"").expect("the board"));
    let rail = &rail[rail.find("id=\"rail\"").expect("the rail")..];
    assert_eq!(rail.matches("<article class=\"card decision").count(), 60, "every decision stands on the rail");
    assert!(!rail.contains("class=\"more\""), "the rail is paged: {rail}");
    assert!(board.contains("class=\"more\""), "sixty bolts on the board are not paged");
}
