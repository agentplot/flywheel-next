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
    let number = numbered
        .entries
        .iter()
        .find(|(id, _)| id.starts_with("bolt/atlas/plan-rows"))
        .expect("the register numbered it")
        .1
        .number;
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
    // The one stylesheet, at the host's own versioned address (310a, S235).
    let style = page.html(&flywheel_surface::page::style_address());

    assert!(
        style.contains("@media (max-width: 760px)"),
        "the bundle lays out under 760px: {style}"
    );
    let (desktop, phone) = style
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
    // Full screen when an object is open, and not before: a dock covering the
    // rail would put every decision behind it, which is the one thing the phone
    // must not do (307, 311). Open is either of the two ways a surface opens —
    // a link inside the page targets it, and a link the machinery wrote names
    // its object in the path and carries no fragment, so the request marks it
    // (308, 205a).
    let hidden = phone
        .lines()
        .find(|line| line.trim_start().starts_with(".dock:not(") && line.contains("display: none"))
        .expect("the dock covers the rail only once an object is opened");
    assert!(
        hidden.contains(":has(.surface:target)") && hidden.contains("data-opened=\"true\""),
        "the dock is shut for one of the two ways a surface opens: {hidden}"
    );
    assert!(
        desktop.contains(".back { display: none; }"),
        "the back control is the phone's"
    );

    // The same bundle: the tabs and the dock are in the document either way,
    // so nothing the phone answers is missing on the desktop (306).
    assert!(html.contains("id=\"tab-decisions\"") && html.contains("id=\"tab-board\""));
    assert!(html.contains("id=\"dock-back\""));
    assert!(html.contains("id=\"capture-box\""));
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
    // One document: one `<html>`, and one stylesheet, linked at the host's own
    // address under the binary's version rather than carried in it (310a, S235).
    assert_eq!(html.matches("<html").count(), 1);
    assert_eq!(html.matches("<style").count(), 0, "a stylesheet rides in the page");
    assert_eq!(html.matches("rel=\"stylesheet\"").count(), 1, "one stylesheet");
    assert!(
        html.contains(&format!(
            "<link rel=\"stylesheet\" href=\"/bundle/page.{}.css\">",
            env!("CARGO_PKG_VERSION")
        )),
        "the stylesheet is not linked at the host's own versioned address: {html}"
    );
}

