//! The served page: the rail, the capture box, the board, the status view and
//! the dock (D11, D16, `surfaces/page`).
//!
//! Its design is not settled here. It is the blueprints' ratified mockup,
//! `design/flywheel-next/mockups/rail-and-board.html`, and this module renders
//! that file's markup and styles for the regions phase 1 has (D16). The
//! skeleton and the stylesheet are one file, `page/template.html`, which the
//! mockup is diffed against by `page_carries_the_mockups_regions`; every
//! element the mockup gives an id in those regions is present under that id, so
//! the page cannot drift from the design the way a paraphrase can.
//!
//! It is rendered from the register and the objects on every request. No
//! rendering is stored (15), the page holds no client state a reload loses
//! (310), and the bundle fetches nothing from anywhere else — every style it
//! needs is in the document it serves, and it runs no script at all (310).
//!
//! Under 760px it is the same bundle: two tabs, Decisions and Board, with the
//! dock full screen and a back control (307). One bundle is built and one is
//! served, and its version is the binary's (307).

use crate::links;
use flywheel_atoms::{StateStore, World};
use flywheel_domain::signals;
use flywheel_domain::sinks;
use flywheel_domain::{commands, status};
use flywheel_engine::{DecisionInstance, Definitions, Object};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The version the bundle carries: the binary's own (307).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The one file the mockup is diffed against (D16, task 14.1).
const TEMPLATE: &str = include_str!("page/template.html");

/// What one request read. Everything the page shows comes from here, so the
/// whole page is one read and a reload shows what is recorded (310).
pub struct Read {
    pub decisions: Vec<DecisionInstance>,
    pub status: status::Status,
    pub objects: Vec<Object>,
    /// The host's address, for the links every rail line carries (308, 205a).
    pub address: String,
    /// The identity every response records as given by (153, 236a, 253a).
    pub operator: String,
    /// The objects held by a host past its stale window: a link to one says the
    /// host is away and since when, rather than failing silently (308, 150a).
    pub away: BTreeMap<String, sinks::Away>,
    /// What each proposed intent weighs: its signals, how many, from which
    /// sources and over what span (109, 118).
    pub weight: BTreeMap<String, signals::Weight>,
    /// Every answer recorded, by the number of the decision it answered. A
    /// response is recorded when it is given and applied on the next tick, so
    /// this is what a reload shows the operator: the answer, who gave it and
    /// when (153, 154, 310).
    pub answered: BTreeMap<u32, Vec<Answered>>,
    /// The signals with no standing move, each one the curator's surface offers
    /// the five moves on (107, 110, 116, 118).
    pub unmoved: Vec<signals::Signal>,
    /// The curation session the operator is the runner of, where one is charged
    /// (93b, 110). With none, the surface says so and offers no control: a move
    /// is a session's delivery and never a write of its own.
    pub curation: Option<String>,
    /// The intents a move may name, so attaching and joining are picked rather
    /// than remembered (194).
    pub intents: Vec<String>,
}

/// The curation session the operator runs, where one is charged: the session
/// fact with no `ended_at`, whose stem is the curation object's (93b, 110).
///
/// It is read from the same objects the rest of the page is, so the whole page
/// is still one read (310).
pub fn curating(objects: &[Object]) -> Option<String> {
    let curation: Vec<&Object> = objects.iter().filter(|o| o.machine == "curation").collect();
    objects
        .iter()
        .filter_map(|o| o.id.strip_prefix("fact/session/"))
        .filter(|session| curation.iter().any(|c| session.starts_with(&format!("{}/", c.id))))
        .find(|session| {
            objects
                .iter()
                .find(|o| o.id == format!("fact/session/{session}"))
                .and_then(|o| o.record.get("ended_at"))
                .is_none_or(|v| v.is_null())
        })
        .map(String::from)
}

/// One answer as the record holds it (153).
#[derive(Debug, Clone)]
pub struct Answered {
    pub answer: String,
    pub given_by: String,
    pub given_at: String,
}

