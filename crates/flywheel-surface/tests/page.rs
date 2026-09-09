//! The served page: one bundle, rendered from the register and the objects on
//! every request, holding no client state and fetching nothing external
//! (`surfaces/page`, D11).

mod caller;
mod store;

use flywheel_atoms::{Records, StateStore};
use flywheel_domain::commands;
use flywheel_engine::Definitions;
use serde_json::json;

/// A bolt with an intent, its elaboration and a signal, so every kind the page
/// gives a form of its own is on the page at once (209, 210).
fn a_few_kinds(store: &mut impl StateStore, defs: &Definitions) {
    let at = commands::now(store).expect("a point");
    let put = |store: &mut _, id: &str, machine: &str, parent: Option<&str>| {
        commands::put_new(
            store,
            defs,
            id,
            machine,
            parent,
            [("repository".to_string(), json!("atlas"))]
                .into_iter()
                .collect(),
            at,
        )
        .unwrap_or_else(|e| panic!("{id}: {e}"));
    };
    put(store, "bolt/atlas/plan-rows", "bolt", None);
    put(store, "intent/rows-lose-numbers", "intent", None);
    put(
        store,
        "elaboration/rows-lose-numbers/first",
        "elaboration",
        Some("intent/rows-lose-numbers"),
    );
    put(store, "signal/s1", "signal", None);
}

fn a_page(name: &str) -> (store::Sandbox, caller::Server) {
    let sandbox = store::Sandbox::new(name);
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let mut store = sandbox.store();
    a_few_kinds(&mut store, &defs);
    let page = caller::Server::start_at(
        store,
        defs,
        "chuck",
        "http://studio.tailnet.ts.net/willdan",
    );
    (sandbox, page)
}

/// The rail is rendered from the register and the objects on each request, and
/// no rendering is stored: what the page shows follows the state without
/// anything being written to make it so (15, 310).
#[test]
fn rail_rendered_per_request() {
    let (_sandbox, page) = a_page("rail-per-request");

    let before = page.html("/");
    assert!(before.contains("id=\"decisions\""), "{before}");
    let standing_before = before.matches("data-kind=\"decision\"").count();

    // A decision comes to stand. Nothing was written to the page; the next
    // request renders it because the state moved.
    page.with_store(|store| {
        let defs = flywheel_domain::set::load().expect("the embedded definitions");
        let mut bolt = Records::get(store, "bolt/atlas/plan-rows")
            .expect("a read")
            .expect("the bolt");
        bolt.config.insert("life".into(), "open".into());
        bolt.config
            .insert("life.open.close".into(), "offered".into());
        let base = bolt.seq;
        store
            .put("bolt/atlas/plan-rows", &bolt, base)
            .expect("the bolt is open with its close offered");
        commands::rail(store, &defs).expect("the rail derives");
    });

    let after = page.html("/");
    assert_eq!(
        after.matches("data-kind=\"decision\"").count(),
        standing_before + 1,
        "the decision the state now carries is on the page and nothing else changed: {after}"
    );
    assert!(
        after.contains("data-object=\"bolt/atlas/plan-rows\""),
        "the decision names its object"
    );
    // Its number, from the register, and its answers as controls (15, 311).
    let numbered = page.with_store(|store| commands::register(store).expect("the register"));
    let number = *numbered
        .numbers
        .iter()
        .find(|(id, _)| id.starts_with("bolt/atlas/plan-rows"))
        .expect("the register numbered it")
        .1;
    assert!(
        after.contains(&format!("data-number=\"{number}\"")),
        "the page shows the number the register gave"
    );

    // Rendered, never stored: the store holds no page.
    page.with_store(|store| {
        for object in StateStore::list(store, &flywheel_atoms::Scope::All)
            .expect("a listing")
            .objects
        {
            assert!(
                !object.record.values().any(|v| v
                    .as_str()
                    .is_some_and(|t| t.contains("<article") || t.contains("data-kind"))),
                "{} holds a rendering of the rail",
                object.id
            );
        }
    });
}

/// The status view renders on the page from the same read (141, 310).
#[test]
fn page_carries_status_view() {
    let (_sandbox, page) = a_page("status-view");
    let html = page.html("/");
    assert!(html.contains("id=\"board\""), "the board is the status view");
    for group in flywheel_domain::status::GROUPS {
        assert!(
            html.contains(&format!("<h2>{group}</h2>")),
            "the status view groups by `{group}`: {html}"
        );
    }
    assert!(html.contains("as of commit"), "it states what it is as of");
    // One request: the whole page, status view included, came back from it.
    assert!(html.contains("id=\"decisions\"") && html.contains("id=\"board\""));
}