/// The bundle carries no dependency the phone must fetch from anywhere else
/// (310).
#[test]
fn bundle_has_no_external_fetch() {
    let (_sandbox, page) = a_page("no-external");
    let html = page.html("/");
    // The stylesheet and the script the page links, as the host serves them
    // (310a, S235).
    let style = page.html(&flywheel_surface::page::style_address());
    let script = page.html(&flywheel_surface::page::script_address());
    for reaching_out in ["src=\"http", "href=\"http://cdn", "@import", "fetch(\"http", "fetch('http", "XMLHttpRequest", "WebSocket("] {
        for (what, text) in [("the page", &html), ("its stylesheet", &style), ("its script", &script)] {
            assert!(!text.contains(reaching_out), "{what} fetches from elsewhere: {reaching_out}");
        }
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
    // The two faces are fetched past the document, and from the page's own
    // host alone, under a name carrying the binary's version (310a, S235, 291).
    let fetched: Vec<&str> = style
        .match_indices("url(")
        .map(|(at, _)| {
            let rest = &style[at + 4..];
            &rest[..rest.find(')').unwrap_or(0)]
        })
        .collect();
    assert_eq!(fetched.len(), flywheel_surface::page::FONTS.len(), "the stylesheet fetches {fetched:?}");
    for (name, ..) in flywheel_surface::page::FONTS {
        let own = flywheel_surface::page::font_address(name);
        assert!(fetched.contains(&own.as_str()), "{name} is not fetched from the host: {fetched:?}");
        assert!(own.starts_with("/fonts/") && own.contains(flywheel_surface::page::VERSION), "{own}");
    }
    // No client state a reload loses, and nothing fetched from elsewhere: the
    // one script the page carries is its keys and its own refresh, which
    // fetches this page from this host and keeps nothing (310, 311, S221).
    // No script from anywhere but the host's own versioned address.
    let scripts: Vec<&str> = html
        .match_indices("<script")
        .map(|(at, _)| &html[at..at + html[at..].find('>').unwrap_or(0)])
        .collect();
    assert_eq!(
        scripts,
        vec![format!("<script src=\"{}\"", flywheel_surface::page::script_address()).as_str()],
        "the page loads a script from somewhere other than its host's versioned address"
    );
    for kept in ["fetch(\"http", "fetch('http", "localStorage", "sessionStorage", "XMLHttpRequest", "indexedDB"] {
        assert!(!html.contains(kept) && !script.contains(kept), "the page keeps client state or fetches from elsewhere: `{kept}`");
    }
    assert!(script.contains("fetch(location.pathname"), "the page fetches itself when the host says it moved (S221)");
    assert!(script.contains("new EventSource('/events')"), "the page listens for the host's changes (S221)");
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
    let first = page.html("/");
    // Each object's dock page, as the drawer fetches it when it is opened: the
    // first view carries none a link did not name (310a, S235).
    let docks: String = ["bolt/atlas/plan-rows", "intent/rows-lose-numbers", "elaboration/rows-lose-numbers/first", "signal/s1"]
        .iter()
        .map(|id| page.html(&format!("/willdan/{id}?part=dock")))
        .collect();
    let html = first + &docks;

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
    // Nothing beside the field is a judgment about the text (19, 34).
    assert!(!html.contains("id=\"mark-intent\""));

    let typed = "drop unit/atlas/u and close the bolt";
    let answered = page.form("/api/tools/capture", &[("text", typed), ("source", "page")]);
    assert!(answered.recorded(), "the control sent the operator back to the page: {:?}", answered.refused());

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
    assert!(answered.recorded(), "the control sent the operator back to the page: {:?}", answered.refused());

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

// ---- 9.11 a proposed intent's weight

/// A proposed intent cites its signals and shows how many, from which sources
/// and over what span, counted by event date (109, 118).
#[test]
fn proposed_intent_shows_weight() {
    use flywheel_domain::signals::{self, Capture, Signal};

    let (_sandbox, page) = a_page("weight");
    let defs = flywheel_domain::set::load().expect("the embedded definitions");

    // Three signals for the one intent: two from a meeting a week ago, one
    // from a chat forward yesterday.
    let sources = [
        ("meeting/2026-09-02/willdan-weekly", "meeting", "2026-09-02T10:00:00+00:00", 2),
        ("message/1421", "forwarded-message", "2026-09-08T17:30:00+00:00", 1),
    ];
    let mut cited: Vec<String> = vec![];
    page.with_world(|world| {
        for (key, source, event_at, how_many) in sources {
            signals::write_capture(
                world,
                &Capture {
                    key: key.into(),
                    source: source.into(),
                    event_at: event_at.into(),
                    captured_by: "chuck".into(),
                    raw: format!("raw://{key}"),
                },
            )
            .expect("the capture");
            for n in 1..=how_many {
                let id = signals::signal_object(key, n);
                signals::write_signal(
                    world,
                    key,
                    n,
                    &Signal {
                        id: id.clone(),
                        capture: signals::object_of(key),
                        kind: "constraint".into(),
                        asserted_by: "dana".into(),
                        assertion: "one writer per provider".into(),
                        excerpt: "one writer per provider".into(),
                        ..Default::default()
                    },
                )
                .expect("the signal");
                cited.push(id);
            }
        }
    });

    // The proposed intent that cites them (110, 109).
    let signals_cited = cited.clone();
    page.with_store(|store| {
        let at = commands::now(store).expect("a point");
        commands::put_new(
            store,
            &defs,
            "intent/retry-jitter",
            "intent",
            None,
            [
                ("signals".to_string(), json!(signals_cited)),
                ("signals_count".to_string(), json!(signals_cited.len())),
            ]
            .into_iter()
            .collect(),
            at,
        )
        .expect("the proposed intent");
    });

    // The intent's dock page, as the drawer fetches it when it is opened (310a,
    // S235).
    let html = page.html("/") + &page.html("/willdan/intent/retry-jitter?part=dock");
    // How many, from which sources, over what span — the span counted by event
    // date and never by when the flywheel read it (109).
    assert!(html.contains("data-signals=\"3\""), "the count is not shown: {html}");
    assert!(
        html.contains("data-sources=\"forwarded-message meeting\""),
        "the sources are not shown: {html}"
    );
    assert!(
        html.contains("3 signals from forwarded-message, meeting"),
        "the weight does not read as a sentence: {html}"
    );
    assert!(
        html.contains("2026-09-02T10:00:00+00:00 to 2026-09-08T17:30:00+00:00"),
        "the span is not shown: {html}"
    );
    // And it cites them, each one (109).
    for signal in &cited {
        assert!(
            html.contains(&format!("<li class=\"signal\"><a class=\"elaboration\" href=\"#dock-{signal}\">")),
            "the intent does not cite `{signal}`"
        );
    }
}

// ---- 14.1 the page is the mockup

/// The ratified design of the page, in the blueprints, read from the path this
/// test names (D16, AGENTS.md — Where the design lives).
fn mockup() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../blueprints/main/design/flywheel-next/mockups/rail-and-board.html");
    std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "the page's ratified design is `design/flywheel-next/mockups/rail-and-board.html` in \
             the blueprints, and the served page is built from it (D16). It is not at {}: {e}",
            path.display()
        )
    })
}

