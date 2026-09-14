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
    /// What each standing decision is about, in one line: the fields and the
    /// evidence the machine's own `shows:` names for that decision kind,
    /// resolved against the object, and how long it has stood. Keyed by the
    /// decision's id (15, 11, 18).
    pub why: BTreeMap<String, Vec<String>>,
    /// The scenario this instance stands part-way through, where it stands in
    /// one (`design/flywheel-next/scenarios/storefront.md`).
    ///
    /// A scenario applied into an instance leaves a fact behind, and that fact
    /// is the whole of what puts the tour's overlay on the page: an instance
    /// with actions left onboards whoever is looking at it, and one with none
    /// — every instance an operator made for themselves among them — has no
    /// overlay at all. It is read from the store like everything else here, so
    /// there is no flag and no second page.
    pub tour: Option<flywheel_domain::tour::Tour>,
    /// What the last control the operator used was refused for, where it was
    /// refused. The page's controls are plain forms with no script behind them,
    /// so a refusal comes back as the page itself with the reason on it and
    /// never as a body the operator is stranded on (310, 311).
    pub refused: Option<String>,
    /// The object a link opened the page at, where the request named one: the
    /// dock's surface for it is the one already open, so a link the machinery
    /// wrote lands on the object it names rather than on the board (308, 205a,
    /// 209).
    pub opened: Option<String>,
}

/// The curation session the operator runs, where one is charged: the newest
/// attempt under a curation object whose `run` region is `running` (93b, 110,
/// `curation.yaml`).
///
/// The machine is what says whether the operator is being asked. `run` is
/// `running` while a session stands over the unmoved signals, and the moment
/// that session's exit is reported the run applies the moves and goes idle. A
/// session fact carrying no `ended_at` is not that reading and never was: a
/// session that reports an exit reaches `exited`, which is final, and
/// `end_session` — the only thing that writes `ended_at` — runs from `ended`
/// alone, which the owner's explicit end reaches (`session.yaml`). So the box
/// stood on the page long after the curation it belonged to had closed, over
/// signals whose moves were already recorded, and a submit on it would have
/// delivered a second exit for a session that had already reported one.
///
/// It is read from the same objects the rest of the page is, so the whole page
/// is still one read (310).
pub fn curating(objects: &[Object]) -> Option<String> {
    let running: Vec<&str> = objects
        .iter()
        .filter(|o| o.machine == "curation")
        .filter(|o| o.config.get("run").map(String::as_str) == Some("running"))
        .map(|o| o.id.as_str())
        .collect();
    objects
        .iter()
        .filter_map(|o| o.id.strip_prefix("fact/session/"))
        .filter(|session| running.iter().any(|c| session.starts_with(&format!("{c}/"))))
        // The attempt standing now is the newest: a run that failed and came
        // round again left the earlier one readable under its own number
        // (`session.yaml` id).
        .max_by_key(|session| {
            session
                .rsplit('/')
                .next()
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(0)
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
    let decisions = commands::rail_read(store, defs)?;
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
    // What each decision is about. `shows:` in the machine is the model's own
    // answer to what a decision puts in front of the operator, so the line is
    // read from the definitions rather than invented per kind (15, 11).
    let mut why: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for decision in &decisions {
        let Some(object) = objects.iter().find(|o| o.id == decision.object) else {
            continue;
        };
        let evidence = store.read(&decision.object).map(|read| read.evidence).unwrap_or_default();
        let mut said: Vec<String> = Vec::new();
        for name in &decision.shows {
            // The model names a field of the record or an atom of the
            // evidence; an atom is named for the machine it belongs to
            // (`elaboration.type`), and the record holds it under its own name
            // (`type`), so the last segment is the third place to look.
            let held = object
                .record
                .get(name)
                .or_else(|| evidence.get(name))
                .or_else(|| object.record.get(name.rsplit('.').next().unwrap_or(name)));
            if let Some(line) = held.and_then(|value| shown(name, value)) {
                said.push(line);
            }
        }
        if let Some(age) = age(at, decision.since) {
            said.push(age);
        }
        why.insert(decision.id.clone(), said);
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
        why,
        tour: flywheel_domain::tour::read(store, instance_of(address)),
        refused: None,
        opened: None,
    })
}

/// One thing a decision shows, as a line on its card, or nothing where the
/// object does not carry it.
///
/// A list is said by how many are in it, because that is what the operator
/// weighs; a flag that is not set says nothing; and everything else is said by
/// its name and its value. The name is the model's, without the path that
/// reaches it.
fn shown(name: &str, value: &serde_json::Value) -> Option<String> {
    let label = name.rsplit('.').next().unwrap_or(name).replace('_', " ");
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(false) => None,
        serde_json::Value::Bool(true) => Some(label),
        serde_json::Value::Array(held) if held.is_empty() => None,
        serde_json::Value::Array(held) => Some(format!("{} {}", held.len(), counted(&label, held.len()))),
        serde_json::Value::String(said) if said.trim().is_empty() => None,
        serde_json::Value::String(said) => Some(format!("{label} {}", clipped(said))),
        // What a structured field says is its values, not its JSON: `target
        // {"bolt": "bolt/atlas/plan-rows"}` is read as `target
        // bolt/atlas/plan-rows`.
        serde_json::Value::Object(fields) if fields.is_empty() => None,
        serde_json::Value::Object(fields) => {
            let said: Vec<String> = fields
                .values()
                .filter_map(|value| match value {
                    serde_json::Value::String(text) => Some(clipped(text)),
                    serde_json::Value::Null => None,
                    other => Some(other.to_string()),
                })
                .collect();
            match said.is_empty() {
                true => None,
                false => Some(format!("{label} {}", said.join(" "))),
            }
        }
        other => Some(format!("{label} {other}")),
    }
}