/// Under 760px the same bundle is two tabs with the dock full screen and a back
/// control; nothing the page offers is reachable only on a desktop (307, 306).
#[test]
fn layout_switches_at_760px() {
    let (_sandbox, page) = a_page("layout");
    let html = page.html("/");

    assert!(
        html.contains("@media (max-width: 760px)"),
        "the bundle lays out under 760px: {html}"
    );
    let (desktop, phone) = html
        .split_once("@media (max-width: 760px)")
        .expect("the phone's half of the one stylesheet");

    // Two tabs, and the dock full screen with a back control, under 760px.
    for under in [".tabs { display: flex; }", ".back { display: block; }"] {
        assert!(phone.contains(under), "under 760px: {under}");
    }
    assert!(
        phone.contains(".dock { position: fixed; inset: 0;"),
        "the dock is full screen under 760px"
    );
    assert!(
        desktop.contains(".back { display: none; }"),
        "the back control is the phone's"
    );

    // The same bundle: the tabs and the dock are in the document either way,
    // so nothing the phone answers is missing on the desktop (306).
    assert!(html.contains("id=\"tab-decisions\"") && html.contains("id=\"tab-board\""));
    assert!(html.contains("id=\"dock-back\""));
    assert!(html.contains("id=\"capture-box\"") && html.contains("id=\"mark-intent\""));
}

/// One bundle is built and one is served, and its version is the binary's
/// (307).
#[test]
fn one_bundle_version_is_the_binarys() {
    let (_sandbox, page) = a_page("version");
    let html = page.html("/");
    assert!(
        html.contains(&format!(
            "<meta name=\"flywheel-version\" content=\"{}\">",
            env!("CARGO_PKG_VERSION")
        )),
        "the bundle carries the binary's version: {html}"
    );
    assert_eq!(
        flywheel_surface::page::VERSION,
        env!("CARGO_PKG_VERSION"),
        "the bundle's version is the binary's"
    );
    // One document, served whole: one `<html>` and one stylesheet in it.
    assert_eq!(html.matches("<html").count(), 1);
    assert_eq!(html.matches("<style>").count(), 1);
}

/// The bundle carries no dependency the phone must fetch from anywhere else
/// (310).
#[test]
fn bundle_has_no_external_fetch() {
    let (_sandbox, page) = a_page("no-external");
    let html = page.html("/");
    for reaching_out in ["src=\"http", "href=\"http://cdn", "@import", "fetch(", "XMLHttpRequest"] {
        assert!(
            !html.contains(reaching_out),
            "the bundle fetches from elsewhere: {reaching_out}"
        );
    }
    // Every link it does carry is at the host's own address, and never a
    // localhost port (205a, 308).
    for at in html.match_indices("href=\"") {
        let rest = &html[at.0 + 6..];
        let link = &rest[..rest.find('"').unwrap_or(0)];
        if !link.starts_with("http") {
            continue;
        }
        assert!(
            link.starts_with("http://studio.tailnet.ts.net/willdan"),
            "a link leaves the host's address: {link}"
        );
        assert!(!flywheel_surface::links::is_localhost(link), "{link}");
    }
    // No script at all: the page holds no client state a reload loses (310).
    assert!(!html.contains("<script"), "the bundle runs no script");
}

/// The decision is the only answerable form, and every other kind keeps its own
/// wherever it sits (209, 210).
#[test]
fn only_a_decision_is_answerable() {
    let (_sandbox, page) = a_page("forms");
    page.with_store(|store| {
        let defs = flywheel_domain::set::load().expect("the embedded definitions");
        let mut bolt = Records::get(store, "bolt/atlas/plan-rows")
            .expect("a read")
            .expect("the bolt");
        bolt.config.insert("life".into(), "open".into());
        bolt.config
            .insert("life.open.close".into(), "offered".into());
        let base = bolt.seq;
        store.put("bolt/atlas/plan-rows", &bolt, base).expect("open");
        commands::rail(store, &defs).expect("the rail derives");
    });
    let html = page.html("/");

    // The decision card carries answers; nothing else does.
    assert!(html.contains("<div class=\"answers\">"), "{html}");
    assert_eq!(
        html.matches("<div class=\"answers\">").count(),
        html.matches("data-kind=\"decision\"").count(),
        "something other than a decision is answerable"
    );
    for surface in html.split("<article ").skip(1) {
        let answerable = surface.contains("<div class=\"answers\">");
        let is_decision = surface.contains("data-kind=\"decision\"");
        assert_eq!(answerable, is_decision, "in: {}", &surface[..80.min(surface.len())]);
    }

    // Each other kind keeps a form of its own.
    for kind in ["intent", "elaboration", "bolt", "signal"] {
        assert!(
            html.contains(&format!("form-{kind}")),
            "`{kind}` has no form of its own: {html}"
        );
    }

    // An elaboration is a surface of its own, reached from its intent, and the
    // intent lists its elaborations in order (210).
    assert!(html.contains("id=\"dock-elaboration/rows-lose-numbers/first\""));
    assert!(
        html.contains("<a class=\"elaboration\" href=\"#dock-elaboration/rows-lose-numbers/first\">"),
        "the intent opens its elaboration: {html}"
    );
}