/// The parts phase 1 does not have, and why each one is not on the page. Every
/// other id the mockup gives is asserted below, so a region added to the design
/// fails here until the page carries it.
const NOT_THIS_PHASE: [(&str, &str); 20] = [
    // The system context map (198, accepted; not yet in requirements.md) and
    // its scope, review and add-repository flows.
    ("map", "the map view is not phase 1's"),
    ("map-wrap", "the map view"),
    ("map-stage", "the map view"),
    ("map-panel", "the map view"),
    ("arrow", "the map's edge marker"),
    ("scope-send", "the map's scope gesture"),
    ("scope-clear", "the map's scope gesture"),
    ("review-toggle", "since-last-review, on the map"),
    ("review-mark", "since-last-review, on the map"),
    ("addrepo-open", "the map's add-repository flow"),
    ("ar-name", "the map's add-repository flow"),
    ("ar-add", "the map's add-repository flow"),
    ("ar-cancel", "the map's add-repository flow"),
    // The flywheel instrument and its panel (214).
    ("load", "the flywheel instrument is not phase 1's"),
    ("load-open", "the flywheel instrument"),
    ("load-close", "the flywheel instrument"),
    // The account item and its sign-in: phase 1 has one operator, named by the
    // manifest, and no sign-in at all (D10, 236a, 253a; 233 closes it).
    // The account item's slot in the header stays, because the one operator's
    // name has to be on the page: it is what every response records as
    // `given_by` (153, 236a, 253a). What is not here is the menu behind it and
    // the sign-in, which phase 1 does not have (D10; 233 closes it).
    ("acct", "no sign-in in phase 1 (D10, 253a)"),
    ("acct-menu", "no sign-in in phase 1 (D10, 253a)"),
    ("signin", "no sign-in in phase 1 (D10, 253a)"),
    ("signedout", "no sign-in in phase 1 (D10, 253a)"),
];

/// The mockup draws the control twice — once in the header and once per approve
/// group — and the page draws it once, in the header where S2 puts it.
///
/// That `answer` takes one decision per call is what "yes all" *is*: S7 says it
/// sends one `answer` per approve decision, in number order, each recorded on
/// its own, and never a batch. So one control, fanned out server-side, with the
/// numbers it will answer on it (S2, S7, 11, 17.5). A second copy of it lower
/// down names no further action.
const YES_ALL: [&str; 1] = ["yesall2"];

/// What the mockup draws and the page does not, because it survives no action.
/// The mockup is the record of what we were picturing, not an authority to copy
/// element-by-element; before an element is carried across, the action it
/// serves is named, and one that serves none does not ship (amends D16, 17.0).
const NO_ACTION: [(&str, &str); 2] = [
    (
        "yesallhint",
        "a bare strip of every waiting decision number across the top of the page names no \
         action. It is a mis-rendering of S2, which asks for the `yes all` control with the \
         numbers it will answer: the numbers belong on the control that answers them (S2, 17.5)",
    ),
    (
        "pal-intent",
        "a toggle beside the capture field that marked the text an intent named an operation \
         no machine read, and a judgment about a capture is curation's (20, 110); what the \
         operator does with a capture is make it a unit, on the capture itself (34, 12)",
    ),
];