impl Answered {
    /// The answer as the page says it: what was answered, by whom, and when.
    pub fn said(&self) -> String {
        format!("{} · {} · {}", self.answer, self.given_by, short(&self.given_at))
    }
}

/// A recorded time as the page prints it: the day and the minute.
fn short(at: &str) -> String {
    at.split_once('T')
        .map(|(day, rest)| format!("{day} {}", &rest[..rest.len().min(5)]))
        .unwrap_or_else(|| at.to_string())
}

/// The answers among a read's objects, by the decision number each answered.
/// Read from the same objects the rest of the page is, so the whole page is
/// still one read (310).
fn answers_of(objects: &[Object]) -> BTreeMap<u32, Vec<Answered>> {
    let mut out: BTreeMap<u32, Vec<Answered>> = BTreeMap::new();
    for object in objects.iter().filter(|o| o.machine == "response") {
        let field = |name: &str| {
            object
                .record
                .get(name)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };
        let Some(number) = object.record.get("decision").and_then(|v| v.as_u64()) else {
            continue;
        };
        out.entry(number as u32).or_default().push(Answered {
            answer: field("answer"),
            given_by: field("given_by"),
            given_at: field("given_at"),
        });
    }
    out
}

/// Read once, for one request (310).
pub fn read<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &W,
    defs: &Definitions,
    address: &str,
    operator: &str,
) -> anyhow::Result<Read> {
    let decisions = commands::rail(store, defs)?;
    let at = commands::now(store)?;
    let as_of = store.read(flywheel_domain::RAIL)?.as_of;
    let files = signals::Blueprints(world);
    let status = status::read_with(
        store,
        defs,
        &as_of,
        at,
        chrono::Duration::minutes(5),
        chrono::Duration::minutes(30),
        &files,
    )?;
    let objects = store.list_records(&flywheel_atoms::Scope::All)?;
    let away = sinks::away_by_object(store, at, chrono::Duration::minutes(5))?;
    // What a proposed intent weighs, from the same material (109, 118).
    let mut weight = BTreeMap::new();
    for object in &objects {
        if object.machine != "intent" {
            continue;
        }
        let cited: Vec<String> = object
            .record
            .get("signals")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        if cited.is_empty() {
            continue;
        }
        weight.insert(object.id.clone(), signals::weight_of(&files, &cited));
    }
    let answered = answers_of(&objects);
    // What curation has to judge, and the session the operator judges it in
    // (110, 118, 93b).
    let unmoved = signals::unmoved(&files);
    let curation = curating(&objects);
    let intents: Vec<String> = objects
        .iter()
        .filter(|o| o.machine == "intent")
        .map(|o| o.id.clone())
        .collect();
    Ok(Read {
        decisions,
        status,
        objects,
        address: address.to_string(),
        operator: operator.to_string(),
        away,
        weight,
        answered,
        unmoved,
        curation,
        intents,
    })
}

/// The kinds the page gives a form of its own. The phase an object is in is
/// shown by where it sits and never by its form (209).
pub const KINDS: [&str; 6] = [
    "decision",
    "intent",
    "elaboration",
    "bolt",
    "proposal",
    "signal",
];

/// The mockup's four lanes, and the machines each one holds. Phase is the
/// lane's; kind is the shape's (D16, the mockup's own rule).
const LANES: [(&str, &[&str]); 4] = [
    (
        "{{LANE_INCEPTION}}",
        &["capture", "signal", "intent", "elaboration"],
    ),
    ("{{LANE_PLAN}}", &["proposal"]),
    (
        "{{LANE_CONSTRUCTION}}",
        &["bolt", "unit", "work item", "work-item", "session", "rail"],
    ),
    ("{{LANE_OPERATION}}", &["instance", "host", "sink", "landed"]),
];

/// The lane's own title and what it holds, in the mockup's order.
const LANE_HEADS: [(&str, &str); 4] = [
    ("Inception", "captures, signals, intents and their elaborations"),
    ("Bolt plan", "one proposal per repository"),
    ("Construction", "bolts, their units and the sessions under them"),
    ("Operation", "the instance, its hosts and its sinks"),
];

