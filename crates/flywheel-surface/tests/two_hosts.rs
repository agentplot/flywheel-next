//! What a chat rendering says about an object another host holds: two hosts on
//! one state repository, which is the seam this asserts and the reason it is an
//! integration test rather than a unit one (D17, 150a, 308).

mod store;
use flywheel_surface::testing as world;

use flywheel_atoms::Records;
use flywheel_domain::commands;
use flywheel_domain::sinks::{self, Spec};
use flywheel_engine::Definitions;
use flywheel_store_git::GitStore;
use flywheel_surface::chat::{Chat, Recorded};
use serde_json::json;

/// Where the page is served from in these tests: a private-network name with
/// the instance in the path, never a localhost port (205a, 308, D10a).
const ADDRESS: &str = "http://studio.tailnet.ts.net/willdan";

/// A bolt whose close is offered: one decision, standing (22, 39).
fn a_decision(store: &mut GitStore, defs: &Definitions, id: &str) {
    let at = commands::now(store).expect("a point");
    commands::put_new(
        store,
        defs,
        id,
        "bolt",
        None,
        [("repository".to_string(), json!("atlas"))]
            .into_iter()
            .collect(),
        at,
    )
    .expect("the bolt");
    let mut bolt = store.get(id).expect("a read").expect("the bolt");
    bolt.config.insert("life".into(), "open".into());
    bolt.config.insert("life.open.close".into(), "offered".into());
    let seq = bolt.seq;
    store.put(id, &bolt, seq).expect("the close offered");
}

/// A link to an object held by a host past its stale window says the host is
/// away and since when, rather than failing silently (308, 150a).
#[test]
fn away_link_says_so() {
    let sandbox = store::Sandbox::new("away-link");
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let mut store = sandbox.store();
    let sink = sinks::ensure(&mut store, &defs, &Spec::chat("chat-chuck", "chuck", "#willdan"))
        .expect("the chat sink");
    let mut chat = Chat::new(&sink.id, "studio", ADDRESS, Recorded::new());
    assert!(chat.present(&mut store).expect("the presenter lease"));
    let world = world::Files::new();
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");

    // A laptop takes the object and then goes quiet. Its lease stands (150a).
    let seen = store.now;
    let mut mac = sandbox.store_as("mac-mini");
    mac.heartbeat(4, true).expect("the laptop's heartbeat");
    flywheel_atoms::StateStore::lease(
        &mut store,
        &flywheel_atoms::LeaseOp::Take {
            object: "bolt/atlas/plan-rows".into(),
            holder: "mac-mini".into(),
        },
    )
    .expect("the lease");
    store.fetch().expect("the host branch");

    // While it is inside its stale window nothing is said: it is simply here.
    let post = chat.rendering(&mut store, &defs).expect("a rendering");
    let line = post
        .lines
        .iter()
        .find(|l| l.object == "bolt/atlas/plan-rows")
        .expect("the decision");
    assert_eq!(line.away, None, "a live host was called away");

    // Six minutes on, past the 5-minute stale window (150a).
    store.now = seen + chrono::Duration::minutes(6);
    let post = chat.rendering(&mut store, &defs).expect("a rendering");
    let line = post
        .lines
        .iter()
        .find(|l| l.object == "bolt/atlas/plan-rows")
        .expect("the decision");
    let said = line.away.clone().expect("the link says nothing about the host");
    assert!(said.contains("mac-mini is away since"), "{said}");
    assert!(said.contains(&seen.to_rfc3339()), "it does not say since when: {said}");
    // The link still opens the object; what is added is what it says (308).
    assert_eq!(line.link, format!("{ADDRESS}/bolt/atlas/plan-rows"));
    assert!(line.text().contains(&said), "the posted line drops it: {}", line.text());

    // The page says it too, so the link that carried it opens on something that
    // does not fail silently (308, 150a).
    let page = flywheel_surface::page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("the page");
    let html = flywheel_surface::page::render(&page);
    assert!(
        html.contains("data-away-host=\"mac-mini\""),
        "the page says nothing about the away host"
    );
    assert!(html.contains("is away since"), "{html}");
}
