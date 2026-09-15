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
const HEADER: [(&str, &str); 11] = [
    (
        "capture-text",
        "the capture box's field: typed into and sent with return, reached with / or ⌘K \
         (19, 194, 311)",
    ),
    (
        "pal-drop",
        "the same box opened under itself while it has the cursor: what was sent lately, \
         what a `/` or a number lists, what will be sent, and the keys (19, 311, S63)",
    ),
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
    (
        "yesall",
        "the one thing that makes a ten-decision rail workable: yes to the approve group, \
         one response per decision and never a batch, with the numbers it will answer on \
         the control itself so the operator reads what the tap does before making it \
         (11, S2, S7)",
    ),
];

/// What the mockup puts in the header and the page does not, with what each one
/// failed to name.
const NO_ACTION: [(&str, &str); 1] = [(
    "yesallhint",
    "a bare strip of every waiting decision number across the top names no action. It is \
     a mis-rendering of S2, which asks for the `yes all` control with the numbers it will \
     answer: the numbers belong on the control that answers them, and that is where \
     `yesall` carries them (S2, 17.5)",
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

/// Every host stands in the hosts strip as a chip with a dot for whether it
/// is heard from, and a host past its stale window says so on the chip: work
/// it holds is not moving, and that is in the operator's way (141, 143, 146,
/// 79, 150a).
#[test]
fn every_host_is_a_chip_and_a_stale_one_says_so() {
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
        let start = html.find("id=\"hosts\"").expect("the hosts strip");
        let rest = &html[start..];
        rest[..rest.find("</div>").expect("it closes")].to_string()
    };
    assert!(
        strip.contains("data-host=\"studio\"") && strip.contains("data-liveness=\"alive\""),
        "a well host stands as a chip, alive: {strip}"
    );
    assert!(!strip.contains("class=\"host gone\""), "and is not raised as wrong: {strip}");

    // Six minutes on it is past its stale window, and that the operator does
    // need in their way: work it holds is not moving (150a, 79).
    store.set_now(at + chrono::Duration::minutes(6));
    let html = rendered(&mut store, &world, &defs);
    let strip = {
        let start = html.find("id=\"hosts\"").expect("the hosts strip");
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

/// A response whose decision was gone before it arrived: one attention line,
/// reported and never dropped (6, 129, `engine/response.yaml`).
fn an_attention_line(store: &mut FakeStore, defs: &Definitions) {
    let at = commands::now(store).expect("a point");
    commands::put_new(
        store,
        defs,
        "response/lost-1",
        "response",
        None,
        [("answer".to_string(), json!("yes"))].into_iter().collect(),
        at,
    )
    .expect("the response object");
    let mut held = Records::get(store, "response/lost-1").expect("a read").expect("it");
    held.config.insert("life".into(), "unapplicable".into());
    let base = held.seq;
    Records::put(store, "response/lost-1", &held, base).expect("the response, unapplicable");
    commands::rail(store, defs).expect("the rail derives");
}

/// The rail walks the groups in the model's order, each sorted by number, and
/// names each group once (11, S3).
///
/// `derive` hands the decisions back in number order, because the number is
/// what a response names; the grouping is the reader's. A reader that walked
/// the flat list and printed a heading whenever the group changed gave the
/// operator `approve, decide, approve, decide, attention` with the headings
/// repeating down the page — and then "yes to all" has no group to mean,
/// against 11.
#[test]
fn the_rail_is_grouped_and_each_group_is_named_once() {
    let (mut store, world, defs) = a_page();
    rail_mockup(&mut store, &defs);
    an_attention_line(&mut store, &defs);
    let html = rendered(&mut store, &world, &defs);
    let rail = {
        let start = html.find("id=\"rail\"").expect("the rail");
        let rest = &html[start..];
        rest[..rest.find("</aside>").expect("the rail closes")].to_string()
    };

    // The headings, in the order they appear, and the numbers under each.
    let mut headings: Vec<String> = Vec::new();
    let mut under: Vec<(String, u32)> = Vec::new();
    for piece in rail.split("<div class=\"grp ").skip(1) {
        let group = piece.split('"').next().unwrap_or_default().to_string();
        for card in piece.split("data-number=\"").skip(1) {
            if let Ok(number) = card.split('"').next().unwrap_or_default().parse::<u32>() {
                under.push((group.clone(), number));
            }
        }
        headings.push(group);
    }
    assert!(headings.len() >= 3, "the mockup's rail has several groups: {headings:?}");

    // Each group named once.
    let mut seen: Vec<&String> = Vec::new();
    for group in &headings {
        assert!(
            !seen.contains(&group),
            "`{group}` heads the rail more than once, so the rail is not grouped: {headings:?}"
        );
        seen.push(group);
    }
    // In the model's order, with attention last. `since` is the mockup's own
    // list of what finished lately, under the decisions, and no group of the
    // count (14, S59).
    let order: Vec<usize> = headings
        .iter()
        .filter(|group| group.as_str() != "since")
        .map(|group| {
            flywheel_engine::rail::GROUPS
                .iter()
                .position(|named| named == group)
                .unwrap_or_else(|| panic!("`{group}` is no group the model names"))
        })
        .collect();
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "the groups are out of the model's order {:?}: {headings:?}",
        flywheel_engine::rail::GROUPS
    );
    // And each group sorted by number.
    for group in &headings {
        let numbers: Vec<u32> = under
            .iter()
            .filter(|(held, _)| held == group)
            .map(|(_, number)| *number)
            .collect();
        assert!(
            numbers.windows(2).all(|pair| pair[0] < pair[1]),
            "`{group}` is not in number order: {numbers:?}"
        );
    }

    // Attention stands outside the count: it is what the machinery could not
    // do, reported and never dropped, not a choice being put to the operator
    // (S3, S8, 6).
    let read = crate::page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("a read");
    let attention = read.decisions.iter().filter(|d| d.group == "attention").count();
    assert!(attention > 0, "the mockup's rail carries an attention line");
    let counted = html
        .split("id=\"count\">")
        .nth(1)
        .and_then(|rest| rest.split('<').next())
        .and_then(|n| n.parse::<usize>().ok())
        .expect("the header says how many stand");
    assert_eq!(
        counted,
        read.decisions.len() - attention,
        "the header counts the attention lines among the decisions standing (S3)"
    );
}

/// The "yes all" control carries the numbers it will answer, and they are the
/// approve group's and nothing else (11, S2, S7).
#[test]
fn yes_all_names_the_approve_group_and_nothing_else() {
    let (mut store, world, defs) = a_page();
    rail_mockup(&mut store, &defs);
    let read = crate::page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("a read");
    let html = crate::page::render(&read);

    let answering = crate::page::yes_all_answers(&read.decisions);
    assert!(answering.len() > 1, "the mockup's rail has an approve group to sweep");
    // Only decisions that actually offer `yes`, and only from approve: a
    // decide, an answer or an attention line is not something a sweep of the
    // hand settles (11).
    for (number, object) in &answering {
        let decision = read
            .decisions
            .iter()
            .find(|d| d.number == Some(*number))
            .expect("the decision it names");
        assert_eq!(decision.group, "approve", "{object} is not in the approve group");
        assert!(decision.answers.iter().any(|a| a == "yes"), "{object} takes no yes");
    }
    for decision in read.decisions.iter().filter(|d| d.group != "approve") {
        assert!(
            !answering.iter().any(|(number, _)| Some(*number) == decision.number),
            "`yes all` would answer {} of the {} group",
            decision.number.unwrap_or_default(),
            decision.group
        );
    }

    // The control, with the numbers on it: S2 asks for the control with the
    // numbers it will answer, and a strip of them loose in the header instead
    // names no action.
    let control = html
        .split("<button")
        .find(|block| block.contains("id=\"yesall\""))
        .expect("the header carries no `yes all` control (S2)");
    let control = &control[..control.find("</button>").expect("it closes")];
    assert!(
        control.contains("type=\"submit\""),
        "`yes all` is not a control that posts: {control}"
    );
    for (number, _) in &answering {
        assert!(
            control.contains(&number.to_string()),
            "`yes all` does not say it will answer {number}: {control}"
        );
    }
    // And the form posts to the one place that expands it into one `answer`
    // per decision (S7, 193).
    assert!(
        html.contains("action=\"/api/answer-all\""),
        "the control posts nowhere"
    );
}

/// A link the machinery wrote opens that object in the dock, at every viewport
/// (308, 307, 205a).
///
/// The link names its object in the path and carries no fragment, so nothing
/// is targeted when it is fetched. The page marked the surface opened and the
/// stylesheet keyed on nothing but `:target`, so the link landed the operator
/// on the rail with the dock shut — and on a phone the dock sits under the
/// board panel, which a page with no fragment does not even show.
#[test]
fn a_link_opens_its_object_in_the_dock_with_no_fragment() {
    let (mut store, world, defs) = a_page();
    rail_mockup(&mut store, &defs);
    let mut read = crate::page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("a read");
    let object = "intent/atlas-provider-limits".to_string();
    assert!(read.objects.iter().any(|o| o.id == object), "the object is in the read");
    read.opened = Some(object.clone());
    let html = crate::page::render(&read);

    let surface = html
        .split("<article ")
        .find(|block| block.contains(&format!("id=\"dock-{object}\"")))
        .expect("the dock carries no surface for the object the link named");
    assert!(
        surface.contains("data-opened=\"true\""),
        "the surface the link named is not marked opened: {surface}"
    );
    // And exactly one: a link opens the object it names and nothing else. The
    // stylesheet names the attribute too, so the count is of the document.
    let body = html.split("</style>").nth(1).expect("the document after its stylesheet");
    assert_eq!(
        body.matches("data-opened=\"true\"").count(),
        1,
        "more than one surface is opened"
    );

    // The stylesheet has to act on it. The page runs no script, so what opens
    // the dock is a rule; a document that marks the surface and no rule that
    // shows it is the link opening nothing (310, 311).
    let style = html
        .split("<style>")
        .nth(1)
        .and_then(|rest| rest.split("</style>").next())
        .expect("the page carries its own stylesheet");
    assert!(
        style.contains(".dock .surface[data-opened=\"true\"]"),
        "nothing in the stylesheet shows the surface the request opened"
    );
    assert!(
        style.contains("#board") && style.contains("data-opened=\"true\"") ,
        "nothing shows the board panel the dock lives under, so the link opens nothing \
         under 760px (307, 308)"
    );

    // And it opens with its answer controls in reach (308). On a phone the dock
    // is the whole screen, so a footer that only said answering happens on the
    // rail sent the operator back to hunt for the card the link had just
    // brought them to.
    let decision = read
        .decisions
        .iter()
        .find(|d| d.object == object)
        .expect("a decision stands on the object the link named");
    let number = decision.number.expect("it is numbered");
    let foot = surface
        .split("<div class=\"dk-answers\"")
        .nth(1)
        .expect("the surface carries no answers for the decision standing on it (S27, 308)");
    let foot = &foot[..foot.find("</article>").unwrap_or(foot.len())];
    assert!(
        foot.contains(&format!("data-number=\"{number}\"")),
        "the dock does not say which decision it is answering: {foot}"
    );
    for answer in &decision.answers {
        assert!(
            foot.contains(&format!("data-answer=\"{}\"", crate::page::escape(answer))),
            "the dock carries no `{answer}` control: {foot}"
        );
    }

    // An object with nothing standing on it says so, and why (S27).
    let settled = read
        .objects
        .iter()
        .find(|o| !read.decisions.iter().any(|d| d.object == o.id))
        .expect("something on this instance has nothing standing on it");
    let quiet = html
        .split("<article ")
        .find(|block| block.contains(&format!("id=\"dock-{}\"", settled.id)))
        .expect("it has a surface");
    let quiet = &quiet[..quiet.find("</article>").unwrap_or(quiet.len())];
    assert!(
        quiet.contains("dk-answers none"),
        "`{}` has nothing to answer and does not say so: {quiet}",
        settled.id
    );
}

/// An answer that takes an argument takes it on the card, and what is recorded
/// is the string the machine's pattern matches (S6, 311, 193).
///
/// An answer that takes an argument — `redo: <notes>`, `bolt <name>`, `type
/// <name>` — is one tap on the card, opening the object in the dock, where
/// the field for it is (311, S6, D16). On a unit's card that is six of nine
/// controls: with a text field for each the card read as a form and not as a
/// decision, and the mockup keeps the card to its words. Rendered as the
/// button's value the pattern posted `redo: <notes>` literally, so the field
/// is where the operator says what to redo, what bolt to route to or what
/// type to set.
#[test]
fn an_answer_that_takes_an_argument_takes_it_in_the_dock() {
    let (mut store, world, defs) = a_page();
    rail_mockup(&mut store, &defs);
    let read = crate::page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("a read");
    let html = crate::page::render(&read);

    let unit = read
        .decisions
        .iter()
        .find(|d| d.kind == "unit-proposed")
        .expect("the mockup proposes a unit");
    let number = unit.number.expect("it is numbered");
    let card = html
        .split("<article ")
        .find(|block| block.contains(&format!("data-number=\"{number}\"")))
        .expect("its card");
    let card = &card[..card.find("</article>").expect("the card closes")];
    let dock = html
        .split("<article ")
        .find(|block| block.starts_with(&format!("class=\"surface form-unit\" id=\"dock-{}\"", unit.object)))
        .expect("the unit's dock surface");
    let dock = &dock[..dock.find("</article>").expect("the surface closes")];

    let taking: Vec<&String> = unit.answers.iter().filter(|a| a.contains('<')).collect();
    assert!(
        taking.len() >= 3,
        "a unit takes several answers with an argument: {:?}",
        unit.answers
    );
    for answer in &taking {
        let escaped = crate::page::escape(answer);
        // On the card: one tap to the dock, and no field (311).
        let tap = card
            .split("<a ")
            .find(|block| block.contains(&format!("data-answer=\"{escaped}\"")))
            .unwrap_or_else(|| panic!("no control for `{answer}` on the card"));
        let tap = &tap[..tap.find("</a>").expect("the link closes")];
        assert!(
            tap.contains(&format!("href=\"#dock-{}\"", unit.object)),
            "`{answer}` on the card opens somewhere other than its object: {tap}"
        );
        let label = tap.rsplit_once('>').map(|(_, said)| said).unwrap_or_default();
        assert!(!label.contains("&lt;"), "`{answer}` wears its own placeholder as its label: {label}");
        assert!(!card.contains(&format!("<input type=\"hidden\" name=\"answer\" value=\"{escaped}\"")), "the card carries a field for `{answer}`");

        // In the dock: the form with somewhere to type, and nothing sent empty.
        let form = dock
            .split("<form ")
            .find(|block| block.contains(&format!("data-answer=\"{escaped}\"")))
            .unwrap_or_else(|| panic!("no control for `{answer}` in the dock"));
        let form = &form[..form.find("</form>").expect("the control closes")];
        assert!(
            form.contains("name=\"text\"") && form.contains("type=\"text\""),
            "`{answer}` is a control with nowhere to type its argument: {form}"
        );
        assert!(form.contains("required"), "`{answer}` would send an empty argument as an answer: {form}");
    }
    // And a bare answer stays one tap on the card, with nothing to fill in (311).
    let bare = card
        .split("<form ")
        .find(|block| block.contains("data-answer=\"yes\""))
        .expect("the card takes a yes");
    let bare = &bare[..bare.find("</form>").expect("it closes")];
    assert!(!bare.contains("type=\"text\""), "a yes asks for something to type: {bare}");
}

/// The curator's surface stands while the curation is running and goes when it
/// is not (110, `curation.yaml`).
///
/// It was read off the session fact carrying no `ended_at`, which is not a
/// reading of anything: a session that reports an exit reaches `exited`, and
/// `end_session` — the only writer of `ended_at` — runs from `ended` alone,
/// which the owner's explicit end reaches (`session.yaml`). So the box stayed on
/// the page after the curation closed, offering the operator moves over signals
/// whose moves were already recorded, and a submit on it would have delivered a
/// second exit for a session that had already reported one.
#[test]
fn the_curators_surface_goes_when_the_curation_does() {
    let mut store = FakeStore::default();
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let at = commands::now(&store).expect("a point");
    commands::put_new(
        &mut store,
        &defs,
        "curation/willdan",
        "curation",
        None,
        [("threshold".to_string(), json!(2))].into_iter().collect(),
        at,
    )
    .unwrap();
    // The session the run charged, as the operator's runner records it.
    let session = flywheel_engine::Object {
        id: "fact/session/curation/willdan/main/1".into(),
        machine: "fact".into(),
        parent: None,
        config: Default::default(),
        entered_at: Default::default(),
        record: [
            ("started_at".to_string(), json!(at.to_rfc3339())),
            ("ended_at".to_string(), serde_json::Value::Null),
            ("place".to_string(), json!("curation/willdan")),
        ]
        .into_iter()
        .collect(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    };
    store.put(&session.id, &session, 0).unwrap();

    let running = |store: &FakeStore, state: &str| -> Option<String> {
        let mut curation = Records::get(store, "curation/willdan").unwrap().unwrap();
        curation.config.insert("run".into(), state.into());
        let objects: Vec<flywheel_engine::Object> = vec![curation, session.clone()];
        crate::page::curating(&objects)
    };

    assert_eq!(
        running(&store, "running").as_deref(),
        Some("curation/willdan/main/1"),
        "the operator is being asked, so the surface names the session to deliver under"
    );
    // The exit was reported: the run applied the moves and went idle. The
    // session fact still carries no `ended_at`, and there is nothing to curate.
    assert_eq!(running(&store, "applying"), None);
    assert_eq!(
        running(&store, "idle"),
        None,
        "the curator's surface outlived the curation it belonged to"
    );
}

/// A capture raises no decision: typed on the page it leaves the rail's count
/// as it was, and the newest line of what happened lately says it was
/// captured, in its own words (19a, S9, S212, S224).
#[test]
fn a_capture_raises_no_decision() {
    let (mut store, mut world, defs) = a_page();
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");
    let count = |store: &mut FakeStore, world: &world::Files| {
        crate::page::read(store, world, &defs, ADDRESS, "chuck")
            .expect("the page reads")
            .decisions
            .len()
    };
    let before = count(&mut store, &world);
    assert_eq!(before, 1, "the bolt's close stands");

    let call = crate::catalogue::Call::new("capture", "chuck", "page")
        .arg("text", json!("the rows lose their numbers on the second page"))
        .arg("source", json!("console"));
    crate::catalogue::call(&mut store, &mut world, &defs, &call).expect("the capture is taken");
    commands::rail(&mut store, &defs).expect("the rail derives");
    assert_eq!(count(&mut store, &world), before, "a capture put a decision on the rail (19a)");
    // It stands in inception as a quote, waiting on curation and not on the
    // operator (19a, 110).
    let read = crate::page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("the page reads");
    let row = read
        .status
        .rows
        .iter()
        .find(|row| row.machine == "capture")
        .expect("the capture is on the board");
    assert_eq!(row.group, "queued", "a capture is grouped as waiting on the operator (19a)");

    let html = rendered(&mut store, &world, &defs);
    let listed = html
        .split("<ul class=\"since\">")
        .nth(1)
        .expect("what happened lately is listed");
    let newest = listed.split("</li>").next().expect("a line");
    assert!(newest.contains("class=\"v captured\">captured<"), "the newest line: {newest}");
    assert!(newest.contains("the rows lose their numbers"), "the newest line: {newest}");
}

/// A proposal whose type the instance has no definition of says so on its
/// card, in place of its type, with what to do about it; one whose type is
/// defined says its type (85a).
#[test]
fn an_undefined_type_is_named_on_its_card() {
    let (mut store, world, defs) = a_page();
    let at = commands::now(&store).expect("a point");
    for (id, kind) in [("unit/atlas/spike", "spike"), ("unit/atlas/rows", "chore")] {
        let record = [("repository".to_string(), json!("atlas")), ("type".to_string(), json!(kind))];
        commands::put_new(&mut store, &defs, id, "unit", None, record.into_iter().collect(), at).expect("the unit");
    }
    commands::rail(&mut store, &defs).expect("the rail derives");
    let read = crate::page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("the page reads");
    let why = |object: &str| -> Vec<String> {
        let decision = read
            .decisions
            .iter()
            .find(|d| d.object == object)
            .unwrap_or_else(|| panic!("{object} stands on no decision"));
        read.why.get(&decision.id).cloned().unwrap_or_default()
    };
    let undefined = why("unit/atlas/spike");
    assert!(undefined.iter().any(|l| l == "spike is not a type here · set one with type…"), "{undefined:?}");
    assert!(!undefined.iter().any(|l| l == "type spike"), "the type line stands beside it: {undefined:?}");
    let defined = why("unit/atlas/rows");
    assert!(defined.iter().any(|l| l == "type chore"), "{defined:?}");
    assert!(!defined.iter().any(|l| l.contains("not a type")), "{defined:?}");
}

// ---- 17.4 the board draws each kind in its own form

/// Every kind on the board has one form of its own, and no two share one (209).
///
/// The board used to write one body for every kind — a heading, a state line, a
/// holder line and an "open" link — under an article whose class was the
/// silhouette's name. That is one card with variants wearing six names, which is
/// the one thing 209 refuses, and it is why the board read as a list of strings
/// where the design draws threads and ledgers. What is asserted here is the
/// mockup's own inner elements per form.
#[test]
fn each_kind_on_the_board_is_drawn_in_its_own_form() {
    let (mut store, world, defs) = a_page();
    rail_mockup(&mut store, &defs);
    let html = rendered(&mut store, &world, &defs);

    // An intent is a thread with its elaborations as beads, in order (S13).
    let thread = html
        .split("<article class=\"thread\"")
        .find(|block| block.contains("data-machine=\"intent\""))
        .expect("an intent is on the board");
    let thread = thread.split("</article>").next().unwrap_or_default();
    for element in ["class=\"th-head\"", "class=\"th-k\"", "class=\"th-name\"", "class=\"th-line\""] {
        assert!(thread.contains(element), "an intent's thread carries no `{element}`: {thread}");
    }
    assert!(
        thread.contains("class=\"bead") && thread.contains("data-machine=\"elaboration\""),
        "an intent's elaborations are not beads on its thread (209, S13): {thread}"
    );

    // A bolt is a ledger with its units as a left-to-right chain (S15).
    let ledger = html
        .split("<article class=\"ledger\"")
        .nth(1)
        .expect("a bolt is on the board")
        .split("</article>")
        .next()
        .unwrap_or_default();
    for element in ["class=\"lg-head\"", "class=\"lg-chain\"", "class=\"lg-unit"] {
        assert!(ledger.contains(element), "a bolt's ledger carries no `{element}`: {ledger}");
    }
    assert!(
        ledger.contains("class=\"lg-unit merged\"") || ledger.contains("class=\"lg-unit building\""),
        "the chain does not say which units are merged, building or queued (S15): {ledger}"
    );

    // And no form is the generic body the board used to write for every kind.
    assert!(
        !html.contains("<p class=\"state\">") && !html.contains("<p class=\"holder\">"),
        "the board still writes one generic state and holder line for every kind (209)"
    );
}

/// A child is drawn inside its parent and is not also a row of its own: the
/// board would otherwise say the same thing twice, and the thread would be a
/// name with nothing hanging off it (209, S13, S15).
#[test]
fn a_child_is_drawn_inside_its_parent_and_not_beside_it() {
    let (mut store, world, defs) = a_page();
    rail_mockup(&mut store, &defs);
    let html = rendered(&mut store, &world, &defs);

    // The elaborations of an intent that is on the board are beads on it and
    // appear nowhere else on the board.
    let board = html.split("class=\"lanes\"").nth(1).expect("the board");
    let board = board.split("class=\"dock\"").next().unwrap_or(board);
    for elaboration in ["elaboration/atlas-provider-limits/research", "elaboration/atlas-provider-limits/prototype"] {
        assert_eq!(
            board.matches(&format!("id=\"{elaboration}\"")).count(),
            1,
            "`{elaboration}` is drawn twice on the board"
        );
        let at = board.find(&format!("id=\"{elaboration}\"")).expect("the bead");
        let before = &board[..at];
        let opened = before.rfind("<article class=\"thread\"").expect("inside a thread");
        assert!(
            !before[opened..].contains("</article>"),
            "`{elaboration}` is not inside its intent's thread (209, S13)"
        );
    }
    // And a unit of a bolt is a link in that bolt's chain.
    let ledger = board
        .split("<article class=\"ledger\"")
        .find(|block| block.contains("id=\"bolt/atlas/plan-rows\""))
        .expect("the bolt");
    let ledger = ledger.split("</article>").next().unwrap_or_default();
    assert!(
        ledger.contains("id=\"unit/atlas/status-writer\""),
        "a unit is not on its bolt's chain (209, S15): {ledger}"
    );
}

/// The helpers the board reads an object by, each on its own: what a thing is
/// called, which repository it belongs to, and what it is doing split into the
/// one word a head has room for and the rest.
#[test]
fn the_board_reads_an_object_by_its_name_its_repository_and_its_state() {
    use crate::page::{name_of, repository_of, state_and_rest};
    assert_eq!(name_of("intent/atlas-provider-limits"), "atlas-provider-limits");
    assert_eq!(name_of("bolt/atlas/plan-rows"), "plan-rows");
    assert_eq!(name_of("unit/atlas/rail-tail/wi-1"), "wi-1");
    assert_eq!(name_of("willdan"), "willdan");

    // A repository is the middle segment where the id has one, and nothing
    // where it does not: `intent/atlas-provider-limits` names no repository.
    assert_eq!(repository_of("bolt/atlas/plan-rows"), Some("atlas"));
    assert_eq!(repository_of("unit/atlas/rail-tail/wi-1"), Some("atlas"));
    assert_eq!(repository_of("intent/atlas-provider-limits"), None);
    assert_eq!(repository_of("willdan"), None);

    // The head takes the object's own life; the rest goes under it, where it
    // wraps. A head is `nowrap` by design and the whole string ran off the lane.
    assert_eq!(
        state_and_rest("open · citations moved · line current"),
        ("open", "citations moved · line current")
    );
    assert_eq!(state_and_rest("proposed"), ("proposed", ""));
    assert_eq!(state_and_rest(""), ("", ""));
}

/// Proposed chores of one repository's shared line fold into one decision
/// headed by the repository's name and answered yes or drop; the blueprints'
/// fold apart under their own name; and the fold's page lists every chore by
/// the document it points at (60, 11, S231).
#[test]
fn shared_line_chores_fold_by_repository() {
    let (mut store, world, defs) = a_page();
    let at = commands::now(&store).expect("a point");
    commands::put_new(&mut store, &defs, "instance/willdan", "instance", None, Default::default(), at).expect("the instance");
    for (id, parent, repository, document, entry) in [
        ("unit/atlas/chore-1", "repository/atlas", "atlas", "flywheel/curation/chores/agents-md.md", "curation/willdan/main/1#3"),
        ("unit/atlas/chore-2", "repository/atlas", "atlas", "flywheel/curation/chores/rename-ref.md", "curation/willdan/main/1#4"),
        ("unit/blueprints/chore-1", "instance/willdan", "blueprints", "flywheel/curation/chores/skill-typo.md", "curation/willdan/main/1#5"),
    ] {
        let record = [
            ("type", json!("chore")),
            ("type_version", json!(2)),
            ("batch", json!(repository)),
            ("repository", json!(repository)),
            ("document", json!(document)),
            ("sources", json!([entry])),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        commands::put_new(&mut store, &defs, id, "unit", Some(parent), record, at).expect("the chore");
    }
    commands::rail(&mut store, &defs).expect("the rail derives");
    let mut read = crate::page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("the page reads");
    let chores: Vec<_> = read.decisions.iter().filter(|d| d.kind == "unit-proposed").cloned().collect();
    assert_eq!(chores.len(), 2, "one decision per repository: {chores:?}");
    let atlas = chores
        .iter()
        .find(|d| d.folds.iter().any(|f| f == "unit/atlas/chore-1"))
        .expect("atlas's fold");
    assert!(atlas.folds.iter().any(|f| f == "unit/atlas/chore-2"), "atlas's chores are one decision: {:?}", atlas.folds);

    let html = crate::page::render(&read);
    let card = |number: Option<u32>| -> String {
        let label = format!("aria-label=\"decision {}\"", number.expect("numbered"));
        let start = html.find(&label).unwrap_or_else(|| panic!("no card {label}"));
        let rest = &html[start..];
        rest[..rest.find("</article>").expect("the card closes")].to_string()
    };
    let held = card(atlas.number);
    assert!(held.contains(">atlas · 2 chores<"), "headed by its repository: {held}");
    assert!(held.contains(">chores<"), "{held}");
    assert!(held.contains("offered by curation/willdan/main/1"), "{held}");
    assert!(held.contains("data-answer=\"yes\"") && held.contains("data-answer=\"drop\""), "{held}");
    assert_eq!(held.matches("data-answer=").count(), 2, "answered yes or drop and nothing else: {held}");
    let blueprints = chores.iter().find(|d| d.object == "unit/blueprints/chore-1").expect("the blueprints' fold");
    assert!(card(blueprints.number).contains(">blueprints · 1 chore<"));
    // On the board, each chore says which repository it is in: both are
    // `chore-1` by name.
    assert!(html.contains("<span class=\"pre\">blueprints · </span>chore-1"), "the blueprints' chore is not told apart on the board");

    read.opened = Some("unit/atlas/chore-2".into());
    let docked = crate::page::render(&read);
    for document in ["flywheel/curation/chores/agents-md.md", "flywheel/curation/chores/rename-ref.md"] {
        assert!(docked.contains(document), "the fold's page does not list {document}");
    }
    assert!(docked.contains("atlas · chores"));
}

/// Two chores merged onto two shared lines are both `chore-1` by name, so each
/// line under since says which repository it merged in, greyed before the name,
/// as its slip did on the board (209, S231).
#[test]
fn a_merged_chore_under_since_names_its_repository() {
    let (mut store, world, defs) = a_page();
    let at = commands::now(&store).expect("a point");
    commands::put_new(&mut store, &defs, "instance/willdan", "instance", None, Default::default(), at).expect("the instance");
    for (id, parent, repository) in [
        ("unit/atlas/chore-1", "repository/atlas", "atlas"),
        ("unit/blueprints/chore-1", "instance/willdan", "blueprints"),
    ] {
        let record = [("type", json!("chore")), ("repository", json!(repository)), ("scope", json!("shared-line"))]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        commands::put_new(&mut store, &defs, id, "unit", Some(parent), record, at).expect("the chore");
        let mut unit = Records::get(&store, id).expect("a read").expect("the chore");
        unit.config.retain(|region, _| !region.starts_with("life."));
        unit.config.insert("life".into(), "merged".into());
        unit.entered_at.insert("life".into(), at);
        let base = unit.seq;
        Records::put(&mut store, id, &unit, base).expect("the chore merged");
    }
    commands::rail(&mut store, &defs).expect("the rail derives");

    let html = rendered(&mut store, &world, &defs);
    let listed = html
        .split("<ul class=\"since\">")
        .nth(1)
        .and_then(|rest| rest.split("</ul>").next())
        .expect("what happened lately is listed");
    for repository in ["atlas", "blueprints"] {
        let line = format!("<span class=\"pre\">{repository} · </span>chore-1</a>");
        assert!(listed.contains(&line), "no merged line names {repository}: {listed}");
    }
}