/// Render the whole page. One document, one request, nothing stored.
pub fn render(read: &Read) -> String {
    let instance = read.address.rsplit('/').next().unwrap_or_default();
    let standing = read.decisions.len();
    let sent: usize = read.answered.values().map(Vec::len).sum();
    let mut out = TEMPLATE
        .replace("{{VERSION}}", VERSION)
        .replace("{{INSTANCE}}", &escape(instance))
        .replace("{{OPERATOR}}", &escape(&read.operator))
        .replace("{{CLOCK}}", &escape(&short(&read.status.at.to_rfc3339())))
        .replace("{{COUNT}}", &standing.to_string())
        .replace("{{COUNTHINT}}", &count_hint(read))
        .replace("{{SENT}}", &sent.to_string())
        .replace("{{OBJECTS}}", &read.status.rows.len().to_string())
        .replace("{{HOSTS}}", &hosts(read))
        .replace("{{RAIL}}", &rail(read))
        .replace("{{BOARDH}}", &board_header(read))
        .replace("{{DOCK}}", &dock(read))
        .replace("{{SENTLIST}}", &sent_list(read))
        .replace("{{LOG}}", &log(read));
    for ((slot, machines), (title, sub)) in LANES.iter().zip(LANE_HEADS.iter()) {
        out = out.replace(slot, &lane(read, title, sub, machines));
    }
    out
}

/// What the count's hint says: the numbers standing, in the order the register
/// gave them (15).
fn count_hint(read: &Read) -> String {
    let numbers: Vec<String> = read
        .decisions
        .iter()
        .filter_map(|d| d.number)
        .map(|n| n.to_string())
        .collect();
    match numbers.is_empty() {
        true => String::new(),
        false => escape(&format!("· {}", numbers.join(", "))),
    }
}

/// The hosts strip: each host holding an object, with its liveness and how much
/// it holds. It administers nothing (141, 143, 146).
fn hosts(read: &Read) -> String {
    let mut held: BTreeMap<&str, (usize, &str)> = BTreeMap::new();
    for row in &read.status.rows {
        let Some(holder) = row.holder.as_deref() else {
            continue;
        };
        let entry = held.entry(holder).or_insert((0, "alive"));
        entry.0 += 1;
        if let Some(liveness) = row.liveness.as_deref() {
            entry.1 = liveness;
        }
    }
    let mut out = String::from("<span class=\"lab\">hosts</span>");
    if held.is_empty() {
        out.push_str("<span class=\"note\">no host holds an object</span>");
        return out;
    }
    for (host, (holds, liveness)) in held {
        let _ = write!(
            out,
            "<span class=\"host {gone}\" data-host=\"{h}\" data-liveness=\"{l}\">\
             <b class=\"hn\">{h}</b><span class=\"hm\">{holds} held · <b>{l}</b></span></span>",
            gone = match liveness {
                "alive" => "",
                _ => "gone",
            },
            h = escape(host),
            l = escape(liveness)
        );
    }
    out
}

/// The rail: every standing decision with its number and its answers, one tap
/// each, in the groups the model folds them into (15, 11, 311).
fn rail(read: &Read) -> String {
    let mut out = String::from(
        "<div class=\"rail-h\"><h2>Decisions</h2>\
         <span class=\"sub\">the plan, in the order the chat prints it</span>\
         <a class=\"btn sm phone-only\" id=\"pal-open-rail\" href=\"#pal-scrim\">capture…</a></div>\n",
    );
    if read.decisions.is_empty() {
        out.push_str(
            "<div class=\"empty\">Nothing waits on you. The board runs on its own until the \
             next decision is yours.</div>\n",
        );
    }
    let mut group = String::new();
    for decision in &read.decisions {
        if decision.group != group {
            group = decision.group.clone();
            let _ = write!(
                out,
                "<div class=\"grp {0}\"><span class=\"g\">{0}</span></div>\n",
                escape(&group)
            );
        }
        out.push_str(&card(read, decision));
    }
    out
}