/// One of a thing is not the thing's plural. The model's own names are plural
/// where the field holds a list, so the `s` comes off when there is one.
fn counted(label: &str, how_many: usize) -> String {
    match how_many == 1 && label.ends_with('s') && !label.ends_with("ss") {
        true => label[..label.len() - 1].to_string(),
        false => label.to_string(),
    }
}

/// A value long enough to push the controls off the card is cut, with the end
/// kept where the end is what tells it apart — a path's file, a claim's version.
fn clipped(said: &str) -> String {
    let said = said.trim();
    match said.chars().count() > 60 {
        false => said.to_string(),
        true => format!("…{}", said.chars().skip(said.chars().count() - 59).collect::<String>()),
    }
}

/// How long a decision has stood, in the coarsest unit that says it: what the
/// operator weighs a proposal by is its age, not its timestamp (15, 118).
fn age(now: chrono::DateTime<chrono::Utc>, since: chrono::DateTime<chrono::Utc>) -> Option<String> {
    let stood = now - since;
    if stood < chrono::Duration::minutes(1) {
        return None;
    }
    Some(match (stood.num_days(), stood.num_hours(), stood.num_minutes()) {
        (days, _, _) if days > 0 => format!("{days}d"),
        (_, hours, _) if hours > 0 => format!("{hours}h"),
        (_, _, minutes) => format!("{minutes}m"),
    })
}

/// Which of the board's four phases an object sits in. The phase is where it
/// is, and its kind is what it is; a card says both because the operator reads
/// the rail out of the board's order (D16, 209).
fn phase_of(machine: &str) -> &'static str {
    for ((_, machines), (title, _)) in LANES.iter().zip(LANE_HEADS.iter()) {
        if machines.contains(&machine) {
            return title;
        }
    }
    "Construction"
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
/// The instance this page is of: the last segment of the host's own address,
/// which is where every link the machinery writes puts it (205a, 308).
fn instance_of(address: &str) -> &str {
    address.trim_end_matches('/').rsplit('/').next().unwrap_or_default()
}

