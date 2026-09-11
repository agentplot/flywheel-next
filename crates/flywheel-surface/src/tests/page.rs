//! The page's own rendering: what the header carries, and what it raises.
//!
//! The order of authority is the requirements and `surfaces.md` first, the
//! operator's actual intended action second, and the mockup third, as the
//! record of what we were picturing. An element that serves no action does not
//! ship, whatever the mockup shows (amends D16).

use crate::testing as world;
use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::Records;
use flywheel_domain::commands;
use flywheel_engine::Definitions;
use serde_json::json;

const ADDRESS: &str = "http://studio.tailnet.ts.net/willdan";

fn a_page() -> (FakeStore, world::Files, Definitions) {
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    (FakeStore::default(), world::Files::new(), defs)
}

fn rendered(store: &mut FakeStore, world: &world::Files, defs: &Definitions) -> String {
    let read = crate::page::read(store, world, defs, ADDRESS, "chuck").expect("the page reads");
    crate::page::render(&read)
}

/// A bolt whose close is offered: one decision, standing (22, 39).
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
    commands::rail(store, defs).expect("the rail derives");
}

/// The ids inside one element of the document, by its opening tag.
fn ids_within(html: &str, opening: &str, closing: &str) -> Vec<String> {
    let start = html.find(opening).unwrap_or_else(|| panic!("`{opening}` is not on the page"));
    let rest = &html[start..];
    let end = rest.find(closing).unwrap_or_else(|| panic!("`{opening}` is never closed"));
    let within = &rest[..end];
    let mut out: Vec<String> = Vec::new();
    for at in within.match_indices("id=\"") {
        let tail = &within[at.0 + 4..];
        let Some(quote) = tail.find('"') else { continue };
        let id = &tail[..quote];
        if !id.is_empty() && !out.iter().any(|held| held == id) {
            out.push(id.to_string());
        }
    }
    out
}

/// What the header carries, and the action each one serves. Nothing stands
/// here that the operator does not act on.
///
/// This is the list the requirements justify, not the list the mockup draws.
/// Two of the mockup's header elements do not survive the question and are
/// named below the table with the reason each one failed it.
const HEADER: [(&str, &str); 7] = [
    (
        "orgname",
        "where the operator is: the instance this host serves, which is in the path of \
         every link the machinery writes (205a, 219)",
    ),
    (
        "clock",
        "what the page is as of, so what is read is dated and a stale tab is visible as \
         one (310)",
    ),
    (
        "pal-open",
        "a way to capture: the one thing a person arrives wanting to do, and the text is \
         captured whole with nothing in it read as a command (19, 194)",
    ),
    (
        "count",
        "what is waiting on them: how many decisions stand (15, 11)",
    ),
    (
        "acct-wrap",
        "who they are: the operators list's single entry, which is what every response \
         records as given by (153, 236a, 253a)",
    ),
    (
        "logbtn",
        "what they have already sent, each response on its own and the moment it was \
         given (154, 310)",
    ),
    ("sent", "how many those are: the count on that control (154)"),
];

/// The one element kept that answers a question rather than an action: the page
/// follows the system's theme and holds no client state a reload would lose, so
/// the label says so in place of a switch that would (310).
const KEPT_AS_A_STATEMENT: [&str; 1] = ["theme"];

/// What the mockup puts in the header and the page does not, with what each one
/// failed to name.
const NO_ACTION: [(&str, &str); 1] = [(
    "yesallhint",
    "a bare strip of every waiting decision number across the top names no action. It is \
     a mis-rendering of S2, which asks for the `yes all` control with the numbers it will \
     answer: the numbers belong on the control that answers them (S2, 17.5)",
)];