/// One decision, in the mockup's card silhouette — the only answerable form on
/// the page (209, D16).
fn card(read: &Read, decision: &DecisionInstance) -> String {
    let mut out = String::new();
    let number = decision.number.map(|n| n.to_string()).unwrap_or_default();
    let link = links::to_object(&read.address, &decision.object).unwrap_or_default();
    let _ = write!(
        out,
        "<article class=\"card decision g-{group}\" data-kind=\"decision\" \
         data-number=\"{number}\" data-object=\"{object}\" data-group=\"{group}\" \
         aria-label=\"decision {number}\">\n",
        group = escape(&decision.group),
        object = escape(&decision.object)
    );
    let _ = write!(
        out,
        "<div class=\"ch\"><span class=\"n number\">{number}</span>\
         <span class=\"kind\">{kind}</span></div>\n",
        kind = escape(&decision.group)
    );
    let _ = write!(
        out,
        "<div class=\"title\"><a class=\"object\" href=\"{}\">{}</a></div>\n",
        escape(&link),
        escape(&decision.object)
    );
    if let Some(away) = read.away.get(&decision.object) {
        let _ = write!(
            out,
            "<p class=\"away\" data-away-host=\"{}\">{}</p>\n",
            escape(&away.host),
            escape(&away.said())
        );
    }
    out.push_str("<div class=\"answers\">\n");
    for answer in &decision.answers {
        // One tap each, and nothing behind a hover or a keyboard (311). The
        // control posts to the one tool the chat's numbered reply grammar
        // calls, so the two surfaces share a write path (193, 194).
        let _ = write!(
            out,
            "<form method=\"post\" action=\"/api/tools/{tool}\" class=\"answer\">\n\
             <input type=\"hidden\" name=\"decision\" value=\"{number}\">\n\
             <input type=\"hidden\" name=\"answer\" value=\"{0}\">\n\
             <button type=\"submit\" class=\"btn sm\" data-answer=\"{0}\">{0}</button>\n</form>\n",
            escape(answer),
            tool = crate::catalogue::ANSWER
        );
    }
    out.push_str("</div>\n");
    // What has already been answered, with who gave it and when: the response
    // is recorded when it is given and applied on the next tick, so a reload
    // shows it before the decision is retracted (153, 154, 310).
    out.push_str(&answered(read, decision.number));
    out.push_str("</article>\n");
    out
}

/// What was answered on one decision, with who gave it and when (153, 154).
fn answered(read: &Read, number: Option<u32>) -> String {
    let mut out = String::new();
    for given in number.and_then(|n| read.answered.get(&n)).into_iter().flatten() {
        let _ = write!(
            out,
            "<p class=\"answered\" data-answer=\"{}\" data-given-by=\"{}\" \
             data-given-at=\"{}\">{}</p>\n",
            escape(&given.answer),
            escape(&given.given_by),
            escape(&given.given_at),
            escape(&given.said())
        );
    }
    out
}

/// The board's header: what the read is as of, which is what the committed
/// projection states too (145, D12).
fn board_header(read: &Read) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "<span class=\"as-of\">as of commit {} at {}</span>",
        escape(&read.status.as_of.mark),
        escape(&read.status.as_of.at.to_rfc3339())
    );
    let _ = write!(
        out,
        "<span class=\"r\"><a class=\"btn sm phone-only\" id=\"pal-open-phone\" \
         href=\"#pal-scrim\">capture…</a></span>"
    );
    out
}

