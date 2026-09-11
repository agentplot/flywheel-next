//! A tick that moves nothing writes nothing, and a read writes nothing at all.
//!
//! On the git profile every write is a commit at the shared line (161, 167), so
//! these are not economies: a loop that wrote four commits a pass forever made
//! `main` unreadable as an audit record and made every page request wait on a
//! push. 78 states the rule and this tier is where it is held.

use crate::commands;
use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::Records;
use flywheel_engine::Definitions;
use serde_json::json;

fn a_store() -> (FakeStore, Definitions) {
    let defs = crate::set::load().expect("the embedded definitions");
    (FakeStore::default(), defs)
}

/// A bolt whose close is offered: one standing decision to number.
fn a_decision(store: &mut FakeStore, defs: &Definitions, id: &str) {
    let at = commands::now(store).expect("a point");
    commands::put_new(
        store,
        defs,
        id,
        "bolt",
        None,
        [("repository".to_string(), json!("atlas"))].into_iter().collect(),
        at,
    )
    .expect("the bolt");
    let mut bolt = Records::get(store, id).expect("a read").expect("the bolt");
    bolt.config.insert("life".into(), "open".into());
    bolt.config.insert("life.open.close".into(), "offered".into());
    let base = bolt.seq;
    Records::put(store, id, &bolt, base).expect("the bolt with its close offered");
}

/// Deriving the rail twice with nothing changed in between writes once: the
/// first derive numbers what is unnumbered, the second has nothing to say (78,
/// 15).
#[test]
fn numbering_twice_writes_once() {
    let (mut store, defs) = a_store();
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");

    let before = store.writes();
    let first = commands::rail(&mut store, &defs).expect("the rail derives");
    assert_eq!(first.len(), 1);
    assert!(first[0].number.is_some(), "the first derive numbers it");
    let after_first = store.writes();
    assert!(after_first > before, "the register was written");

    commands::rail(&mut store, &defs).expect("the rail derives again");
    assert_eq!(
        store.writes(),
        after_first,
        "a derive that gave no new number and retracted nothing wrote the rail record again"
    );
}

/// The page's read of the rail writes nothing at all.
///
/// Numbering is the rail machine's own effect, held by the holder of the rail's
/// lease (D12), and a request is not a tick. A read that renumbered put a
/// commit and a push on the request path, made by a host that may not hold the
/// lease — which is what the operator was waiting on when they loaded the page
/// (15, 310).
#[test]
fn reading_the_rail_writes_nothing() {
    let (mut store, defs) = a_store();
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");
    commands::rail(&mut store, &defs).expect("the tick numbers it");

    let before = store.writes();
    let read = commands::rail_read(&store, &defs).expect("the rail reads");
    assert_eq!(read.len(), 1);
    assert_eq!(read[0].number, Some(1), "it carries the number the tick gave");
    assert_eq!(store.writes(), before, "reading the rail wrote to the store");

    // And a decision the tick has not numbered yet reads without one rather
    // than taking a number on the request path.
    a_decision(&mut store, &defs, "bolt/atlas/drop-the-tail");
    let before = store.writes();
    let read = commands::rail_read(&store, &defs).expect("the rail reads");
    assert_eq!(store.writes(), before, "reading the rail numbered a decision");
    assert_eq!(
        read.iter().filter(|d| d.number.is_none()).count(),
        1,
        "the unnumbered decision reads as unnumbered"
    );
}

/// The decisions standing after the last derive are on the rail record, where
/// `response.decision_present` is read against them.
///
/// A reader that took this for empty called every answer the operator ever gave
/// unapplicable and put each one back on the rail as an attention line, which
/// is exactly what 13 says never happens.
#[test]
fn the_standing_set_is_on_the_rail_record() {
    let (mut store, defs) = a_store();
    assert!(commands::standing(&store).expect("a read").is_empty());

    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");
    let derived = commands::rail(&mut store, &defs).expect("the rail derives");
    let standing = commands::standing(&store).expect("a read");
    assert_eq!(
        standing,
        derived.iter().map(|d| d.id.clone()).collect::<Vec<_>>(),
        "the rail record does not hold what stands"
    );
}

/// A tick over a settled instance writes nothing.
///
/// The rail's status region re-enters the state it is in whenever the
/// projection is behind its source, and the transition moves nothing: no state
/// changed, no response was consumed, no counter turned. Writing it is a commit
/// that says only that the sequence went up (78).
#[test]
fn a_tick_that_moves_nothing_writes_nothing() {
    let (mut store, defs) = a_store();
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");
    // Settle it: tick until nothing moves.
    for _ in 0..8 {
        let before = store.writes();
        commands::tick(
            &mut store,
            &defs,
            &flywheel_atoms::Scope::All,
            |_, _, _, _| true,
            |_, _, _| {},
        )
        .expect("a tick");
        if store.writes() == before {
            break;
        }
    }

    let before = store.writes();
    commands::tick(
        &mut store,
        &defs,
        &flywheel_atoms::Scope::All,
        |_, _, _, _| true,
        |_, _, _| {},
    )
    .expect("a tick over a settled instance");
    assert_eq!(
        store.writes(),
        before,
        "a tick over a settled instance wrote; on the git profile each of those is a commit \
         at the shared line and a push the operator waits on (78, 161, 167)"
    );
}

/// A transition that re-enters the state it is in reports no write.
///
/// The run record is what a reader trusts to say what a host did, and an entry
/// for a state change that did not happen is the one thing it must not hold
/// (79, 167).
#[test]
fn a_re_entry_is_not_reported_as_a_write() {
    let (mut store, defs) = a_store();
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");
    for _ in 0..8 {
        let before = store.writes();
        commands::tick(&mut store, &defs, &flywheel_atoms::Scope::All, |_, _, _, _| true, |_, _, _| {})
            .expect("a tick");
        if store.writes() == before {
            break;
        }
    }

    let mut noted: Vec<String> = Vec::new();
    commands::tick(
        &mut store,
        &defs,
        &flywheel_atoms::Scope::All,
        |_, _, _, _| true,
        |_, fired, _| noted.push(format!("{} {}: {} → {}", fired.object, fired.region, fired.from, fired.to)),
    )
    .expect("a tick over a settled instance");
    assert!(
        noted.is_empty(),
        "a settled tick reported writes that did not happen: {noted:?}"
    );
}
