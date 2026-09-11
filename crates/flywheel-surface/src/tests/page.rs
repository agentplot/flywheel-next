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

/// What the header carries, and the work each element does. Nothing stands here
/// that the operator does not act on — and an element that explains why a
/// control is absent is doing work too, because it answers a question the
/// operator would otherwise act on.
///
/// This is the list the requirements justify, not the list the mockup draws.
/// What the mockup draws and this does not is named below, with the reason.
const HEADER: [(&str, &str); 8] = [
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
    (
        "theme",
        "why there is no control here: the page follows the system's theme, because a \
         switch would be client state a reload loses, and saying so answers the question \
         instead of leaving the operator to hunt for one (310)",
    ),
];

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

// ---------------------------------------------------------------- the rail

/// The scenario the page's own walkthrough stands on, read as data.
///
/// `scenarios/rail-mockup.yaml` holds every decision kind once, so the rail it
/// raises is the set of forms a card has to have. The file's types are
/// `flywheel-atoms`', so this reads it without the runner (94).
fn rail_mockup(store: &mut FakeStore, defs: &Definitions) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scenarios/rail-mockup.yaml");
    let scenario = flywheel_atoms::scenario::load(&path).expect("the rail mockup's scenario");
    let at = commands::now(store).expect("a point");
    // What the world reports, which is what the guards read (B.3).
    for (name, per_object) in &scenario.given.evidence {
        for (object, value) in per_object {
            store.given(object, name, value.clone());
        }
    }
    for given in &scenario.given.objects {
        commands::put_new(
            store,
            defs,
            &given.id,
            &given.machine,
            given.parent.as_deref(),
            given.record.clone(),
            at,
        )
        .unwrap_or_else(|e| panic!("{}: {e}", given.id));
        if given.state.is_empty() {
            continue;
        }
        let mut object = Records::get(store, &given.id).expect("a read").expect("the object");
        // The top-level state first, so a nested one is not cleared by the
        // region above it being set after it.
        let mut paths: Vec<(&String, &String)> = given.state.iter().collect();
        paths.sort_by_key(|(region, _)| region.matches('.').count());
        for (region, state) in paths {
            let under = format!("{region}.");
            let gone: Vec<String> = object
                .config
                .keys()
                .filter(|held| held.starts_with(&under))
                .cloned()
                .collect();
            for key in gone {
                object.config.remove(&key);
            }
            object.config.insert(region.clone(), state.clone());
            object.entered_at.insert(region.clone(), at);
        }
        flywheel_engine::initialise(defs, &mut object, at);
        let base = object.seq;
        Records::put(store, &given.id, &object, base).expect("the described state");
    }
    commands::rail(store, defs).expect("the rail derives");
}

/// Every card on the rail carries what its kind is, where it is, why it is
/// being asked, and the answers that kind takes — each one a control
/// (15, 11, 18, 209, 210, S7, 311).
#[test]
fn a_rail_card_carries_its_kind_controls() {
    let (mut store, world, defs) = a_page();
    rail_mockup(&mut store, &defs);
    let read = crate::page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("the page");
    let html = crate::page::render(&read);
    assert!(
        read.decisions.len() >= 6,
        "the rail mockup raises every decision kind once: {}",
        read.decisions.len()
    );

    let mut kinds: Vec<String> = Vec::new();
    for decision in &read.decisions {
        let number = decision.number.expect("the register numbered it");
        let card = html
            .split("<article ")
            .find(|block| block.contains(&format!("data-number=\"{number}\"")))
            .unwrap_or_else(|| panic!("decision {number} has no card"));
        kinds.push(decision.kind.clone());

        // What it is: the object's own machine, not the group it is filed
        // under, which is the heading above it (209).
        let machine = read
            .objects
            .iter()
            .find(|o| o.id == decision.object)
            .map(|o| o.machine.clone())
            .expect("the decision's object");
        assert!(
            card.contains(&format!("<span class=\"kind\">{machine}</span>")),
            "decision {number} does not say it is a {machine}: {card}"
        );
        // Where it is: the lane it sits in on the board (D16).
        assert!(
            card.contains("class=\"ph\" data-phase=\""),
            "decision {number} carries no phase: {card}"
        );
        // The object it concerns, as a link that opens it (308).
        assert!(
            card.contains(&format!("class=\"object\" href=\"{ADDRESS}/{}\"", decision.object)),
            "decision {number} does not link its object: {card}"
        );
        // And its own answers, each one a control that posts (311, 193).
        assert!(!decision.answers.is_empty(), "decision {number} takes no answer");
        for answer in &decision.answers {
            assert!(
                card.contains(&format!("data-answer=\"{}\"", crate::page::escape(answer))),
                "decision {number} carries no `{answer}` control: {card}"
            );
        }
    }

    // The kinds the mockup draws a form for, each with the answers its machine
    // names — which is what makes one kind's card a different thing from
    // another's rather than one card with variants (209).
    let card_for = |kind: &str| -> &crate::page::Read {
        assert!(kinds.iter().any(|held| held == kind), "no `{kind}` on the rail: {kinds:?}");
        &read
    };
    for (kind, answers) in [
        ("intent-proposed", &["yes", "drop", "split"][..]),
        ("unit-proposed", &["yes", "drop", "bolt <name>", "new bolt <name>"][..]),
        ("elaboration-proposed", &["yes", "drop", "type <name>"][..]),
    ] {
        let read = card_for(kind);
        let decision = read
            .decisions
            .iter()
            .find(|d| d.kind == kind)
            .expect("the kind is on the rail");
        for answer in answers {
            assert!(
                decision.answers.iter().any(|held| held == answer),
                "a `{kind}` takes `{answer}` and the card offers {:?}",
                decision.answers
            );
        }
    }

    // The one-line why: what the machine's own `shows:` names is on the card,
    // so a decision says what it is about and not only what it is (15, 11).
    let intent = read
        .decisions
        .iter()
        .find(|d| d.kind == "intent-proposed")
        .expect("the intent's proposal");
    let why = read.why.get(&intent.id).expect("the intent's card says why");
    assert!(
        why.iter().any(|said| said.contains("signals")),
        "the intent cites signals and its card does not say how many: {why:?}"
    );
}