/// One lane of the board: the objects whose phase it is, grouped as the status
/// view groups them — queued, in progress, waiting on the operator and done —
/// each with its holder, its runner and that host's liveness (141, 143, 146).
///
/// The lane is the phase and the silhouette is the kind, which is the mockup's
/// own rule; the grouping inside it is 141's (D16).
fn lane(read: &Read, title: &str, sub: &str, machines: &[&str]) -> String {
    let mut out = String::new();
    let mine: Vec<&status::Row> = read
        .status
        .rows
        .iter()
        .filter(|row| in_lane(&row.machine, machines))
        .collect();
    let _ = write!(
        out,
        "<div class=\"lane-h\"><h2 class=\"lane-title\">{}</h2>\
         <span class=\"sub\">{}</span><span class=\"n\">{}</span></div>\n",
        escape(title),
        escape(sub),
        mine.len()
    );
    // The unmoved signals belong to inception, where curation reads them, and
    // none of them is discarded (118, 110).
    if machines.contains(&"signal") {
        out.push_str(&unmoved(read));
    }
    out.push_str("<div class=\"status-groups\">\n");
    for group in status::GROUPS {
        let rows: Vec<&&status::Row> = mine.iter().filter(|row| row.group == group).collect();
        let _ = write!(
            out,
            "<section class=\"sec-h-group\" data-group=\"{}\">\n<h2>{group}</h2>\n",
            group.replace(' ', "-")
        );
        if rows.is_empty() {
            out.push_str("<div class=\"empty\">nothing</div>\n");
        }
        for row in rows {
            out.push_str(&object_on_the_board(read, row));
        }
        out.push_str("</section>\n");
    }
    out.push_str("</div>\n");
    out
}

/// Whether a machine's objects sit in this lane. A machine no lane names sits
/// in construction, which is where the work is.
fn in_lane(machine: &str, machines: &[&str]) -> bool {
    if machines.contains(&machine) {
        return true;
    }
    let named = LANES.iter().any(|(_, m)| m.contains(&machine));
    !named && machines.contains(&"bolt")
}

/// One object on the board, in the silhouette its kind has: never one card
/// class with variants (D16, the mockup's own rule).
fn object_on_the_board(read: &Read, row: &status::Row) -> String {
    let mut out = String::new();
    let link = links::to_object(&read.address, &row.object).unwrap_or_default();
    let _ = write!(
        out,
        "<article class=\"{form}\" id=\"{object}\" data-machine=\"{machine}\" \
         data-holder=\"{holder}\" data-runner=\"{runner}\" data-liveness=\"{liveness}\">\n",
        form = silhouette(&row.machine),
        object = escape(&row.object),
        machine = escape(&row.machine),
        holder = escape(row.holder.as_deref().unwrap_or("none")),
        runner = escape(row.runner.as_deref().unwrap_or("none")),
        liveness = escape(row.liveness.as_deref().unwrap_or("none")),
    );
    let _ = write!(
        out,
        "<h3><a href=\"#dock-{0}\">{1}</a></h3>\n",
        escape(&row.object),
        escape(&row.object)
    );
    let _ = write!(
        out,
        "<p class=\"state\">{}</p>\n",
        escape(&row.states.join(" · "))
    );
    let _ = write!(
        out,
        "<p class=\"holder\">held by {} ({})</p>\n",
        escape(row.holder.as_deref().unwrap_or("no host")),
        escape(row.liveness.as_deref().unwrap_or("no host"))
    );
    if let Some(runner) = &row.runner {
        let _ = write!(out, "<p class=\"runner\">run by the {}</p>\n", escape(runner));
    }
    // The question, the answer and the note stay with the object and are shown
    // under it (144).
    for (kind, said) in &row.discussion {
        let _ = write!(
            out,
            "<p class=\"said\" data-kind=\"{}\">{}</p>\n",
            escape(kind),
            escape(said)
        );
    }
    let _ = write!(out, "<a class=\"lk\" href=\"{}\">open</a>\n", escape(&link));
    out.push_str("</article>\n");
    out
}

/// The mockup's silhouette for a kind: a sheet for a proposal, a slip for a
/// unit, a thread for an intent, a ledger for a bolt, a quote for a signal, a
/// session row for everything with a session under it (D16).
fn silhouette(machine: &str) -> &'static str {
    match machine {
        "proposal" => "sheet",
        "unit" => "slip",
        "intent" => "thread",
        "elaboration" => "thread bead",
        "bolt" => "ledger",
        "signal" | "capture" => "quote",
        _ => "sessrow",
    }
}