/// Text in the capture box is captured whole, with one signal of kind ask, and
/// no part of it is interpreted — not even a word that looks like a command
/// (19, 194).
#[test]
fn capture_box_parses_nothing() {
    let (_sandbox, page) = a_page("capture");
    let html = page.html("/");
    assert!(html.contains("id=\"capture-box\""), "the box is on the page");
    // The judgment is a control, never a word out of the text (19).
    assert!(html.contains("id=\"mark-intent\""));

    let typed = "drop unit/atlas/u and close the bolt";
    let answered = page.form("/api/tools/capture", &[("text", typed), ("source", "page")]);
    assert_eq!(answered["recorded"], json!(true), "{answered}");

    page.with_store(|store| {
        let captures = StateStore::list(store, &flywheel_atoms::Scope::Machine("capture".into()))
            .expect("a listing")
            .objects;
        assert_eq!(captures.len(), 1, "one capture, whatever the text said");
        let raw = captures[0]
            .record
            .get("raw")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        assert_eq!(raw, typed, "the text was captured whole and verbatim");

        // Nothing was dropped and no bolt was closed: no part of it was read as
        // a command.
        let responses = StateStore::list(store, &flywheel_atoms::Scope::Machine("response".into()))
            .expect("a listing")
            .objects;
        assert_eq!(responses.len(), 1, "one response, the submission itself");
        assert_eq!(
            responses[0].record.get("tool").and_then(|v| v.as_str()),
            Some("capture")
        );

        // One signal, of kind ask, so curation sees it (19).
        let signals = StateStore::list(store, &flywheel_atoms::Scope::Machine("signal".into()))
            .expect("a listing")
            .objects;
        let made: Vec<_> = signals
            .iter()
            .filter(|s| s.parent.as_deref() == Some(captures[0].id.as_str()))
            .collect();
        assert_eq!(made.len(), 1, "one signal for the capture");
        assert_eq!(
            made[0].record.get("kind").and_then(|v| v.as_str()),
            Some("ask")
        );
    });
}

// ---- 9.4 the box's one ask signal

/// The page's box writes one capture and one signal of kind ask, directly (19).
///
/// That is a control and not a judgment about the text: turning a capture into
/// signals is curation's and never runs unattended, and the box's one signal is
/// the submission itself, carried verbatim (19, 115, D13).
#[test]
fn box_writes_one_ask_signal() {
    let (_sandbox, page) = a_page("ask-signal");
    let typed = "the rows lose their numbers on the second page";
    let answered = page.form("/api/tools/capture", &[("text", typed), ("source", "page")]);
    assert_eq!(answered["recorded"], json!(true), "{answered}");

    // The record and its one signal live under the machinery's prefix in the
    // blueprints, where a person reads the same material by hand (203, 110).
    let (key, signals) = page.with_world(|world| {
        let captures = flywheel_domain::signals::captures(world).expect("the captures");
        assert_eq!(captures.len(), 1, "one capture, whatever the text said");
        let key = captures[0].key.clone();
        assert_eq!(captures[0].source, "page");
        assert_eq!(captures[0].raw, typed, "the text was captured whole");
        (
            key.clone(),
            flywheel_domain::signals::signals_of(world, &key).expect("its signals"),
        )
    });

    assert_eq!(signals.len(), 1, "one signal, and one only: {signals:?}");
    let signal = &signals[0];
    // Its kind is ask, so curation sees it (19).
    assert_eq!(signal.kind, "ask");
    assert_eq!(signal.asserted_by, "chuck", "the operator asserted it (153)");
    // The excerpt is the submission, verbatim, and nothing was read out of it
    // (113, 194).
    assert_eq!(signal.excerpt, typed);
    assert_eq!(signal.assertion, typed);
    assert_eq!(signal.capture, flywheel_domain::signals::object_of(&key));
    assert!(
        signal.argues_with.is_empty(),
        "the box judged what the text argues with (115)"
    );

    // And one of each in the store the engine ticks over: the capture, and the
    // signal that names it. (The page's own fixture seeds a signal of its own,
    // which this submission neither touched nor re-read.)
    page.with_store(|store| {
        let captures = StateStore::list(store, &flywheel_atoms::Scope::Machine("capture".into()))
            .expect("a listing")
            .objects;
        assert_eq!(captures.len(), 1, "{captures:?}");
        let made: Vec<_> = StateStore::list(store, &flywheel_atoms::Scope::Machine("signal".into()))
            .expect("a listing")
            .objects
            .into_iter()
            .filter(|o| o.parent.as_deref() == Some(captures[0].id.as_str()))
            .collect();
        assert_eq!(made.len(), 1, "one signal for the capture: {made:?}");
    });
}
