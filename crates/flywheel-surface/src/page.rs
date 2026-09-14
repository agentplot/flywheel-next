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
    /// The built repositories the instance tracks, which is where a unit
    /// lands: one, and the `unit` control on a capture never asks; several,
    /// and it does (34, 205, 206).
    pub repositories: Vec<String>,
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
    /// What each object's sessions left behind: a file at a path, which the
    /// page links to (190, 213). Without it the operator sees that an
    /// elaboration is done and cannot read what it produced.
    pub delivered: BTreeMap<String, Vec<Delivered>>,
    /// The deliverable the request asked to read, rendered into the page: a
    /// markdown document is read here, beside the object it belongs to. An HTML
    /// deliverable is never here — it is served at its own address (310).
    pub reading: Option<Reading>,
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

/// A deliverable open on the page: the document, rendered, with the object it
/// belongs to named beside it (190, 213, S27).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reading {
    /// The object whose session delivered it, so the surface says what this is
    /// the output of and gets back to it.
    pub object: String,
    /// `<repository>/<path>`, which is what the link named.
    pub named: String,
    /// The document, rendered.
    pub body: String,
}

/// One file a session left behind: a deliverable, as the exit that reported it
/// named it (190, 213).
///
/// A deliverable is recorded as `<repository>/<path>`, which is where the
/// session's place actually wrote it, so the page has everything it needs to
/// open it: the repository to read from and the path within it. Nothing here is
/// a rendering — the file is read when the operator asks for it, not on every
/// request (15, 310).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivered {
    pub repository: String,
    pub path: String,
    /// The session that reported it, and when it did.
    pub session: String,
    pub at: String,
}

impl Delivered {
    /// The `<repository>/<path>` the exit recorded, which is the name the page
    /// links by and the one a request is checked against.
    pub fn named(&self) -> String {
        format!("{}/{}", self.repository, self.path)
    }

    /// What the file is called, which is what a list of them reads by.
    pub fn file(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// Whether this is a document the page renders into itself, or one served
    /// at its own address. An HTML deliverable is a document with its own
    /// styles and its own head; rendering it inside the page would be putting a
    /// file's markup into the page that shows it, which is the one thing the
    /// renderer will not do (310).
    pub fn is_its_own_document(&self) -> bool {
        let path = self.path.to_ascii_lowercase();
        path.ends_with(".html") || path.ends_with(".htm")
    }
}

/// Every deliverable of every session on an object, newest last, by the object
/// the session ran on (190, 213).
///
/// A session's fact is `fact/session/<object>/<n>`, so the object a session ran
/// on is the prefix of its id; what it delivered is on the `exit` entry of its
/// thread, which is what `flywheel exit done --deliverable <path>` writes (67).
fn delivered_by_object<S: StateStore>(
    store: &S,
    objects: &[Object],
) -> BTreeMap<String, Vec<Delivered>> {
    let mut out: BTreeMap<String, Vec<Delivered>> = BTreeMap::new();
    for fact in objects.iter().filter(|o| o.id.starts_with(SESSION_FACT)) {
        let session = fact.id.trim_start_matches(SESSION_FACT).to_string();
        // The object is the session id without the attempt under it, and it is
        // matched against the objects the same read holds rather than guessed
        // from the shape of the id.
        let Some(object) = objects
            .iter()
            .filter(|o| session.starts_with(&format!("{}/", o.id)))
            .max_by_key(|o| o.id.len())
            .map(|o| o.id.clone())
        else {
            continue;
        };
        let Ok(thread) = store.thread(&session) else {
            continue;
        };
        for entry in thread.iter().filter(|e| e.kind == "exit") {
            let named: Vec<String> = entry
                .fields
                .get("deliverables")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or_default();
            for name in named {
                // `<repository>/<path>`. A deliverable recorded as a bare name
                // — the kind from the type's table rather than a file — names
                // no repository and is not something the page can open, so it
                // is left to the record section that already says it.
                let Some((repository, path)) = name.split_once('/') else {
                    continue;
                };
                if repository.is_empty() || path.is_empty() {
                    continue;
                }
                out.entry(object.clone()).or_default().push(Delivered {
                    repository: repository.to_string(),
                    path: path.to_string(),
                    session: session.clone(),
                    at: entry.at.to_rfc3339(),
                });
            }
        }
    }
    out
}

/// Where a session's fact is kept, which is what its id is read off.
const SESSION_FACT: &str = "fact/session/";

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
    let delivered = delivered_by_object(store, &objects);
    // What curation has to judge, and the session the operator judges it in
    // (110, 118, 93b).
    let unmoved = signals::unmoved(&files);
    let curation = curating(&objects);
    let intents: Vec<String> = objects
        .iter()
        .filter(|o| o.machine == "intent")
        .map(|o| o.id.clone())
        .collect();
    // The built repositories alone: the state and the blueprints are the
    // machinery's own, and nothing lands a unit on them (205, 206).
    let repositories: Vec<String> = world
        .repositories()
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.name)
        .filter(|name| name != "flywheel-state" && name != "flywheel-blueprints")
        .collect();
    Ok(Read {
        decisions,
        status,
        objects,
        address: address.to_string(),
        operator: operator.to_string(),
        repositories,
        away,
        weight,
        answered,
        unmoved,
        curation,
        intents,
        why,
        delivered,
        reading: None,
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
    ("{{LANE_OPERATION}}", &["sink", "landed"]),
];

/// The lane's own title and what it holds, in the mockup's order.
const LANE_HEADS: [(&str, &str); 4] = [
    ("Inception", ""),
    ("Bolt plan", ""),
    ("Construction", ""),
    ("Operation", ""),
];

/// What a lane says when it holds nothing: where the next thing comes from,
/// in the operator's terms, and no more.
const LANE_EMPTY: [(&str, &str); 4] = [
    ("Inception", "Nothing captured yet. Type what you noticed in the box above."),
    ("Bolt plan", "No proposals yet."),
    ("Construction", "No bolts yet. A unit on a capture starts one."),
    ("Operation", "Nothing running."),
];

/// The machinery's own objects, which the hosts strip and the header carry
/// and the lanes do not: an instance and a curation are not work a person
/// follows on the board (141, D16).
const OFF_THE_BOARD: [&str; 4] = ["instance", "curation", "host", "rail"];

/// Render the whole page. One document, one request, nothing stored.
/// The instance this page is of: the last segment of the host's own address,
/// which is where every link the machinery writes puts it (205a, 308).
fn instance_of(address: &str) -> &str {
    address.trim_end_matches('/').rsplit('/').next().unwrap_or_default()
}

/// The two faces the mockup names, as `@font-face` rules over data: the
/// latin variable-weight files, embedded so the bundle fetches nothing from
/// anywhere else (310, D16; `page/fonts/NOTICE`). Encoded once per process.
fn fonts_css() -> &'static str {
    static CSS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CSS.get_or_init(|| {
        use base64::Engine;
        let face = |family: &str, weights: &str, bytes: &[u8]| {
            format!(
                "@font-face{{font-family:\"{family}\";font-style:normal;font-weight:{weights};\
                 font-display:swap;src:url(data:font/woff2;base64,{}) format(\"woff2\")}}\n",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            )
        };
        face("Manrope", "200 800", include_bytes!("page/fonts/manrope-latin.woff2"))
            + &face("JetBrains Mono", "100 800", include_bytes!("page/fonts/jetbrains-mono-latin.woff2"))
    })
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
        .replace("{{FONTS}}", fonts_css())
        .replace("{{INSTANCE}}", &escape(instance))
        .replace("{{OPERATOR}}", &escape(&read.operator))
        .replace("{{CLOCK}}", &escape(&read.status.at.format("%H:%M").to_string()))
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
    // Every host the instance has, as a chip: its name, a dot for whether it
    // is heard from, and what it is running. A host past its stale window
    // carries that on the chip (141, 143, 146, 150a).
    let mut names: Vec<String> = read
        .objects
        .iter()
        .filter(|o| o.machine == "host")
        .map(|o| o.id.trim_start_matches("host/").to_string())
        .collect();
    for row in &read.status.rows {
        if let Some(holder) = &row.holder {
            if !names.contains(holder) {
                names.push(holder.clone());
            }
        }
    }
    names.sort();
    let mut out = String::new();
    if !names.is_empty() {
        out.push_str("<span class=\"lab\">hosts</span>");
    }
    for host in &names {
        let liveness = read
            .status
            .rows
            .iter()
            .filter(|r| r.holder.as_deref() == Some(host))
            .find_map(|r| r.liveness.clone())
            .or_else(|| read.away.values().find(|a| &a.host == host).map(|_| "gone".to_string()))
            .unwrap_or_else(|| "alive".into());
        let running = read
            .status
            .rows
            .iter()
            .filter(|r| r.holder.as_deref() == Some(host) && r.runner.is_some())
            .count();
        let since = read
            .away
            .values()
            .find(|away| &away.host == host)
            .map(|away| format!(" since {}", short(&away.since.to_rfc3339())))
            .unwrap_or_default();
        let said = match (liveness.as_str(), running) {
            ("alive", 0) => "idle".to_string(),
            ("alive", 1) => "1 session".to_string(),
            ("alive", n) => format!("{n} sessions"),
            (other, _) => format!("{other}{since}"),
        };
        let _ = write!(
            out,
            "<span class=\"host{gone}\" data-host=\"{h}\" data-liveness=\"{l}\">\
             <span class=\"dot {l}\"></span><b class=\"hn\">{h}</b><span class=\"hm\">{said}</span></span>",
            gone = match liveness.as_str() {
                "alive" => "",
                _ => " gone",
            },
            h = escape(host),
            l = escape(&liveness),
            said = escape(&said),
        );
    }
    if !read.repositories.is_empty() {
        out.push_str("<span class=\"lab\">repos</span>");
        for repository in &read.repositories {
            let _ = write!(out, "<span class=\"repo\">{}</span>", escape(repository));
        }
    }
    out
}