/// The signals with no move, by source and by age. Nothing here is discarded: a
/// signal nobody has judged is one the operator has not seen yet (118).
fn unmoved(read: &Read) -> String {
    let mut out = String::from(
        "<section id=\"unmoved-signals\"><div class=\"sec-h\">unmoved signals\
         <span class=\"r\">by source, with the oldest one's age</span></div>\n",
    );
    if read.status.unmoved.is_empty() {
        out.push_str("<div class=\"empty\">nothing unmoved</div>\n");
    }
    for source in &read.status.unmoved {
        let age = source
            .oldest
            .as_deref()
            .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
            .map(|at| (read.status.at - at.with_timezone(&chrono::Utc)).num_days())
            .map(|days| format!(", oldest {days}d"))
            .unwrap_or_default();
        let _ = write!(
            out,
            "<p class=\"unmoved\" data-source=\"{}\" data-count=\"{}\">{} from {}{age}</p>\n",
            escape(&source.source),
            source.count,
            source.count,
            escape(&source.source)
        );
    }
    out.push_str(&curator(read));
    out.push_str("</section>\n");
    out
}

/// The curator's surface: every unmoved signal with the standing moves as
/// controls, and one submit that is the curation session's delivery and its
/// exit (110, 101, 116, 93b, D16).
///
/// Curation is charged by the tick and in this phase the operator is its
/// session, so this is where a fresh instance turns captures into a decision
/// with nothing written by hand. The moves are controls and the target is
/// picked: no word of what the signal says is read for meaning (194).
fn curator(read: &Read) -> String {
    let mut out = String::from(
        "<section id=\"curate\" class=\"curation\"><div class=\"sec-h\">curation\
         <span class=\"r\">one move per signal; the submit is the session's delivery \
         and its exit (110, 116)</span></div>\n",
    );
    let Some(session) = &read.curation else {
        out.push_str(
            "<div class=\"empty\">no curation session is charged; the tick charges one when \
             the unmoved signals cross the threshold or the cadence says so (110)</div>\n\
             </section>\n",
        );
        return out;
    };
    if read.unmoved.is_empty() {
        out.push_str("<div class=\"empty\">nothing unmoved to judge</div>\n</section>\n");
        return out;
    }
    let _ = write!(
        out,
        "<form method=\"post\" action=\"/api/curate\" id=\"curate-box\" data-session=\"{}\">\n",
        escape(session)
    );
    // The intents a move may name, offered rather than remembered; a join may
    // also name one that is not there yet, which is how a cluster becomes a
    // proposed intent (110, 116).
    out.push_str("<datalist id=\"curate-intents\">\n");
    for intent in &read.intents {
        let _ = write!(out, "<option value=\"{0}\"></option>\n", escape(intent));
    }
    out.push_str("</datalist>\n");
    for signal in &read.unmoved {
        let _ = write!(
            out,
            "<div class=\"quote\" data-signal=\"{}\"><q>{}</q>\
             <span class=\"qm\">{} · asserted by {}</span>\n",
            escape(&signal.id),
            escape(&signal.excerpt),
            escape(&signal.kind),
            escape(&signal.asserted_by)
        );
        let _ = write!(
            out,
            "<select name=\"move.{0}\" aria-label=\"the move for {0}\">\n\
             <option value=\"\" selected>leave unmoved</option>\n",
            escape(&signal.id)
        );
        for word in signals::MOVES {
            let _ = write!(out, "<option value=\"{word}\">{word}</option>\n");
        }
        out.push_str("</select>\n");
        let _ = write!(
            out,
            "<input type=\"text\" name=\"target.{0}\" list=\"curate-intents\" \
             aria-label=\"what the move for {0} names\" \
             placeholder=\"the intent, claim or offer it names\">\n",
            escape(&signal.id)
        );
        out.push_str("</div>\n");
    }
    out.push_str(
        "<button class=\"btn pri\" type=\"submit\" id=\"curate-go\">submit the moves</button>\n\
         </form>\n</section>\n",
    );
    out
}