pub fn render(read: &Read) -> String {
    let instance = instance_of(&read.address);
    // Attention stands outside the count: it is what the machinery could not do,
    // reported and never dropped, and not a choice the operator is being asked
    // to make (S3, S8, 6).
    let standing = read
        .decisions
        .iter()
        .filter(|d| flywheel_engine::rail::counted(&d.group))
        .count();
    let sent: usize = read.answered.values().map(Vec::len).sum();
    let mut out = TEMPLATE
        .replace("{{VERSION}}", VERSION)
        .replace("{{INSTANCE}}", &escape(instance))
        .replace("{{OPERATOR}}", &escape(&read.operator))
        .replace("{{CLOCK}}", &escape(&short(&read.status.at.to_rfc3339())))
        .replace("{{COUNT}}", &standing.to_string())
        .replace("{{YESALL}}", &yes_all(read))
        .replace("{{SENT}}", &sent.to_string())
        .replace("{{OBJECTS}}", &read.status.rows.len().to_string())
        .replace("{{HOSTS}}", &hosts(read))
        .replace("{{RAIL}}", &rail(read))
        .replace("{{BOARDH}}", &board_header(read))
        .replace("{{DOCK}}", &dock(read))
        .replace("{{SENTLIST}}", &sent_list(read))
        .replace("{{LOG}}", &log(read))
        .replace("{{TOUR}}", &tour(read))
        .replace("{{TOURHEAD}}", &tour_head(read));
    for ((slot, machines), (title, sub)) in LANES.iter().zip(LANE_HEADS.iter()) {
        out = out.replace(slot, &lane(read, title, sub, machines));
    }
    out
}

/// The group whose decisions "yes all" answers (11, S3, S7).
pub const APPROVE: &str = "approve";

/// The decisions "yes all" would answer, in number order.
///
/// 11 asks that the decisions be grouped so "yes to all" is a meaningful answer
/// for a simple rail, and that any single decision can still be answered on its
/// own. What makes it meaningful rather than reckless is the grouping: it is the
/// approve group, and within it only the decisions that actually offer `yes`.
/// A decide, an attention or an answer is not something a sweep of the hand
/// settles.
pub fn yes_all_answers(decisions: &[DecisionInstance]) -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = decisions
        .iter()
        .filter(|d| d.group == APPROVE)
        .filter(|d| d.answers.iter().any(|a| a == "yes"))
        .filter_map(|d| d.number.map(|n| (n, d.object.clone())))
        .collect();
    out.sort_by_key(|(number, _)| *number);
    out
}

/// The control itself: one tap, and the response it makes is one per decision
/// and never a batch (11, S2, S7).
///
/// The numbers it will answer are **on the control**, not strung across the
/// header beside it. A strip of every waiting number names no action; these
/// numbers name this control's, which is what S2 asks for — the "yes all"
/// control with the numbers it will answer — and it is what lets the operator
/// read what one tap is about to do before they make it.
fn yes_all(read: &Read) -> String {
    let numbers: Vec<u32> = yes_all_answers(&read.decisions)
        .into_iter()
        .map(|(number, _)| number)
        .collect();
    // With nothing to say yes to the control stays, and says so: an absent
    // control leaves the operator looking for one (S2).
    if numbers.is_empty() {
        return String::from(
            "<form method=\"post\" action=\"/api/answer-all\" class=\"yesall-form\">\
             <button class=\"btn pri\" id=\"yesall\" type=\"submit\" data-answers=\"\" disabled>\
             yes all<span class=\"k\">nothing waiting</span></button></form>",
        );
    }
    // The numbers travel with the tap, and the tap answers those and no
    // others. The page is rendered in one request and the control is used in
    // the next, so a control that answered a fresh read would answer a decision
    // that arrived in between — one the operator never saw. What they saw is
    // what they said yes to (S2, S7, 15).
    let ids = numbers.iter().map(u32::to_string).collect::<Vec<_>>().join(" ");
    format!(
        "<form method=\"post\" action=\"/api/answer-all\" class=\"yesall-form\">\
         <input type=\"hidden\" name=\"numbers\" value=\"{ids}\">\
         <button class=\"btn pri\" id=\"yesall\" type=\"submit\" data-answers=\"{ids}\">\
         yes all<span class=\"k\">{said}</span></button></form>",
        said = escape(&runs(&numbers)),
    )
}

/// Consecutive numbers said as a range, which is how the operator reads a rail
/// that came in together: `412–417, 422, 424`.
fn runs(numbers: &[u32]) -> String {
    let mut said: Vec<String> = Vec::new();
    let mut at = 0;
    while at < numbers.len() {
        let mut end = at;
        while end + 1 < numbers.len() && numbers[end + 1] == numbers[end] + 1 {
            end += 1;
        }
        said.push(match end - at {
            0 => numbers[at].to_string(),
            1 => format!("{}, {}", numbers[at], numbers[end]),
            _ => format!("{}–{}", numbers[at], numbers[end]),
        });
        at = end + 1;
    }
    said.join(", ")
}

