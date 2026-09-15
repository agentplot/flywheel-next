//! The ask: a record for planning that holds its words, filed by the operator
//! or by curation, and named by the route that offered it (28, 116).

use crate::{asks, commands, records, signals};
use chrono::{DateTime, Duration, TimeZone, Utc};
use flywheel_atoms::testing::{FakeStore, FakeWorld};
use flywheel_atoms::{Ask, Records};
use serde_json::json;

fn at(minute: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap() + Duration::minutes(minute)
}

fn an_ask(id: &str, repository: &str, by: &str, text: &str) -> Ask {
    Ask {
        id: id.into(),
        repository: repository.into(),
        text: text.into(),
        by: by.into(),
        at: at(1),
        consumed_by: None,
    }
}

/// A curation session routes a signal that argues with no claim to the ask it
/// filed: the route names the ask by the id its command printed, and that name
/// reads back the record the session wrote, words and all (116, 28,
/// `sessions.yaml` commands.ask).
#[test]
fn a_route_names_the_ask_curation_made() {
    let mut store = FakeStore::default();
    let mut world = FakeWorld::new().tracking("atlas");
    let defs = crate::set::load().unwrap();
    let signal = "signal/chat-1/1";
    commands::put_new(
        &mut store,
        &defs,
        signal,
        "signal",
        Some("capture/chat-1"),
        [("kind".to_string(), json!("ask"))].into_iter().collect(),
        at(0),
    )
    .unwrap();

    let session = "curation/willdan/curation/1";
    let id = asks::next_id(&store, "atlas").unwrap();
    assert_eq!(id, "atlas-1");
    let words = "keep the row numbers when the table is sorted";
    store
        .put_ask(&an_ask(&id, "atlas", &asks::by_session(session), words))
        .unwrap();
    let name = asks::name_of(&id);

    let applied = signals::apply_move(
        &mut store,
        &mut world,
        &signals::Move {
            signal: signal.into(),
            target: format!("route {name}"),
            reason: "it argues with no claim".into(),
            at: at(2).to_rfc3339(),
        },
        at(2),
    )
    .unwrap();
    assert_eq!(applied.target.as_deref(), Some(name.as_str()));
    let held = store.get(signal).unwrap().unwrap();
    assert_eq!(held.record.get("route"), Some(&json!(name)));
    let standing = signals::standing_move(&world, signal)
        .unwrap()
        .expect("the move stands");
    assert_eq!(standing.word(), "route");

    let asked = asks::named(&store, standing.names())
        .unwrap()
        .expect("the route names an ask on record");
    assert_eq!(asked.repository, "atlas");
    assert_eq!(asked.text, words);
    assert_eq!(asked.by, format!("session/{session}"));
    assert_eq!(asked.consumed_by, None, "nothing consumes an ask until planning does (28)");
}

/// An ask takes the next number of its own repository, counted from the
/// records, and the same id written again is not a second ask (15, 127).
#[test]
fn an_ask_takes_the_next_number_of_its_repository() {
    let mut store = FakeStore::default();
    for (repository, words) in [("atlas", "one"), ("atlas", "two"), ("switchboard", "three")] {
        let id = asks::next_id(&store, repository).unwrap();
        assert!(store.put_ask(&an_ask(&id, repository, "chuck", words)).unwrap());
    }
    let ids: Vec<String> = store.asks().unwrap().into_iter().map(|ask| ask.id).collect();
    assert_eq!(ids, ["atlas-1", "atlas-2", "switchboard-1"]);

    let again = store.asks().unwrap()[0].clone();
    assert!(!store.put_ask(&again).unwrap(), "the same id was written twice");
    assert_eq!(asks::next_id(&store, "atlas").unwrap(), "atlas-3");
    assert_eq!(asks::id_named("ask/atlas-2"), Some("atlas-2"));
    assert_eq!(asks::id_named("unit/atlas/rows"), None);
}

/// The ask is the curation session's and the operator's own session's, and
/// no other session's; a session's call is recorded by `session/<id>` (69,
/// 197, `sessions.yaml` commands.ask).
#[test]
fn only_curation_and_the_operators_own_session_file_an_ask() {
    for granted in [
        "curation/willdan/curation/1",
        "operator-session/chuck-1/with-operator/1",
        "session/curation/willdan/curation/1",
    ] {
        assert!(asks::granted(granted), "{granted} was refused the ask");
    }
    for refused in [
        "unit/atlas/rows/build/1",
        "elaboration/atlas/research-1/research/1",
        "planning/atlas/planner/1",
        "session/unit/atlas/u/main",
    ] {
        assert!(!asks::granted(refused), "{refused} was granted the ask");
    }
    assert_eq!(
        asks::by_session("curation/willdan/curation/1"),
        "session/curation/willdan/curation/1"
    );
    assert_eq!(asks::by_session("session/unit/atlas/u/main"), "session/unit/atlas/u/main");
}

/// The record holds the words whole, however many lines they run to, and a
/// consumer once planning has one (28, 62, git-only `layout.asks`).
#[test]
fn an_ask_record_holds_its_words() {
    let ask = an_ask(
        "atlas-1",
        "atlas",
        "session/curation/willdan/curation/1",
        "keep the row numbers\nwhen the table is sorted",
    );
    let written = flywheel_engine::rec::write(std::slice::from_ref(&records::ask_to_record(&ask)));
    let read = records::ask_from_record(&flywheel_engine::rec::parse(&written)[0]).unwrap();
    assert_eq!(read, ask);

    let consumed = Ask {
        consumed_by: Some("unit/atlas/rows".into()),
        ..ask
    };
    let written = flywheel_engine::rec::write(std::slice::from_ref(&records::ask_to_record(&consumed)));
    let read = records::ask_from_record(&flywheel_engine::rec::parse(&written)[0]).unwrap();
    assert_eq!(read, consumed);
}