/// The dock: one surface per object, each kind in its own form, full screen
/// under 760px with a back control (209, 210, 307).
fn dock(read: &Read) -> String {
    let mut out = String::new();
    for object in &read.objects {
        let kind = match object.machine.as_str() {
            m if KINDS.contains(&m) => m,
            other => other,
        };
        let link = links::to_object(&read.address, &object.id).unwrap_or_default();
        let _ = write!(
            out,
            "<article class=\"surface form-{kind}\" id=\"dock-{}\" data-kind=\"{kind}\" \
             data-answerable=\"false\">\n",
            escape(&object.id)
        );
        let _ = write!(
            out,
            "<a class=\"object\" href=\"{}\">{}</a>\n",
            escape(&link),
            escape(&object.id)
        );
        // A link to a host past its stale window opens this surface and says
        // the host is away and since when, rather than failing silently
        // (308, 150a).
        if let Some(away) = read.away.get(&object.id) {
            let _ = write!(
                out,
                "<p class=\"away\" data-away-host=\"{}\">{}</p>\n",
                escape(&away.host),
                escape(&away.said())
            );
        }
        // A proposed intent shows its weight: the signals it cites, how many,
        // from which sources and over what span, counted by event date
        // (109, 118).
        if let Some(weight) = read.weight.get(&object.id) {
            let _ = write!(
                out,
                "<p class=\"weight\" data-signals=\"{}\" data-sources=\"{}\">{} signal{} from {}{}</p>\n",
                weight.count,
                escape(&weight.sources.join(" ")),
                weight.count,
                match weight.count {
                    1 => "",
                    _ => "s",
                },
                escape(&weight.sources.join(", ")),
                weight
                    .span()
                    .map(|s| format!(", {}", escape(&s)))
                    .unwrap_or_default()
            );
            out.push_str("<ul class=\"cited\">\n");
            for signal in &weight.signals {
                let _ = write!(out, "<li class=\"signal\">{}</li>\n", escape(signal));
            }
            out.push_str("</ul>\n");
        }
        // An elaboration is a surface of its own, reached from its intent, and
        // an intent lists its elaborations in order (210).
        if object.machine == "intent" {
            out.push_str("<ol class=\"elaborations\">\n");
            for child in read
                .objects
                .iter()
                .filter(|o| o.machine == "elaboration" && o.parent.as_deref() == Some(&object.id))
            {
                let _ = write!(
                    out,
                    "<li><a class=\"elaboration\" href=\"#dock-{0}\">{0}</a></li>\n",
                    escape(&child.id)
                );
            }
            out.push_str("</ol>\n");
        }
        out.push_str("</article>\n");
    }
    out
}

/// The last five responses, at the head of the capture box, as the mockup's
/// palette shows them at rest (D16).
fn sent_list(read: &Read) -> String {
    let mut all: Vec<(u32, &Answered)> = read
        .answered
        .iter()
        .flat_map(|(number, given)| given.iter().map(move |g| (*number, g)))
        .collect();
    all.sort_by(|a, b| b.1.given_at.cmp(&a.1.given_at));
    if all.is_empty() {
        return String::from(
            "<div class=\"pal-empty\">nothing sent yet · what you type is captured whole \
             and nothing in it is read as a command (19, 194)</div>",
        );
    }
    let mut out = String::from("<div class=\"pal-h\">sent</div>");
    for (number, given) in all.into_iter().take(5) {
        let _ = write!(
            out,
            "<div class=\"pal-sent\"><time>{}</time><span>{number} → {}</span></div>",
            escape(&short(&given.given_at)),
            escape(&given.answer)
        );
    }
    out
}

/// Every response recorded, each one on its own, with who gave it and when
/// (153, 154).
fn log(read: &Read) -> String {
    let mut out = String::new();
    for (number, given) in &read.answered {
        for one in given {
            let _ = write!(
                out,
                "<li class=\"say\" data-decision=\"{number}\"><time>{}</time>\
                 <span>{number} → {} · {}</span></li>\n",
                escape(&short(&one.given_at)),
                escape(&one.answer),
                escape(&one.given_by)
            );
        }
    }
    if out.is_empty() {
        out.push_str("<li class=\"say\"><span>nothing sent yet</span></li>\n");
    }
    out
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