/// The rail: every standing decision with its number and its answers, one tap
/// each, in the groups the model folds them into (15, 11, 311).
fn rail(read: &Read) -> String {
    let mut out = String::from(
        "<div class=\"rail-h\"><h2>Decisions</h2>\
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
            "<div class=\"empty\">Nothing to decide.</div>\n",
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
    // The object it concerns, as the board draws it: the repository it is in,
    // greyed, and the name. The whole id is what the link carries and what the
    // dock says; a card is read for which thing this is, and an id repeated
    // down the rail is the same four words over and over (209, 308).
    let _ = write!(
        out,
        "<div class=\"title\"><a class=\"object\" href=\"{link}\" data-object=\"{object}\">{pre}{name}</a></div>\n",
        link = escape(&link),
        object = escape(&decision.object),
        pre = repository_of(&decision.object)
            .map(|r| format!("<span class=\"pre\">{} · </span>", escape(r)))
            .unwrap_or_default(),
        name = escape(name_of(&decision.object)),
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
        out.push_str(&card_control(&number, answer, &decision.object));
    }
    out.push_str("</div>\n");
    // What has already been answered, with who gave it and when: the response
    // is recorded when it is given and applied on the next tick, so a reload
    // shows it before the decision is retracted (153, 154, 310).
    out.push_str(&answered(read, decision.number));
    out.push_str("</article>\n");
    out
}

/// An answer on the rail's card. A bare answer is the one-tap control; an
/// answer that takes an argument — `redo: <notes>`, `bolt <name>`, `type
/// <name>` — is one tap too, opening the object in the dock where the field
/// for it is (311, S6, D16). A card with six text fields on it read as a form
/// and not as a decision; the mockup keeps the card to its words and takes
/// the argument in a box the tap opens.
fn card_control(number: &str, answer: &str, object: &str) -> String {
    if takes_an_argument(answer).is_none() {
        return control(number, answer);
    }
    format!(
        "<a class=\"btn sm to-dock\" href=\"#dock-{object}\" data-answer=\"{whole}\" \
         data-decision=\"{number}\">{said}…</a>\n",
        object = escape(object),
        whole = escape(answer),
        said = escape(&said(answer)),
    )
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
        "<form method=\"post\" action=\"/api/tools/{tool}\" class=\"answer takes-text{in_words}\">\n\
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
        // A whole-answer field is a sentence and is given the room for one.
        in_words = match is_all_argument(answer) {
            true => " in-words",
            false => "",
        },
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
///
/// An answer that is *nothing but* its argument reads `reply`. A question's one
/// answer is `<text>` — the operator answers it in a sentence — and stripping
/// the argument left the control with no word on it at all: a box the operator
/// could see and not read. `reply` is the action it performs and the word the
/// chat's own grammar uses for it (S30, 311, 194).
fn said(answer: &str) -> String {
    let (before, rest) = answer.split_once('<').unwrap_or((answer, ""));
    let after = rest.split_once('>').map(|(_, after)| after).unwrap_or("");
    let words = format!("{before}{after}")
        .replace(':', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    match words.is_empty() {
        true => "reply".to_string(),
        false => words,
    }
}

/// Whether an answer is nothing but the words the operator writes — a
/// question's `<text>` — rather than a control with an argument beside it. Such
/// an answer is a sentence and is given the room for one (311).
fn is_all_argument(answer: &str) -> bool {
    takes_an_argument(answer).is_some_and(|argument| {
        answer.trim() == format!("<{argument}>")
    })
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

/// A commit's mark as a person reads it: its first seven characters, with the
/// whole on hover (D12).
fn short_mark(mark: &str) -> String {
    mark.chars().take(7).collect()
}

/// The board's header: what the read is as of, which is what the committed
/// projection states too (145, D12).
fn board_header(read: &Read) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "<span class=\"as-of\" title=\"{}\">as of commit {} · {}</span>",
        escape(&read.status.as_of.mark),
        escape(&short_mark(&read.status.as_of.mark)),
        escape(&read.status.as_of.at.format("%H:%M UTC").to_string())
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
///
/// An object that hangs off another is drawn inside it rather than beside it:
/// an intent is a thread with its elaborations as beads, a bolt is a ledger
/// with its units as a chain, and a unit carries the work items running on it
/// (209, S13, S15). A child drawn inside its parent is not also a row of its
/// own, or the board would say the same thing twice and the thread would be a
/// list of names with nothing on it.
fn lane(read: &Read, title: &str, sub: &str, machines: &[&str]) -> String {
    let mut out = String::new();
    let mine: Vec<&status::Row> = read
        .status
        .rows
        .iter()
        .filter(|row| !OFF_THE_BOARD.contains(&row.machine.as_str()) && in_lane(&row.machine, machines))
        .collect();
    let _ = write!(
        out,
        "<div class=\"lane-h\"><h2 class=\"lane-title\">{}</h2>{}<span class=\"n\">{}</span></div>\n",
        escape(title),
        match sub.is_empty() {
            true => String::new(),
            false => format!("<span class=\"sub\">{}</span>", escape(sub)),
        },
        mine.len()
    );
    // The unmoved signals belong to inception, where curation reads them, and
    // none of them is discarded (118, 110).
    if machines.contains(&"signal") {
        out.push_str(&unmoved(read));
    }
    out.push_str("<div class=\"status-groups\">\n");
    let mut drawn = 0;
    for group in status::GROUPS {
        let rows: Vec<&&status::Row> = mine
            .iter()
            .filter(|row| row.group == group && !nested_in_its_parent(read, row, &mine))
            .collect();
        // A group with nothing in it keeps its heading and says nothing
        // under it: four "nothing"s down a lane read as the machine talking to
        // itself, and the grouping is what 141 asks for (141, D16).
        drawn += rows.len();
        let _ = write!(
            out,
            "<section class=\"sec-h-group{empty}\" data-group=\"{}\">\n<h2>{group}</h2>\n",
            group.replace(' ', "-"),
            empty = match rows.is_empty() {
                true => " empty",
                false => "",
            },
        );
        for row in rows {
            out.push_str(&object_on_the_board(read, row, &mine));
        }
        out.push_str("</section>\n");
    }
    if drawn == 0 {
        let said = LANE_EMPTY
            .iter()
            .find(|(t, _)| *t == title)
            .map(|(_, s)| *s)
            .unwrap_or("Nothing here.");
        let _ = write!(out, "<div class=\"quiet empty\">{}</div>\n", escape(said));
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

/// Whether this object is drawn inside another on the same lane, and so is not
/// a row of its own: an elaboration on its intent's thread, a unit on its
/// bolt's ledger, a work item under the unit it runs on (209, S13, S15).
fn nested_in_its_parent(read: &Read, row: &status::Row, lane: &[&status::Row]) -> bool {
    let Some(parent) = parent_of(read, &row.object) else {
        return false;
    };
    lane.iter()
        .any(|held| held.object == parent && nests(&held.machine, &row.machine))
}

/// Which kind draws which inside itself.
fn nests(parent: &str, child: &str) -> bool {
    matches!(
        (parent, child),
        ("intent", "elaboration") | ("bolt", "unit") | ("unit", "work-item") | ("unit", "work item")
    )
}

/// The object this one hangs off, as the store records it.
fn parent_of<'a>(read: &'a Read, object: &str) -> Option<&'a str> {
    read.objects
        .iter()
        .find(|o| o.id == object)
        .and_then(|o| o.parent.as_deref())
}

/// The rows drawn inside this one, in the order the store holds them, which is
/// the order the operator reads a thread or a chain in (209).
fn children_of<'a>(
    read: &Read,
    row: &status::Row,
    lane: &'a [&'a status::Row],
) -> Vec<&'a status::Row> {
    lane.iter()
        .filter(|held| {
            nests(&row.machine, &held.machine)
                && parent_of(read, &held.object) == Some(row.object.as_str())
        })
        .copied()
        .collect()
}

/// What an object is called, without the machine and the repository the id
/// carries to make it unique: `intent/atlas-provider-limits` is
/// `atlas-provider-limits`, `bolt/atlas/plan-rows` is `plan-rows`. The board
/// draws the name, because the operator reads a lane for what is in it and not
/// for the path that addresses it; the id itself is one tap away in the dock.
pub(crate) fn name_of(object: &str) -> &str {
    object.rsplit('/').next().unwrap_or(object)
}

/// The repository an object belongs to, where its id carries one:
/// `bolt/atlas/plan-rows` is atlas's. A ledger says it, because a bolt is read
/// against the repository it is building (S15).
pub(crate) fn repository_of(object: &str) -> Option<&str> {
    let mut segments = object.split('/');
    let _machine = segments.next()?;
    let repository = segments.next()?;
    segments.next().map(|_| repository)
}

/// The attributes every object on the board carries, whatever its silhouette:
/// which host holds it, which runs it, and whether that host is alive (141).
fn board_attributes(row: &status::Row) -> String {
    format!(
        " id=\"{object}\" data-machine=\"{machine}\" data-holder=\"{holder}\" \
         data-runner=\"{runner}\" data-liveness=\"{liveness}\"",
        object = escape(&row.object),
        machine = escape(&row.machine),
        holder = escape(row.holder.as_deref().unwrap_or("none")),
        runner = escape(row.runner.as_deref().unwrap_or("none")),
        liveness = escape(row.liveness.as_deref().unwrap_or("none")),
    )
}

/// Who is holding an object and who is running it, in one line: the two things
/// 141 asks for per object, said rather than printed as fields.
fn held_by(row: &status::Row) -> String {
    let mut said = Vec::new();
    if let Some(holder) = &row.holder {
        said.push(match &row.liveness {
            Some(liveness) => format!("held by {holder} ({liveness})"),
            None => format!("held by {holder}"),
        });
    }
    if let Some(runner) = &row.runner {
        said.push(format!("run by the {runner}"));
    }
    said.join(" · ")
}

/// What an object is doing, split into the one word a head has room for and
/// the rest of it.
///
/// `said` is every live region of the machine, joined — "open · citations moved
/// · close not-offered · line current · place ready · services declared" for a
/// bolt. The first of those is the object's own life, which is what a head
/// states; the rest belongs under it, where it can wrap. Running the whole
/// string into a head that is `nowrap` by design pushed it off the lane.
pub(crate) fn state_and_rest(said: &str) -> (&str, &str) {
    match said.split_once(" \u{b7} ") {
        Some((state, rest)) => (state, rest),
        None => (said, ""),
    }
}

/// The question, the answer and the note kept on an object, which stay with it
/// once the session that said them is gone (144).
fn discussion(row: &status::Row) -> String {
    let mut out = String::new();
    for (kind, said) in &row.discussion {
        let _ = write!(
            out,
            "<p class=\"said\" data-kind=\"{}\">{}</p>\n",
            escape(kind),
            escape(said)
        );
    }
    out
}

/// One object on the board, in the silhouette its kind has: never one card
/// class with variants (209, D16, the mockup's own rule).
///
/// 209 names the forms and this writes each one's own markup rather than one
/// body under a different class: a proposal is a document, an intent a thread
/// with its elaborations in order, a bolt a ledger with its units in order, a
/// signal a quote. A form that differs only by the word on its class is one
/// card with variants, which is what 209 refuses.
/// The standing decisions on an object, as marks on its board form: the
/// mockup's rule that every object carries a mark per decision that waits on
/// it, so the board says where the rail's numbers belong (D16, 15, 18). Each
/// is one tap to the dock, where the answer is.
fn marks(read: &Read, object: &str) -> String {
    let standing: Vec<&DecisionInstance> = read
        .decisions
        .iter()
        .filter(|d| d.object == object && d.number.is_some())
        .collect();
    if standing.is_empty() {
        return String::new();
    }
    let mut out = String::from("<div class=\"marks\">");
    for decision in standing {
        let sign = match decision.group.as_str() {
            "approve" => "✓",
            "decide" => "?",
            "attention" => "!",
            _ => "✎",
        };
        let _ = write!(
            out,
            "<a class=\"mark {group}\" href=\"#dock-{object}\" data-mark=\"{number}\" title=\"{kind}\">\
             {sign} {number} <span>{kind}</span></a>",
            group = escape(&decision.group),
            object = escape(object),
            number = decision.number.unwrap_or_default(),
            kind = escape(&decision.kind),
        );
    }
    out.push_str("</div>\n");
    out
}

/// A form with its marks inside it, before its closing tag.
fn with_marks(html: String, marks: String) -> String {
    if marks.is_empty() {
        return html;
    }
    let close = html.trim_end();
    let at = close
        .rfind("</article>")
        .or_else(|| close.rfind("</div>"))
        .unwrap_or(close.len());
    format!("{}{}{}\n", &close[..at], marks, &close[at..])
}

/// A landed bolt: the mockup's record — the name stamped landed, what it
/// carried, and what is left of it — a form of its own and not a ledger with
/// nothing building (209, S15).
fn record(read: &Read, row: &status::Row, lane: &[&status::Row]) -> String {
    let (state, rest) = state_and_rest(&row.said);
    let units = children_of(read, row, lane);
    let mut out = String::new();
    let _ = write!(out, "<article class=\"record\"{}>\n", board_attributes(row));
    let _ = write!(
        out,
        "<div class=\"rc-head\"><span class=\"rc-name\"><a href=\"#dock-{object}\">{repo}{name}</a></span>\
         <span class=\"rc-at\">{state}</span></div>\n",
        object = escape(&row.object),
        repo = repository_of(&row.object).map(|r| format!("{}/", escape(r))).unwrap_or_default(),
        name = escape(name_of(&row.object)),
        state = escape(state),
    );
    let _ = write!(
        out,
        "<span class=\"rc-stamp\">{state} · {how_many} {units}</span>\n",
        state = escape(state),
        how_many = units.len(),
        units = counted("units", units.len()),
    );
    if !units.is_empty() {
        out.push_str("<div class=\"rc-env\">");
        for unit in &units {
            let _ = write!(
                out,
                "<a href=\"#dock-{object}\">{name}</a>",
                object = escape(&unit.object),
                name = escape(name_of(&unit.object)),
            );
        }
        out.push_str("</div>\n");
    }
    out.push_str(&discussion(row));
    if !rest.is_empty() {
        let _ = write!(out, "<div class=\"leave\">{}</div>\n", escape(rest));
    }
    out.push_str("</article>\n");
    out
}

fn object_on_the_board(read: &Read, row: &status::Row, lane: &[&status::Row]) -> String {
    let drawn = draw_on_the_board(read, row, lane);
    with_marks(drawn, marks(read, &row.object))
}

fn draw_on_the_board(read: &Read, row: &status::Row, lane: &[&status::Row]) -> String {
    if row.machine == "bolt" && row.group == "done" {
        return record(read, row, lane);
    }
    match silhouette(&row.machine) {
        // An elaboration whose intent is not on this lane is a bead with no
        // thread to hang on, and is drawn as the bead alone: drawing it as a
        // thread of its own would say it is an intent, which is the one thing
        // 209 forbids a form to do.
        "thread bead" => format!("<div class=\"thread\">{}</div>\n", bead(row)),
        "thread" => thread(read, row, lane),
        "ledger" => ledger(read, row, lane),
        "sheet" => sheet(row),
        "slip" => slip(read, row, lane),
        "quote" => quote(read, row),
        _ => sessrow(row),
    }
}

/// An intent: a line off the books, drawn as a stroke with its elaborations as
/// beads in order, no box (209, S13).
fn thread(read: &Read, row: &status::Row, lane: &[&status::Row]) -> String {
    let mut out = String::new();
    let (state, rest) = state_and_rest(&row.said);
    let held = held_by(row);
    let _ = write!(out, "<article class=\"thread\"{}>\n", board_attributes(row));
    let _ = write!(
        out,
        "<div class=\"th-head\"><span class=\"th-k\">{machine}</span>\
         <a class=\"th-name\" href=\"#dock-{object}\">{name}</a>\
         <span class=\"th-state\">{state}</span></div>\n",
        machine = escape(&row.machine),
        object = escape(&row.object),
        name = escape(name_of(&row.object)),
        state = escape(state),
    );
    out.push_str("<div class=\"th-line\">\n");
    let note: Vec<&str> = [rest, held.as_str()]
        .into_iter()
        .filter(|said| !said.is_empty())
        .collect();
    if !note.is_empty() {
        let _ = write!(out, "<div class=\"th-note\">{}</div>\n", escape(&note.join(" · ")));
    }
    out.push_str(&discussion(row));
    let beads = children_of(read, row, lane);
    if beads.is_empty() {
        out.push_str("<div class=\"bead queued\"><span class=\"leave\">no elaboration yet</span></div>\n");
    }
    for bead in beads {
        out.push_str(&self::bead(bead));
    }
    out.push_str("</div>\n</article>\n");
    out
}

/// One elaboration, as a bead on its intent's thread. Each bead is its own
/// target and opens that elaboration's surface (210, S13).
fn bead(row: &status::Row) -> String {
    format!(
        "<div class=\"bead {state}\"{attributes}>\
         <span class=\"bk\">{machine}</span>\
         <a class=\"en\" href=\"#dock-{object}\">{name}</a>\
         <span class=\"bs\">{said}</span></div>\n",
        state = escape(&bead_state(row)),
        attributes = board_attributes(row),
        machine = escape(&row.machine),
        object = escape(&row.object),
        name = escape(name_of(&row.object)),
        said = escape(&row.said),
    )
}

/// What a bead looks like from its group: the mockup gives a working bead a
/// filled dot, an idle one a hollow one, a queued one a dashed ring.
fn bead_state(row: &status::Row) -> String {
    match row.group.as_str() {
        "queued" => "queued".into(),
        "done" => "done".into(),
        "in progress" => "working".into(),
        _ => "idle".into(),
    }
}

/// A bolt: a flat ledger with its units as a left-to-right chain, merged,
/// building and queued, and the work items running under the building one
/// (209, S15).
fn ledger(read: &Read, row: &status::Row, lane: &[&status::Row]) -> String {
    let mut out = String::new();
    let (state, rest) = state_and_rest(&row.said);
    let units = children_of(read, row, lane);
    let _ = write!(out, "<article class=\"ledger\"{}>\n", board_attributes(row));
    let _ = write!(
        out,
        "<div class=\"lg-head\"><a class=\"lg-name\" href=\"#dock-{object}\">{name}</a>",
        object = escape(&row.object),
        name = escape(name_of(&row.object)),
    );
    if let Some(repository) = repository_of(&row.object) {
        let _ = write!(out, "<span class=\"lg-repo\">{}</span>", escape(repository));
    }
    // The head says the bolt's own state and how much is on it. The rest of
    // what it is doing goes to the foot, where it can wrap: a head is `nowrap`
    // by design and the whole of `said` ran off the lane.
    let _ = write!(
        out,
        "<span class=\"lg-run\">{state} · {how_many} {units}</span></div>\n",
        state = escape(state),
        how_many = units.len(),
        units = counted("units", units.len()),
    );
    out.push_str("<div class=\"lg-chain\">\n");
    if units.is_empty() {
        out.push_str("<span class=\"lg-unit queued\">no unit yet</span>\n");
    }
    for (at, unit) in units.iter().enumerate() {
        if at > 0 {
            out.push_str("<span class=\"lg-arrow\">→</span>\n");
        }
        out.push_str(&lg_unit(read, unit, lane));
    }
    out.push_str("</div>\n");
    out.push_str(&discussion(row));
    // What else the bolt is doing, and who is holding it: at the foot, where
    // the line wraps and the head does not (141, S15).
    let mut said = Vec::new();
    if !rest.is_empty() {
        said.push(rest.to_string());
    }
    let held = held_by(row);
    if !held.is_empty() {
        said.push(held);
    }
    if !said.is_empty() {
        let _ = write!(
            out,
            "<div class=\"lg-foot\"><span>{}</span></div>\n",
            escape(&said.join(" · "))
        );
    }
    out.push_str("</article>\n");
    out
}

/// One unit in a bolt's chain, in the state the chain draws it in: merged is
/// ticked and quiet, queued is a dashed outline, and the one being built is
/// opened out with what it is doing and the work items running on it (S15).
fn lg_unit(read: &Read, row: &status::Row, lane: &[&status::Row]) -> String {
    let state = match row.group.as_str() {
        "done" => "merged",
        "queued" => "queued",
        _ => "building",
    };
    let mut out = String::new();
    let _ = write!(
        out,
        "<a class=\"lg-unit {state}\" href=\"#dock-{object}\"{attributes}>",
        object = escape(&row.object),
        attributes = board_attributes(row),
    );
    // A merged or queued unit is its name alone: the chain is read for its
    // shape, and only the unit being built is worth the room its state takes.
    match state {
        "building" => {
            let _ = write!(
                out,
                "<span class=\"lg-un\">{name}</span><span class=\"lg-ut\">{said}</span>",
                name = escape(name_of(&row.object)),
                said = escape(&row.said),
            );
        }
        _ => {
            let _ = write!(out, "{}", escape(name_of(&row.object)));
        }
    }
    let items = children_of(read, row, lane);
    if !items.is_empty() {
        out.push_str("<span class=\"lg-sess\">");
        for item in items {
            let _ = write!(
                out,
                "<span class=\"it\"><span class=\"wi\">{name}</span>\
                 <span class=\"sc\"><span>{said}</span></span></span>",
                name = escape(name_of(&item.object)),
                said = escape(&item.said),
            );
        }
        out.push_str("</span>");
    }
    out.push_str("</a>\n");
    out
}

/// A proposal: a document, taller than wide, with a title line, its metadata
/// in mono and what a yes does at the foot (209, S14).
fn sheet(row: &status::Row) -> String {
    let mut out = String::new();
    let _ = write!(out, "<article class=\"sheet\"{}>\n", board_attributes(row));
    let _ = write!(
        out,
        "<span class=\"sh-k\">{machine}</span>\
         <div class=\"sh-title\"><a href=\"#dock-{object}\">{name}</a></div>\
         <div class=\"sh-meta\">{said}</div>\n",
        machine = escape(&row.machine),
        object = escape(&row.object),
        name = escape(name_of(&row.object)),
        said = escape(&row.said),
    );
    out.push_str(&discussion(row));
    let _ = write!(
        out,
        "<div class=\"sh-foot\">{}</div>\n</article>\n",
        escape(&held_by(row))
    );
    out
}

/// A unit that hangs off no ledger — a baseline, or one whose bolt is not on
/// this lane — as a slip (209, S14).
fn slip(read: &Read, row: &status::Row, lane: &[&status::Row]) -> String {
    let mut out = String::new();
    let _ = write!(out, "<div class=\"slip\"{}>\n", board_attributes(row));
    let _ = write!(
        out,
        "<span class=\"sl-k\">{machine}</span>\
         <a class=\"sl-n\" href=\"#dock-{object}\">{name}</a>\
         <span class=\"sl-to\">{said}</span>\n",
        machine = escape(&row.machine),
        object = escape(&row.object),
        name = escape(name_of(&row.object)),
        said = escape(&row.said),
    );
    let items = children_of(read, row, lane);
    for item in items {
        let _ = write!(
            out,
            "<span class=\"sl-to\"><span class=\"arrow\">→</span> {name} · {said}</span>\n",
            name = escape(name_of(&item.object)),
            said = escape(&item.said),
        );
    }
    out.push_str(&discussion(row));
    let held = held_by(row);
    if !held.is_empty() {
        let _ = write!(out, "<span class=\"sl-to\">{}</span>\n", escape(&held));
    }
    out.push_str("</div>\n");
    out
}

/// A signal: what it says, in quotation marks against its left rule, with
/// curation's move under it and no box at all (209, S16).
///
/// What is quoted is the signal's own words — its assertion, or the excerpt it
/// was taken from — because a quote whose quotation is the object's id says
/// nothing a person can judge, and judging it is the whole of curation (113,
/// 116). Where it carries neither, its id stands in.
fn quote(read: &Read, row: &status::Row) -> String {
    let record = read.objects.iter().find(|o| o.id == row.object).map(|o| &o.record);
    let said = record
        .and_then(|r| r.get("assertion").or_else(|| r.get("excerpt")).or_else(|| r.get("raw")))
        .and_then(|value| value.as_str())
        .filter(|said| !said.trim().is_empty())
        .map(clipped)
        .unwrap_or_else(|| row.object.clone());
    let by = record
        .and_then(|r| r.get("asserted_by"))
        .and_then(|value| value.as_str())
        .filter(|by| !by.trim().is_empty());
    let mut under = format!("<b>{}</b>", escape(&row.said));
    if let Some(by) = by {
        let _ = write!(&mut under, " · {}", escape(by));
    }
    let held = held_by(row);
    if !held.is_empty() {
        let _ = write!(&mut under, " · {}", escape(&held));
    }
    // A capture is where work may start: the `unit` control on it is the
    // operator's dictation naming a bolt, applied directly (34, 12). On the
    // board it is one tap to the dock, where the name is typed; a form under
    // every quote read as a lane of forms.
    let acts = match (row.machine.as_str(), read.repositories.is_empty()) {
        ("capture", false) => format!(
            "<div class=\"acts\"><a class=\"btn sm to-dock\" href=\"#dock-{}\" data-tool=\"propose-unit\">unit…</a></div>",
            escape(&row.object)
        ),
        _ => String::new(),
    };
    format!(
        "<div class=\"quote\"{attributes}>\
         <q><a href=\"#dock-{object}\">{said}</a></q>\
         <span class=\"qm\">{under}</span>{acts}</div>\n",
        attributes = board_attributes(row),
        object = escape(&row.object),
        said = escape(&said),
    )
}

/// The `unit` control on a capture: a bolt's name, and the unit is approved on
/// it through `propose-unit` (34, 12, 19). The repository is the one the
/// instance tracks; only an instance tracking several is asked which. An
/// instance tracking none has nowhere for a unit to land, so there is no
/// control.
fn unit_form(read: &Read, capture: &str) -> String {
    let repository = match read.repositories.as_slice() {
        [] => return String::new(),
        [one] => format!("<input type=\"hidden\" name=\"repository\" value=\"{}\">", escape(one)),
        many => {
            let options: String = many
                .iter()
                .map(|r| format!("<option>{}</option>", escape(r)))
                .collect();
            format!("<select name=\"repository\" aria-label=\"repository\">{options}</select>")
        }
    };
    format!(
        "<form class=\"acts unit\" method=\"post\" action=\"/api/tools/propose-unit\">\
         <input type=\"hidden\" name=\"capture\" value=\"{capture}\">\
         <input type=\"hidden\" name=\"type\" value=\"chore\">\
         <input type=\"text\" name=\"bolt\" placeholder=\"bolt name\" aria-label=\"bolt name\" required>\
         {repository}<button class=\"btn\" type=\"submit\">unit</button></form>\n",
        capture = escape(capture),
    )
}

/// Everything with a session under it — a session, a host, the instance, a
/// sink, curation — as a row: what it is, what it is called, how it stands, and
/// what it last did (209, S13, S15).
fn sessrow(row: &status::Row) -> String {
    let mut out = String::new();
    let _ = write!(out, "<div class=\"sessrow\"{}>\n", board_attributes(row));
    let _ = write!(
        out,
        "<div class=\"oh\"><span class=\"ok\">{machine}</span>\
         <a class=\"on\" href=\"#dock-{object}\">{name}</a>\
         <span class=\"os\">{said}</span></div>\n",
        machine = escape(&row.machine),
        object = escape(&row.object),
        name = escape(name_of(&row.object)),
        said = escape(&row.said),
    );
    let held = held_by(row);
    if !held.is_empty() {
        let _ = write!(out, "<div class=\"ot\">{}</div>\n", escape(&held));
    }
    out.push_str(&discussion(row));
    out.push_str("</div>\n");
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
    // Nothing waiting and no curation charged: the lane's own empty state
    // says it, and no section stands here to be read past.
    if read.status.unmoved.is_empty() && read.curation.is_none() {
        return String::new();
    }
    let mut out = String::from(
        "<section id=\"unmoved-signals\"><div class=\"sec-h\">unmoved signals\
         <span class=\"r\">by source</span></div>\n",
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
         <span class=\"r\">one move per signal</span></div>\n",
    );
    let Some(session) = &read.curation else {
        out.push_str(
            "<div class=\"empty\">not charged; it charges at the threshold or on the cadence</div>\n\
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
/// under 760px with a back control (209, 210, 307, S27, S28).
///
/// A surface takes the form of the object in its header, says what the object
/// is and what it holds in its body, and carries the object's answers — or what
/// there is to answer and why not — at its foot (S27). Everything on it is read
/// from the objects the one request already read, so the whole page is still
/// one read and a reload loses nothing (310).
fn dock(read: &Read) -> String {
    let mut out = String::new();
    // A deliverable the request asked to read, open where an object's surface
    // would be: it is the output of the object beside it and is read there
    // rather than in a viewer of its own (190, 213, S27).
    if let Some(reading) = &read.reading {
        let _ = write!(
            out,
            "<article class=\"surface form-deliverable\" id=\"dock-deliverable\" \
             data-kind=\"deliverable\" data-answerable=\"false\" data-opened=\"true\" \
             data-deliverable=\"{named}\">\n\
             <div class=\"dk-h dk-sheet\">\n<div class=\"row\">\
             <span class=\"kind\">deliverable</span>\
             <span class=\"nav\"><a class=\"object lk\" href=\"#dock-{object}\">{object}</a>\
             </span></div>\n<h2>{file}</h2>\n\
             <div class=\"tail\">{named}</div>\n</div>\n\
             <div class=\"dk-b\"><div class=\"doc\">{body}</div></div>\n\
             <div class=\"dk-f\"><div class=\"dk-answers none\" data-answerable=\"false\">\
             <span class=\"fl\">answers</span>\
             <span class=\"r\">a deliverable is what a session left behind; what is answered \
             is the object it belongs to (190, 213)</span></div></div>\n</article>\n",
            named = escape(&reading.named),
            object = escape(&reading.object),
            file = escape(reading.named.rsplit('/').next().unwrap_or(&reading.named)),
            // Already rendered, and escaped before a tag was written (310).
            body = reading.body,
        );
    }
    for object in &read.objects {
        let kind = match object.machine.as_str() {
            m if KINDS.contains(&m) => m,
            other => other,
        };
        let row = read.status.rows.iter().find(|r| r.object == object.id);
        let _ = write!(
            out,
            "<article class=\"surface form-{kind}\" id=\"dock-{}\" data-kind=\"{kind}\" \
             data-answerable=\"{answerable}\" data-opened=\"{opened}\">\n",
            escape(&object.id),
            answerable = read.decisions.iter().any(|d| d.object == object.id),
            opened = read.opened.as_deref() == Some(object.id.as_str()),
        );
        out.push_str(&dock_head(read, object, row));
        out.push_str(&dock_body(read, object, row));
        out.push_str(&dock_answers(read, &object.id));
        out.push_str("</article>\n");
    }
    out
}

/// A surface's header takes the form of the object (S27): the mockup's own
/// `dk-` header for the silhouette its kind has, with the kind, the phase it
/// sits in, the standing decision's number as a marker (S17), the name, and
/// what the object is doing under it.
fn dock_head(read: &Read, object: &Object, row: Option<&status::Row>) -> String {
    let mut out = String::new();
    let form = match silhouette(&object.machine) {
        "thread bead" => "dk-bead",
        "thread" => "dk-thread",
        "ledger" => "dk-ledger",
        "sheet" => "dk-sheet",
        "slip" => "dk-slip",
        _ => "dk-record",
    };
    let _ = write!(out, "<div class=\"dk-h {form}\">\n<div class=\"row\">");
    // Every decision standing on this object, as the marker the board draws on
    // it: the same number the rail card carries, because a decision carries one
    // number wherever it is shown (15, S17).
    for decision in read.decisions.iter().filter(|d| d.object == object.id) {
        if let Some(number) = decision.number {
            let _ = write!(
                out,
                "<span class=\"n\" data-number=\"{number}\">{number}</span>"
            );
        }
    }
    let _ = write!(
        out,
        "<span class=\"kind\">{machine}</span>\
         <span class=\"ph\" data-phase=\"{phase}\">{phase}</span>",
        machine = escape(&object.machine),
        phase = escape(&phase_of(&object.machine).to_lowercase()),
    );
    // The link at the host's address, which is the one every notification and
    // chat line carries: from the dock it is what the operator copies to send
    // this object to somebody (308, 205a).
    let link = links::to_object(&read.address, &object.id).unwrap_or_default();
    let _ = write!(
        out,
        "<span class=\"nav\"><a class=\"object lk\" href=\"{}\">{}</a></span></div>\n",
        escape(&link),
        escape(&object.id)
    );
    let _ = write!(out, "<h2>{}</h2>\n", escape(name_of(&object.id)));
    if let Some(said) = row.map(|r| r.said.as_str()).filter(|s| !s.is_empty()) {
        let _ = write!(out, "<div class=\"tail\">{}</div>\n", escape(said));
    }
    out.push_str("</div>\n");
    out
}

/// One section of a surface's body, or nothing where the object has nothing to
/// put in it. An empty section is a heading over a blank, which is what made
/// the board read as nothing in every column.
fn sec(title: &str, said: &str, body: &str) -> String {
    if body.trim().is_empty() {
        return String::new();
    }
    let right = match said.is_empty() {
        true => String::new(),
        false => format!("<span class=\"r\">{}</span>", escape(said)),
    };
    format!("<div class=\"sec\"><h3>{}{right}</h3>{body}</div>\n", escape(title))
}

/// The files an object's sessions left behind, each one a link that opens it
/// (190, 213).
///
/// Markdown is read on the page, beside the object it belongs to; HTML opens at
/// its own address, because it is its own document with its own styles and the
/// page will not put a file's markup inside itself (310). The link is the same
/// shape either way and the route decides, so the operator taps a deliverable
/// and reads it, whichever kind it is.
fn deliverables(read: &Read, object: &str) -> String {
    let Some(held) = read.delivered.get(object).filter(|held| !held.is_empty()) else {
        return String::new();
    };
    let instance = read.address.rsplit('/').next().unwrap_or_default();
    let mut out = String::from("<ul class=\"delivered\">\n");
    for file in held {
        let _ = write!(
            out,
            "<li><a class=\"deliverable\" href=\"/{instance}/deliverable/{named}\"{target} \
             data-deliverable=\"{named}\">{file}</a>\
             <span class=\"r\">{under}</span></li>\n",
            named = escape(&file.named()),
            file = escape(file.file()),
            under = escape(&file.repository),
            // Its own document opens in its own tab, because it replaces the
            // page rather than sitting in it and the operator is mid-decision.
            target = match file.is_its_own_document() {
                true => " target=\"_blank\" rel=\"noopener\"",
                false => "",
            },
        );
    }
    out.push_str("</ul>\n");
    out
}

/// The body of a surface: what the object is, where it sits, what hangs off it,
/// what it records, and what has been said on it (S28, 141, 144, 209, 210).
fn dock_body(read: &Read, object: &Object, row: Option<&status::Row>) -> String {
    let mut out = String::from("<div class=\"dk-b\">\n");

    // A link to a host past its stale window opens this surface and says the
    // host is away and since when, rather than failing silently (308, 150a).
    if let Some(away) = read.away.get(&object.id) {
        let _ = write!(
            out,
            "<p class=\"away\" data-away-host=\"{}\">{}</p>\n",
            escape(&away.host),
            escape(&away.said())
        );
    }

    // Why the decision on it is being asked, in the machine's own words. The
    // rail card carries the same line; a link that opened here instead must
    // not make the operator go back for it (15, 308).
    let why: Vec<String> = read
        .decisions
        .iter()
        .filter(|d| d.object == object.id)
        .filter_map(|d| read.why.get(&d.id))
        .filter(|said| !said.is_empty())
        .map(|said| format!("<div class=\"dtext\">{}</div>\n", escape(&said.join(" · "))))
        .collect();
    out.push_str(&sec(
        "the decision",
        "why it is being asked",
        &why.join(""),
    ));

    // A capture is where work may start (34, 12, 19).
    if object.machine == "capture" {
        out.push_str(&sec(
            "make it work",
            "a unit on a bolt of this repository, approved by you",
            &unit_form(read, &object.id),
        ));
    }

    // What the sessions on it left behind. A session that finishes leaves real
    // files at real paths, and until the page linked them the operator could
    // see that an elaboration was done and not read what it produced, which
    // empties out the whole stage (190, 213).
    out.push_str(&sec(
        "what it delivered",
        "a file at a path, as the session left it",
        &deliverables(read, &object.id),
    ));

    // Which host holds it, which host runs it, whether that host is alive, and
    // what its lease is in: the four things 141 asks for per object.
    if let Some(row) = row {
        let mut standing = String::new();
        let held = held_by(row);
        if !held.is_empty() {
            let _ = write!(&mut standing, "<div class=\"row\">{}</div>\n", escape(&held));
        }
        if let Some(lease) = &row.lease {
            let _ = write!(
                &mut standing,
                "<div class=\"row\">lease {}</div>\n",
                escape(lease)
            );
        }
        let _ = write!(
            &mut standing,
            "<div class=\"row\">{} · {}</div>\n",
            escape(&row.group),
            escape(phase_of(&object.machine))
        );
        out.push_str(&sec("where it stands", "the host, the lease and the group", &standing));
    }

    // A proposed intent shows its weight: the signals it cites, how many, from
    // which sources and over what span, counted by event date (109, 118).
    if let Some(weight) = read.weight.get(&object.id) {
        // How many, from which sources and over what span — and a source list
        // nobody recorded says nothing rather than "from " with a blank after
        // it (109, 118).
        let mut said = format!(
            "{} signal{}",
            weight.count,
            match weight.count {
                1 => "",
                _ => "s",
            }
        );
        if !weight.sources.is_empty() {
            let _ = write!(&mut said, " from {}", escape(&weight.sources.join(", ")));
        }
        if let Some(span) = weight.span() {
            let _ = write!(&mut said, ", {}", escape(&span));
        }
        let mut cited = format!(
            "<p class=\"weight\" data-signals=\"{}\" data-sources=\"{}\">{said}</p>\n",
            weight.count,
            escape(&weight.sources.join(" ")),
        );
        cited.push_str("<ul class=\"cited\">\n");
        for signal in &weight.signals {
            let _ = write!(&mut cited, "<li class=\"signal\">{}</li>\n", escape(signal));
        }
        cited.push_str("</ul>\n");
        out.push_str(&sec("what it weighs", "the signals it cites", &cited));
    }

    // What the object records of itself — its type, what it covers, the claims
    // it cites, the document behind it — said rather than printed as JSON.
    let already: Vec<&str> = read
        .decisions
        .iter()
        .filter(|d| d.object == object.id)
        .filter_map(|d| read.why.get(&d.id))
        .flatten()
        .map(String::as_str)
        .collect();
    let mut recorded = String::new();
    for (name, value) in &object.record {
        if let Some(said) = shown(name, value).filter(|said| !already.contains(&said.as_str())) {
            let _ = write!(
                &mut recorded,
                "<div class=\"row\" data-field=\"{}\">{}</div>\n",
                escape(name),
                escape(&said)
            );
        }
    }
    out.push_str(&sec("what it records", "the object's own fields", &recorded));

    // What hangs off it: an intent lists its elaborations in order and opens
    // each, a bolt its units, a unit its work items (209, 210).
    let children: Vec<&Object> = read
        .objects
        .iter()
        .filter(|o| o.parent.as_deref() == Some(object.id.as_str()))
        .collect();
    if !children.is_empty() {
        let holds = match object.machine.as_str() {
            "intent" => "its elaborations, in order",
            "bolt" => "its units, in order",
            "unit" => "the work items on it",
            _ => "what hangs off it",
        };
        let mut held = String::from("<ol class=\"elaborations\">\n");
        for child in &children {
            let said = read
                .status
                .rows
                .iter()
                .find(|r| r.object == child.id)
                .map(|r| r.said.clone())
                .unwrap_or_default();
            let _ = write!(
                &mut held,
                "<li><a class=\"elaboration\" href=\"#dock-{id}\">{name}</a>\
                 <span class=\"r\">{said}</span></li>\n",
                id = escape(&child.id),
                name = escape(name_of(&child.id)),
                said = escape(&said),
            );
        }
        held.push_str("</ol>\n");
        out.push_str(&sec("what it holds", holds, &held));
    }

    // And what it hangs off, which is how an elaboration gets back to its
    // thread and a unit to its ledger (210, S28).
    if let Some(parent) = &object.parent {
        out.push_str(&sec(
            "what it is part of",
            "back to it",
            &format!(
                "<div class=\"row\"><a class=\"elaboration\" href=\"#dock-{id}\">{name}</a></div>\n",
                id = escape(parent),
                name = escape(parent),
            ),
        ));
    }

    // The question, the answer and the note stay with the object and are shown
    // under it once the session that said them is gone (144).
    let said = row.map(discussion).unwrap_or_default();
    out.push_str(&sec("what was said on it", "it stays with the object (144)", &said));

    out.push_str("</div>\n");
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
            "<div class=\"dk-f\"><div class=\"dk-answers none\" data-answerable=\"false\">\
             <span class=\"fl\">answers</span>\
             <span class=\"r\">nothing stands on this object; the machinery takes it from \
             here and raises the next decision that is yours (13)</span></div></div>\n",
        );
    }
    // What the footer says of itself is "one response each" and no more: why
    // answering here is answering on the rail is said once, at the foot of the
    // panel, rather than on all twenty-seven surfaces (15, S27).
    let mut out = String::from(
        "<div class=\"dk-f\"><div class=\"fl\">answer<span class=\"r\">one response each\
         </span></div>\n<div class=\"dk-answers\" data-answerable=\"true\">\n",
    );
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
    out.push_str("</div>\n</div>\n");
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
