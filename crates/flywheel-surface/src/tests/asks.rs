//! The `ask` tool: a dictation's record holding its words, one per delivery,
//! refused for a repository the instance does not track; and the curator's
//! route, which files its ask by the same call before it names it (28, 116,
//! 137, 193).

use crate::catalogue::{self, Call};
use crate::testing as world;
use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::{Records, Scope, StateStore};
use serde_json::json;

fn defs() -> flywheel_engine::Definitions {
    flywheel_domain::set::load().expect("the embedded definitions")
}

/// The operator's ask is one record under `asks/`, written as the dictation's
/// effect: the repository, the words, who gave it, when, and no consumer yet.
/// The call is recorded once as the response naming the ask, and the same
/// delivery twice is one ask (28, 116, 137, 153).
#[test]
fn the_ask_tool_writes_the_ask_record_with_its_words() {
    let mut store = FakeStore::default();
    let mut world = world::Files::new().tracking("atlas").tracking("switchboard");
    let defs = defs();
    let words = "keep the row numbers when the table is sorted";
    let call = Call::new("ask", "chuck", "page")
        .arg("repository", json!("atlas"))
        .arg("text", json!(words))
        .delivered("page-41");
    let outcome = catalogue::call(&mut store, &mut world, &defs, &call).expect("the ask is filed");
    assert_eq!(catalogue::asked(&outcome).as_deref(), Some("ask/atlas-1"));

    let asks = store.asks().expect("a read");
    assert_eq!(asks.len(), 1, "{asks:?}");
    let ask = &asks[0];
    assert_eq!(ask.id, "atlas-1");
    assert_eq!(ask.repository, "atlas");
    assert_eq!(ask.text, words, "a dictation's record holds its words (62 is about documents)");
    assert_eq!(ask.by, "chuck");
    assert_eq!(ask.consumed_by, None, "nothing consumes an ask until planning does (28)");

    let response = store
        .get("response/page-41")
        .expect("a read")
        .expect("the call was recorded");
    assert_eq!(response.record.get("tool"), Some(&json!("ask")));
    assert_eq!(response.record.get("object"), Some(&json!("ask/atlas-1")));
    assert_eq!(response.record.get("given_by"), Some(&json!("chuck")));
    assert_eq!(response.record.get("given_at"), Some(&json!(ask.at.to_rfc3339())));
    assert!(
        flywheel_domain::commands::is_operation("ask"),
        "the response machine would report the ask unapplicable (4, 6)"
    );

    // The same delivery again is the same ask, and nothing more is written.
    let again = catalogue::call(&mut store, &mut world, &defs, &call).expect("the repeat");
    assert_eq!(catalogue::asked(&again).as_deref(), Some("ask/atlas-1"));
    assert_eq!(store.asks().unwrap().len(), 1, "the repeat wrote a second ask");

    // Another repository's ask takes that repository's first number (15).
    let other = Call::new("ask", "chuck", "page")
        .arg("repository", json!("switchboard"))
        .arg("text", json!("page the on-call when a provider rate-limits"));
    let filed = catalogue::call(&mut store, &mut world, &defs, &other).expect("a second ask");
    assert_eq!(catalogue::asked(&filed).as_deref(), Some("ask/switchboard-1"));
}

/// A repository the instance does not track is refused, naming the ones it
/// does, and nothing is written: no ask and no response (205, 206).
#[test]
fn an_ask_naming_an_untracked_repository_is_refused_with_the_tracked_names() {
    let mut store = FakeStore::default();
    let mut world = world::Files::new().tracking("atlas").tracking("switchboard");
    let call = Call::new("ask", "chuck", "page")
        .arg("repository", json!("storefront"))
        .arg("text", json!("a sale banner"));
    let refused = catalogue::call(&mut store, &mut world, &defs(), &call)
        .expect_err("storefront is no repository the instance tracks");
    let said = refused.to_string();
    assert!(said.contains("atlas") && said.contains("switchboard"), "{said}");
    assert!(store.asks().unwrap().is_empty(), "a refused ask was written");
    let responses = store.list(&Scope::Machine("response".into())).unwrap().objects;
    assert!(responses.is_empty(), "a refused ask was recorded: {responses:?}");
}

/// On the curator's surface a route files its ask with the words the operator
/// gave, by the same call, and the route names the ask it filed; the one
/// repository the instance tracks is the one meant. A route that names nothing
/// and asks nothing is refused before anything is written (116, 93b, 193).
#[test]
fn a_route_on_the_curators_surface_files_its_ask_and_names_it() {
    let mut store = FakeStore::default();
    let mut world = world::Files::new().tracking("atlas");
    let defs = defs();
    let session = "curation/willdan/curation/1";
    let signal = "signal/atlas/rows/1";
    let call = Call::new("curate", "chuck", "page")
        .arg("session", json!(session))
        .arg(&format!("move.{signal}"), json!("route"))
        .arg(&format!("ask.{signal}"), json!("keep the row numbers when sorted"))
        .arg(&format!("repository.{signal}"), json!(""));
    catalogue::call(&mut store, &mut world, &defs, &call).expect("the moves are delivered");

    let asks = store.asks().unwrap();
    assert_eq!(asks.len(), 1, "{asks:?}");
    assert_eq!(asks[0].repository, "atlas");
    assert_eq!(asks[0].text, "keep the row numbers when sorted");
    assert_eq!(asks[0].by, "chuck", "the operator runs curation's session and gave the ask (93b)");
    let standing = flywheel_domain::signals::standing_move(&world, signal)
        .unwrap()
        .expect("the move stands");
    assert_eq!(standing.target, "route ask/atlas-1");

    let bare = Call::new("curate", "chuck", "page")
        .arg("session", json!(session))
        .arg("move.signal/atlas/rows/2", json!("route"));
    let refused = catalogue::call(&mut store, &mut world, &defs, &bare)
        .expect_err("a route names what was offered for its signal");
    assert!(refused.to_string().contains("116"), "{refused}");
    assert!(flywheel_domain::signals::standing_move(&world, "signal/atlas/rows/2")
        .unwrap()
        .is_none());
}
