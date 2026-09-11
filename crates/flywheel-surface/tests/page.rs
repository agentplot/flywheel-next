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
    // Full screen when an object is open, and not before: a dock covering the
    // rail would put every decision behind it, which is the one thing the phone
    // must not do (307, 311).
    assert!(
        phone.contains(".dock:not(:has(.surface:target)) { display: none; }"),
        "the dock covers the rail only once an object is opened"
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

    let html = page.html("/");
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
            html.contains(&format!("<li class=\"signal\">{signal}</li>")),
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

/// `yes all` is the chat's numbered reply grammar, which expands into one
/// response per decision before any tool is called (8.3, 11, 194). The page's
/// one write path is the catalogue, whose `answer` takes one decision per call
/// (193), so the mockup's two yes-all controls and their hint have no call to
/// make here and are not drawn. Answering each decision on its own is what the
/// rail offers, and that is what 11 asks any one answer to do.
const YES_ALL: [&str; 2] = ["yesall", "yesall2"];

/// Every id the mockup gives, in the order it gives them.
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
    let html = page.html("/");
    let design = mockup();

    let mut missing: Vec<String> = Vec::new();
    for id in ids_of(&design) {
        if let Some((_, why)) = NOT_THIS_PHASE.iter().find(|(name, _)| *name == id) {
            assert!(
                !html.contains(&format!("id=\"{id}\"")),
                "`{id}` is on the page, and it is named as a part phase 1 does not have: {why}"
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

    // The regions themselves, in the mockup's own silhouettes: the rail's
    // cards, the board's lanes, the dock's surfaces and the palette (D16).
    for present in [
        "class=\"rail\"",
        "class=\"card decision",
        "class=\"lanes\"",
        "class=\"lane inception\"",
        "class=\"pal\"",
        "class=\"dk-b\"",
    ] {
        assert!(html.contains(present), "the mockup's `{present}` is not on the page");
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
        assert!(html.contains(rule), "the page does not carry the mockup's `{rule}`");
    }

    // And still one bundle with no script and nothing fetched from anywhere
    // else: the design is carried, its JavaScript is not (310).
    assert_eq!(html.matches("<style>").count(), 1);
    assert!(!html.contains("<script"));
}