/// Every element in the header names an action, and the two that name none are
/// not on the page (S2, 141, 310, 79).
#[test]
fn the_header_carries_no_element_without_an_action() {
    let (mut store, world, defs) = a_page();
    // Three decisions, so there is a run of numbers for a strip to be made of.
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");
    a_decision(&mut store, &defs, "bolt/atlas/drop-the-tail");
    a_decision(&mut store, &defs, "bolt/switchboard/plan-rows");
    let html = rendered(&mut store, &world, &defs);

    let carried = ids_within(&html, "<header", "</header>");
    for (id, action) in HEADER {
        assert!(
            carried.iter().any(|held| held == id),
            "the header does not carry `{id}`, which serves: {action}"
        );
    }
    // And nothing else: an element arriving in the header has to be named here
    // with the action it serves, or it does not ship.
    let unnamed: Vec<&String> = carried
        .iter()
        .filter(|id| !HEADER.iter().any(|(named, _)| *named == id.as_str()))
        .filter(|id| !KEPT_AS_A_STATEMENT.contains(&id.as_str()))
        .collect();
    assert!(
        unnamed.is_empty(),
        "the header carries {unnamed:?}, and no action is named for them. Name the action \
         each one serves, or take it out (amends D16)"
    );

    for (id, why) in NO_ACTION {
        assert!(
            !html.contains(&format!("id=\"{id}\"")),
            "`{id}` is back on the page: {why}"
        );
    }
    // The strip itself, not only its id: the numbers standing are not strung
    // across the top in any form.
    let numbers: Vec<u32> = {
        let read = crate::page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("a read");
        read.decisions.iter().filter_map(|d| d.number).collect()
    };
    assert!(numbers.len() > 1, "several decisions stand, so there is a run to look for");
    let header = {
        let start = html.find("<header").expect("the header");
        let rest = &html[start..];
        rest[..rest.find("</header>").expect("the header closes")].to_string()
    };
    let strip: String = numbers.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
    assert!(
        !header.contains(&strip),
        "the numbers standing are strung across the header as `{strip}`. They belong on the \
         `yes all` control that will answer them (S2)"
    );
}

/// A host that is well raises nothing, and one that has gone or stalled is
/// raised into the operator's way (141, 143, 146, 79, 150a).
///
/// Noticing that a host has stopped is a real need, and it is an attention
/// need: every host is reported under the status view, and only what is wrong
/// with one is raised here. A permanent pill standing on a healthy instance
/// answers no question the operator has.
#[test]
fn a_healthy_host_raises_no_pill() {
    let (mut store, world, defs) = a_page();
    let at = commands::now(&mut store).expect("a point");
    // A host, holding work, and heard from just now.
    commands::put_new(&mut store, &defs, "host/studio", "host", None, Default::default(), at)
        .expect("the host object");
    store.seed_host("studio", 1);
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");
    store.seed_lease("bolt/atlas/plan-rows", "studio");

    let html = rendered(&mut store, &world, &defs);
    let raised = ids_within(&html, "<div class=\"hosts\"", "</div>");
    assert_eq!(raised, vec!["hosts".to_string()], "{raised:?}");
    let strip = {
        let start = html.find("id=\"hosts\"").expect("the region a host is raised into");
        let rest = &html[start..];
        rest[..rest.find("</div>").expect("it closes")].to_string()
    };
    assert!(
        !strip.contains("class=\"host"),
        "a well host is standing as a pill on a page where nothing is wrong: {strip}"
    );
    // It is reported all the same, under the status view with the rest of the
    // instance (141, 132).
    assert!(
        html.contains("id=\"dock-host/studio\""),
        "the host is not reported anywhere on the page"
    );
    assert!(
        html.contains(">host/studio<"),
        "the status view does not name the host"
    );

    // Six minutes on it is past its stale window, and that the operator does
    // need in their way: work it holds is not moving (150a, 79).
    store.set_now(at + chrono::Duration::minutes(6));
    let html = rendered(&mut store, &world, &defs);
    let strip = {
        let start = html.find("id=\"hosts\"").expect("the region a host is raised into");
        let rest = &html[start..];
        rest[..rest.find("</div>").expect("it closes")].to_string()
    };
    assert!(
        strip.contains("data-host=\"studio\""),
        "a host past its stale window is not raised: {strip}"
    );
    assert!(
        strip.contains("data-liveness=\"stale\""),
        "it does not say what is wrong with it: {strip}"
    );
}