/// What is wrong with a host, and nothing when nothing is (141, 143, 146, 79).
///
/// A host answers a real need — noticing that one has gone or stalled, so work
/// is not silently stopped — but that is an attention need and not a permanent
/// fixture. Every host is reported under the status view, where it stands with
/// the rest of the instance; here it is raised into the operator's way only
/// when something is wrong with it. A healthy instance raises nothing.
fn hosts(read: &Read) -> String {
    // What each host holds, and what is wrong with it where something is.
    let mut wrong: BTreeMap<&str, (usize, &str)> = BTreeMap::new();
    for row in &read.status.rows {
        let (Some(holder), Some(liveness)) = (row.holder.as_deref(), row.liveness.as_deref())
        else {
            continue;
        };
        if liveness == "alive" {
            continue;
        }
        let entry = wrong.entry(holder).or_insert((0, liveness));
        entry.0 += 1;
        entry.1 = liveness;
    }
    if wrong.is_empty() {
        return String::new();
    }
    let mut out = String::from("<span class=\"lab\">attention</span>");
    for (host, (holds, liveness)) in wrong {
        let since = read
            .away
            .values()
            .find(|away| away.host == host)
            .map(|away| format!(" since {}", short(&away.since.to_rfc3339())))
            .unwrap_or_default();
        let _ = write!(
            out,
            "<span class=\"host gone\" data-host=\"{h}\" data-liveness=\"{l}\">\
             <b class=\"hn\">{h}</b><span class=\"hm\">{l}{since} · {holds} held</span></span>",
            h = escape(host),
            l = escape(liveness),
            since = escape(&since),
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
    // A control that was refused says so where the control is, and the page is
    // otherwise the page: nothing is lost and nothing has to be gone back for
    // (81, 310, 311).
    if let Some(refused) = &read.refused {
        let _ = write!(
            out,
            "<article class=\"card refused\" data-refused=\"true\"><p class=\"refused\">{}</p></article>\n",
            escape(refused)
        );
    }
    if read.decisions.is_empty() {
        out.push_str(
            "<div class=\"empty\">Nothing waits on you. The board runs on its own until the \
             next decision is yours.</div>\n",
        );
    }
    // The groups in the model's order, each sorted by number: approve, decide,
    // answer, then attention outside the count (S3, 11). `derive` hands them
    // back in number order, because the number is what a response names; the
    // grouping is the reader's, and without it the rail walks approve, decide,
    // approve, decide and the heading repeats down the page.
    let mut group = String::new();
    let mut first = true;
    for decision in flywheel_engine::rail::in_reading_order(&read.decisions) {
        if decision.group != group || first {
            group = decision.group.clone();
            first = false;
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
    // What it is, and where it is: the kind is the object's own machine, and
    // the phase is the lane it sits in on the board. The group is the heading
    // the card is filed under and is not repeated here (209, D16).
    let machine = read
        .objects
        .iter()
        .find(|o| o.id == decision.object)
        .map(|o| o.machine.as_str())
        .unwrap_or(&decision.kind);
    let _ = write!(
        out,
        "<div class=\"ch\"><span class=\"n number\">{number}</span>\
         <span class=\"kind\">{kind}</span>\
         <span class=\"ph\" data-phase=\"{phase}\">{phase}</span></div>\n",
        kind = escape(machine),
        phase = escape(&phase_of(machine).to_lowercase()),
    );
    let _ = write!(
        out,
        "<div class=\"title\"><a class=\"object\" href=\"{}\">{}</a></div>\n",
        escape(&link),
        escape(&decision.object)
    );
    // Why it is being asked: what the machine's own `shows:` names for this
    // decision kind, and how long it has stood (15, 11, 18).
    if let Some(said) = read.why.get(&decision.id).filter(|said| !said.is_empty()) {
        let _ = write!(
            out,
            "<p class=\"tail why\">{}</p>\n",
            escape(&said.join(" · "))
        );
    }
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
        out.push_str(&control(&number, answer));
    }
    out.push_str("</div>\n");
    // What has already been answered, with who gave it and when: the response
    // is recorded when it is given and applied on the next tick, so a reload
    // shows it before the decision is retracted (153, 154, 310).
    out.push_str(&answered(read, decision.number));
    out.push_str("</article>\n");
    out
}

/// One answer, as a control the operator uses (311, 193, S6).
///
/// A bare answer is one tap. An answer whose pattern takes an argument —
/// `redo: <notes>`, `bolt <name>`, `<intent>: drop` — takes the argument here,
/// in a field beside the control, and the pattern and the text are posted
/// together for the catalogue to fill in. Rendering the pattern as the value of
/// a button posted `redo: <notes>` literally: the operator could not say what
/// to redo, what bolt to route to or what type to set, which is six of the nine
/// controls on a unit's card and every way of sending work back. A long-form
/// answer is given with the platform's own keyboard, which is what the field is
/// (311).
fn control(number: &str, answer: &str) -> String {
    let tool = crate::catalogue::ANSWER;
    let Some(argument) = takes_an_argument(answer) else {
        // One tap, and nothing behind a hover or a keyboard (311). The control
        // posts to the one tool the chat's numbered reply grammar calls, so the
        // two surfaces share a write path (193, 194).
        return format!(
            "<form method=\"post\" action=\"/api/tools/{tool}\" class=\"answer\">\n\
             <input type=\"hidden\" name=\"decision\" value=\"{number}\">\n\
             <input type=\"hidden\" name=\"answer\" value=\"{0}\">\n\
             <button type=\"submit\" class=\"btn sm\" data-answer=\"{0}\">{0}</button>\n</form>\n",
            escape(answer),
        );
    };
    // `required` is what keeps an empty field from being sent as an answer the
    // machine matches nothing against: the browser's own refusal, with no
    // script behind it (310, 311).
    let field = format!("a{number}-{}", argument.replace([' ', ':'], "-"));
    format!(
        "<form method=\"post\" action=\"/api/tools/{tool}\" class=\"answer takes-text\">\n\
         <input type=\"hidden\" name=\"decision\" value=\"{number}\">\n\
         <input type=\"hidden\" name=\"answer\" value=\"{whole}\">\n\
         <label class=\"sr-only\" for=\"{field}\">{argument} for decision {number}</label>\n\
         <input type=\"text\" id=\"{field}\" name=\"text\" placeholder=\"{argument}\" \
         autocomplete=\"off\" required>\n\
         <button type=\"submit\" class=\"btn sm\" data-answer=\"{whole}\">{said}</button>\n\
         </form>\n",
        argument = escape(&argument),
        whole = escape(answer),
        said = escape(&said(answer)),
    )
}

/// What an answer's argument is called, where it takes one: `redo: <notes>` is
/// `notes`, `bolt <name>` is `name` and `<intent>: drop` is `intent`. `None`
/// for an answer that is one word and one tap.
fn takes_an_argument(answer: &str) -> Option<String> {
    let (_, rest) = answer.split_once('<')?;
    let (argument, _) = rest.split_once('>')?;
    match argument.is_empty() {
        true => None,
        false => Some(argument.to_string()),
    }
}

/// What the control says on it: the answer's own words, without the argument
/// the field beside it takes. `redo: <notes>` reads `redo` and `<intent>: drop`
/// reads `drop`.
fn said(answer: &str) -> String {
    let (before, rest) = answer.split_once('<').unwrap_or((answer, ""));
    let after = rest.split_once('>').map(|(_, after)| after).unwrap_or("");
    format!("{before}{after}")
        .replace(':', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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
        escape(&row.said)
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
             data-answerable=\"false\" data-opened=\"{opened}\">\n",
            escape(&object.id),
            opened = read.opened.as_deref() == Some(object.id.as_str()),
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
        out.push_str(&dock_answers(read, &object.id));
        out.push_str("</article>\n");
    }
    out
}

/// The foot of one dock surface: the object's answers where a decision stands
/// on it, and what there is to answer and why not where none does (S27, 308).
///
/// Every link the machinery writes opens the object in the dock "with its
/// answer controls in reach" (308), and on a phone the dock is the whole
/// screen — so a footer that only said answering happens on the rail sent the
/// operator back to hunt for the card the link had just brought them to.
fn dock_answers(read: &Read, object: &str) -> String {
    let standing: Vec<&DecisionInstance> = read
        .decisions
        .iter()
        .filter(|d| d.object == object)
        .collect();
    if standing.is_empty() {
        return String::from(
            "<div class=\"dk-answers none\" data-answerable=\"false\">\
             <span class=\"fl\">answers</span>\
             <span class=\"r\">nothing stands on this object; the machinery takes it from \
             here and raises the next decision that is yours (13)</span></div>\n",
        );
    }
    let mut out = String::from("<div class=\"dk-answers\" data-answerable=\"true\">\n");
    for decision in standing {
        let number = decision.number.map(|n| n.to_string()).unwrap_or_default();
        let _ = write!(
            out,
            "<div class=\"answers\" data-number=\"{number}\">\
             <span class=\"n number\">{number}</span>\n"
        );
        for answer in &decision.answers {
            out.push_str(&control(&number, answer));
        }
        out.push_str("</div>\n");
        out.push_str(&answered(read, decision.number));
    }
    out.push_str("</div>\n");
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

pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ------------------------------------------------------------------ the tour

/// The overlay of an instance standing part-way through a scenario
/// (`design/flywheel-next/scenarios/storefront.md`).
///
/// It is part of the product and not a mode: what puts it here is a fact in
/// the state store, so an instance a scenario was applied into onboards
/// whoever is looking at it and every other instance has no overlay at all.
/// Nothing turns it on and nothing turns it off — when the last action is
/// played there is nothing left to say and the strip is gone.
///
/// One click is one action. The control writes that the next action is owed
/// and the host's loop plays it, so the request the operator is waiting on
/// never runs a cascade of its own (D11).
fn tour(read: &Read) -> String {
    let Some(tour) = &read.tour else {
        return String::new();
    };
    if !tour.standing() {
        return String::new();
    }
    let working = tour.working();
    let step = match working {
        true => tour.played + 1,
        false => tour.played,
    };
    // What the operator is being told: while the machinery is working, the
    // action in flight; otherwise what the board in front of them is showing,
    // and the first time round, nothing has happened yet.
    let said = match (working, &tour.next, &tour.said) {
        (true, Some(next), _) => next.clone(),
        (true, None, _) => "the agent is working".to_string(),
        (false, _, Some(said)) => said.clone(),
        (false, _, None) => tour.title.clone(),
    };
    let control = match working {
        // The beat: the machinery has stalled where a session would be
        // working, and this is the viewer seeing that before the artifact
        // appears. The document asks for itself again when it is over, so
        // nothing here is client state a reload loses (310).
        true => "<span class=\"work\"><i></i><i></i><i></i> the agent is working</span>"
            .to_string(),
        false => format!(
            "<form method=\"post\" action=\"/tour/next\">\
               <button class=\"btn pri\" type=\"submit\">{}</button>\
             </form>",
            match tour.played {
                0 => "start",
                _ => "next",
            }
        ),
    };
    format!(
        "<div class=\"tour{}\" id=\"tour\" aria-live=\"polite\">\
           <span class=\"n\">step <b>{step}</b> of <b>{}</b></span>\
           <span class=\"said\">{}</span>{control}\
         </div>",
        match working {
            true => " working",
            false => "",
        },
        tour.actions,
        escape(&said),
    )
}

/// What the document asks of itself while the beat runs: itself again, once
/// the machinery has had its two seconds. A page with no script says this in
/// the one place a document can (310).
fn tour_head(read: &Read) -> String {
    let Some(tour) = &read.tour else {
        return String::new();
    };
    if !tour.standing() || !tour.working() {
        return String::new();
    }
    // Every second while the machinery owes the action, which is at most one
    // ask more than the beat needs. A refresh that arrives before the loop has
    // played it simply asks again, so nothing here has to know how long is
    // left and no clock is read on the page.
    "<meta http-equiv=\"refresh\" content=\"1\">".to_string()
}