/// Every id the mockup gives, in the order it gives them.
///
/// The whole file is read and not its static markup alone: the mockup renders
/// the palette, the dock's header and the rail's own controls from its script,
/// so `pal-q`, `dk-title`, `dk-x` and `pal-open-rail` are ids the design gives
/// and appear nowhere but there. What a naive scan gets wrong is the other
/// direction — `id="${u.id}"` inside that script is an interpolation and not an
/// id — and that is what is skipped (17.6, D16).
fn ids_of(html: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for at in html.match_indices("id=\"") {
        let rest = &html[at.0 + 4..];
        let Some(end) = rest.find('"') else { continue };
        let id = &rest[..end];
        // A template's own interpolation is not an id the design gives.
        if id.contains("${") || id.is_empty() {
            continue;
        }
        if !out.iter().any(|held| held == id) {
            out.push(id.to_string());
        }
    }
    out
}

/// The page is rendered from the mockup's markup and styles for the parts phase
/// 1 has — the rail, the capture box, the board, the status view and the dock —
/// and every element the mockup gives an id in those regions is present under
/// that id (D16, 14.1).
#[test]
fn page_carries_the_mockups_regions() {
    let (_sandbox, page) = a_page("the-mockups-regions");
    // A decision stands, so the rail's own answerable form is on the page.
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
    // The drawer holds the dock page of the object opened, as it is fetched
    // when the object is opened (310a, S235).
    let html = page.html("/").replacen(
        "<div class=\"dk-b\" id=\"dk-b\"></div>",
        &format!("<div class=\"dk-b\" id=\"dk-b\">{}</div>", page.html("/willdan/bolt/atlas/plan-rows?part=dock")),
        1,
    );
    let design = mockup();
    let style = page.html(&flywheel_surface::page::style_address());

    let mut missing: Vec<String> = Vec::new();
    for id in ids_of(&design) {
        if let Some((_, why)) = NOT_THIS_PHASE.iter().find(|(name, _)| *name == id) {
            assert!(
                !html.contains(&format!("id=\"{id}\"")),
                "`{id}` is on the page, and it is named as a part phase 1 does not have: {why}"
            );
            continue;
        }
        if let Some((_, why)) = NO_ACTION.iter().find(|(name, _)| *name == id) {
            assert!(
                !html.contains(&format!("id=\"{id}\"")),
                "`{id}` is on the page, and it is named as an element that serves no action: {why}"
            );
            continue;
        }
        if YES_ALL.contains(&id.as_str()) {
            continue;
        }
        if !html.contains(&format!("id=\"{id}\"")) {
            missing.push(id);
        }
    }
    assert!(
        missing.is_empty(),
        "the mockup gives these ids and the served page carries none of them (D16): {}",
        missing.join(", ")
    );

    // The regions themselves, in the mockup's own silhouettes, and each one
    // with something in it.
    //
    // This assertion used to be `html.contains(id)` and it passed over a board
    // reading "nothing" in every column for a day, because an id is present on
    // an empty region and a structural test cannot tell the difference. What is
    // asserted now is the region's own content: the words a person would read
    // off it, with the markup taken out (17.4, D16).
    for (region, least) in [
        ("rail", 200),
        ("lane-inception", 120),
        ("lane-construction", 120),
        ("dk-b", 200),
        ("pal", 40),
    ] {
        let read = words_in(&html, region);
        assert!(
            read.chars().count() >= least,
            "`{region}` is on the page and there is nothing in it — {} characters, and a \
             region asserted present must carry its content (17.4): {read:?}",
            read.chars().count()
        );
    }

    // And in the mockup's own silhouettes: a decision is the only answerable
    // card, an intent is a thread with its elaborations as beads, a bolt is a
    // ledger with its units in a chain, and every one of them carries the
    // mockup's inner elements rather than one body under a different class
    // (209, S13, S15, D16).
    for (form, inner) in [
        ("card decision", &["class=\"ch\"", "class=\"title\"", "class=\"answers\""][..]),
        ("thread", &["class=\"th-head\"", "class=\"th-k\"", "class=\"th-line\""][..]),
        ("ledger", &["class=\"lg-head\"", "class=\"lg-chain\""][..]),
        ("dk-h ", &["<h2>", "class=\"kind\""][..]),
    ] {
        assert!(
            html.contains(&format!("class=\"{form}")),
            "the mockup's `{form}` is not on the page"
        );
        let drawn = html
            .split(&format!("class=\"{form}"))
            .nth(1)
            .expect("the silhouette is on the page");
        for element in inner {
            assert!(
                drawn.contains(element),
                "a `{form}` on the page carries no `{element}`; a form that differs only by \
                 the word on its class is one card with variants, which is what 209 refuses"
            );
        }
    }

    // The answer controls the mockup draws on a card, as controls: one tap
    // each, posting to the one tool the reply grammar calls (311, 193, 194).
    let numbered = page.with_store(|store| commands::register(store).expect("the register"));
    let number = numbered
        .entries
        .iter()
        .find(|(id, _)| id.starts_with("bolt/atlas/plan-rows"))
        .expect("the register numbered it")
        .1
        .number;
    let card = html
        .split("<article class=\"card decision")
        .nth(1)
        .expect("a decision card is on the rail");
    let card = card.split("</article>").next().unwrap_or_default();
    assert!(
        card.contains(&format!("<span class=\"n number\">{number}</span>")),
        "the card carries the number the register gave, in the mockup's own place for it: {card}"
    );
    assert!(
        card.contains(&format!("action=\"/api/tools/{}\"", flywheel_surface::catalogue::ANSWER)),
        "the card's answers post to the catalogue: {card}"
    );
    assert!(card.contains("data-answer=\""), "the answers are controls: {card}");

    // The mockup's own stylesheet, carried rather than paraphrased: its tokens
    // and its card, lane, dock and palette rules are the page's (D16).
    for rule in [
        "--accent:#0B6E79;",
        ".card{position:relative;border:1px solid var(--line);border-left-width:3px",
        ".lanes{flex:1;min-height:0;display:grid;",
        ".dk-b{flex:1;overflow:auto;",
        ".pal{width:min(680px,100%);",
    ] {
        assert!(
            design.contains(rule),
            "the mockup no longer carries `{rule}`; the page's stylesheet is copied from it (D16)"
        );
        assert!(style.contains(rule), "the page does not carry the mockup's `{rule}`");
    }

    // And still one bundle with nothing fetched from anywhere but its host: the
    // design is carried, its JavaScript is not — the page's one script is its
    // keys, and it and the stylesheet are at the host's versioned addresses
    // (310, 311, 310a, S235).
    assert_eq!(html.matches("rel=\"stylesheet\"").count(), 1);
    assert!(html.contains(&format!("<link rel=\"stylesheet\" href=\"{}\">", flywheel_surface::page::style_address())));
    assert_eq!(html.matches("<script").count(), 1);
    assert!(html.contains(&format!("<script src=\"{}\"></script>", flywheel_surface::page::script_address())));
}

/// The words a region of the rendered page carries, with its markup taken out.
///
/// A region is found by its id where it has one and by its class otherwise,
/// and read to the end of the document — which is enough to tell a region with
/// something in it from one with nothing, and is what `contains(id)` could not.
fn words_in(html: &str, region: &str) -> String {
    let at = html
        .find(&format!("id=\"{region}\""))
        .or_else(|| html.find(&format!("class=\"{region}\"")))
        .or_else(|| html.find(&format!("class=\"{region} ")))
        .unwrap_or_else(|| panic!("the page carries no region `{region}`"));
    let rest = &html[at..];
    // Every element of the region, to a depth that covers a lane and its
    // objects; what is read is the text between the tags.
    let mut read = String::new();
    let mut inside = false;
    for byte in rest.chars() {
        match byte {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => read.push(byte),
            _ => {}
        }
        if read.chars().count() > 4000 {
            break;
        }
    }
    read.split_whitespace().collect::<Vec<_>>().join(" ")
}
