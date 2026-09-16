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
//! (310), and the bundle fetches nothing from anywhere but its own host: its
//! stylesheet, its script and its faces are served there under the binary's
//! version (310, 310a, S235).
//!
//! Under 760px it is the same bundle: two tabs, Decisions and Board, with the
//! dock full screen and a back control (307). One bundle is built and one is
//! served, and its version is the binary's (307).

use crate::links;
use flywheel_atoms::{CommitRef, StateStore, World};

mod asks;
mod dock;
mod lists;
pub(crate) mod hand;
mod palette;
pub(crate) mod tray;
pub use dock::Session;
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

/// The template as it is served, made once per process: its comments and
/// indentation taken out, and its stylesheet and script lifted out into
/// responses of their own, which the page names and a member's client's bundle
/// carries inline (310a, S235, S230).
struct Parts {
    /// The document, with `{{STYLE}}` and `{{SCRIPT}}` where the two were.
    shell: String,
    stylesheet: String,
    script: String,
}

fn parts() -> &'static Parts {
    static PARTS: std::sync::OnceLock<Parts> = std::sync::OnceLock::new();
    PARTS.get_or_init(|| {
        let lift = |document: &str, open: &str, close: &str, slot: &str| -> (String, String) {
            let start = document.find(open).expect("the template carries the block");
            let end = start + document[start..].find(close).expect("the block is closed");
            let inner = document[start + open.len()..end].trim().to_string();
            (format!("{}{slot}{}", &document[..start], &document[end + close.len()..]), inner)
        };
        let (shell, stylesheet) = lift(&compact(TEMPLATE), "<style>", "</style>", "{{STYLE}}");
        let (shell, script) = lift(&shell, "<script>", "</script>", "{{SCRIPT}}");
        Parts { shell, stylesheet: stylesheet.replace("{{FONTS}}", fonts_at_the_host().trim_end()), script }
    })
}

/// The page's stylesheet, as the host serves it.
pub fn stylesheet() -> &'static str {
    &parts().stylesheet
}

/// The page's script, as the host serves it.
pub fn script() -> &'static str {
    &parts().script
}

/// Where the page asks its own host for its stylesheet and its script: names
/// carrying the binary's version, so what a year's cache holds is never another
/// build's (291, 310a, S235).
pub fn style_address() -> String {
    format!("/bundle/page.{VERSION}.css")
}

pub fn script_address() -> String {
    format!("/bundle/page.{VERSION}.js")
}

/// The stylesheet or the script a request names, with its media type, where it
/// names this build's.
pub fn bundled(file: &str) -> Option<(&'static str, &'static str)> {
    if file == format!("page.{VERSION}.css") {
        return Some(("text/css; charset=utf-8", stylesheet()));
    }
    if file == format!("page.{VERSION}.js") {
        return Some(("text/javascript; charset=utf-8", script()));
    }
    None
}

/// A document with its comments and the indentation of its lines taken out:
/// `<!-- -->` in the markup, `/* */` in the stylesheet and whole `//` lines in
/// the script. Every line keeps its own content and its end, so nothing a line
/// says changes meaning (310a).
pub(crate) fn compact(document: &str) -> String {
    let lines = |text: &str, keep: &dyn Fn(&str) -> bool| -> String {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && keep(line))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let without = |text: &str, open: &str, close: &str| -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(at) = rest.find(open) {
            out.push_str(&rest[..at]);
            match rest[at..].find(close) {
                Some(end) => rest = &rest[at + end + close.len()..],
                None => {
                    rest = "";
                }
            }
        }
        out.push_str(rest);
        out
    };
    let markup = |text: &str| lines(&without(text, "<!--", "-->"), &|_| true);
    let (Some(style), Some(style_end), Some(script), Some(script_end)) = (
        document.find("<style>"),
        document.find("</style>"),
        document.find("<script>"),
        document.rfind("</script>"),
    ) else {
        return markup(document);
    };
    let style = style + "<style>".len();
    let script = script + "<script>".len();
    [
        markup(&document[..style]),
        lines(&without(&document[style..style_end], "/*", "*/"), &|_| true),
        markup(&document[style_end..script]),
        lines(&document[script..script_end], &|line| !line.starts_with("//")),
        markup(&document[script_end..]),
    ]
    .join("\n")
}

/// What one request read. Everything the page shows comes from here, so the
/// whole page is one read and a reload shows what is recorded (310).
#[derive(Clone)]
pub struct Read {
    pub decisions: Vec<DecisionInstance>,
    pub status: status::Status,
    pub objects: Vec<Object>,
    /// The host's address, for the links every rail line carries (308, 205a).
    pub address: String,
    /// The host serving this page, by its id: what a session chip's pane link
    /// reads to say whether the pane is on this computer or another (S234).
    pub served_by: String,
    /// The identity every response records as given by (153, 236a, 253a).
    pub operator: String,
    /// The built repositories the instance tracks, which is where a unit
    /// lands: one, and the `unit` control on a capture never asks; several,
    /// and it does (34, 205, 206).
    pub repositories: Vec<String>,
    /// Every ask on record, so a signal routed to one shows the words and the
    /// repository they are for (28, 116).
    pub asks: Vec<flywheel_atoms::Ask>,
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
    /// The move standing on each signal, by the signal's id, read from the move
    /// records, so what became of a note shows the moment its move is written
    /// (107, S28).
    pub moves: BTreeMap<String, signals::Move>,
    /// What each signal the material holds says, by its id — its assertion,
    /// else its excerpt — for a signal the store holds no record of, so what
    /// is named from it reads its words (113, S226).
    pub signal_words: BTreeMap<String, String>,
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
    /// The last commits on every bolt's line, by bolt id (185, S28).
    pub commits: BTreeMap<String, Vec<CommitRef>>,
    /// The bolts whose commits are main's latest, because their branch holds
    /// nothing main does not — landed, or not yet built on.
    pub commits_are_mains: std::collections::BTreeSet<String>,
    /// Every session the bindings recorded, by session id, with what its
    /// thread says it did (141, 144, S28).
    pub sessions: BTreeMap<String, Session>,
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
    /// Which rendering of the instance this is: a count the host raises when
    /// its store moved, carried on the page so the page can tell whether the
    /// one it holds is still current (S221).
    pub generation: u64,
    /// The object a link opened the page at, where the request named one: the
    /// dock's surface for it is the one already open, so a link the machinery
    /// wrote lands on the object it names rather than on the board (308, 205a,
    /// 209).
    pub opened: Option<String>,
    /// The objects whose type the definitions in force do not hold, so a card
    /// names no type for them and asks for one instead (85a, S226).
    pub untyped: std::collections::BTreeSet<String>,
    /// Where each object stands in `objects` and which hang off each, so a row
    /// looks its object up rather than passing over the whole instance (310a).
    pub index: Index,
}

/// Where each object of a read stands, by its id, and which objects hang off
/// each, built once per read: a lookup a row makes is then no pass over the
/// whole instance, so what the page costs grows with what is on screen (310a,
/// S235).
#[derive(Clone, Default)]
pub struct Index {
    at: BTreeMap<String, usize>,
    children: BTreeMap<String, Vec<usize>>,
}

impl Index {
    fn of(objects: &[Object]) -> Index {
        let mut index = Index::default();
        for (at, object) in objects.iter().enumerate() {
            index.at.entry(object.id.clone()).or_insert(at);
            if let Some(parent) = &object.parent {
                index.children.entry(parent.clone()).or_default().push(at);
            }
        }
        index
    }
}

impl Read {
    /// An object of the read, by its id.
    pub(crate) fn object(&self, id: &str) -> Option<&Object> {
        self.index.at.get(id).map(|&at| &self.objects[at])
    }

    /// The objects hanging off one, in the order the read holds them.
    pub(crate) fn children<'a>(&'a self, parent: &str) -> impl Iterator<Item = &'a Object> + 'a {
        self.index.children.get(parent).into_iter().flatten().map(move |&at| &self.objects[at])
    }
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

/// A host past its stale window, as the page says it: the host, that it is
/// away, and since when (150a, S214, S28).
///
/// The record keeps the moment to the microsecond and a person reads none of
/// it, so the line says the moment the way the hosts strip says it and the way
/// every other moment on the page is said — never the stamp itself (141, S28).
pub(crate) fn away_said(away: &sinks::Away) -> String {
    format!("{} is away since {}", away.host, short(&away.since.to_rfc3339()))
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
        // An answer on one row of a fold says which, so a drop of one chore
        // does not read as the fold's: the letter after the number it named,
        // `415b`, or the `row` a response recorded before the letter rode in
        // `decision` carries (S232).
        let args = object.record.get("args");
        let row = args
            .and_then(|args| args.get("decision"))
            .and_then(|v| v.as_str())
            .map(|said| said.trim().trim_start_matches(|c: char| c.is_ascii_digit()))
            .filter(|row| !row.is_empty())
            .or_else(|| args.and_then(|args| args.get("row")).and_then(|v| v.as_str()).map(str::trim))
            .filter(|row| !row.is_empty());
        out.entry(number as u32).or_default().push(Answered {
            answer: match row {
                Some(row) => format!("{} {number}{row}", field("answer")),
                None => field("answer"),
            },
            given_by: field("given_by"),
            given_at: field("given_at"),
        });
    }
    out
}

/// Read once, for one request (310).
/// The same, read for no named host: nothing is marked as being on another
/// computer, which is what a member's client and a test see (293a, S234).
pub fn read<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &W,
    defs: &Definitions,
    address: &str,
    operator: &str,
) -> anyhow::Result<Read> {
    read_served(store, world, defs, address, operator, "")
}

/// What one request read, for the host serving it. `served_by` is that host's
/// id: a session chip's pane link reads it to say whether the pane is on this
/// computer or another (S234, 232).
pub fn read_served<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &W,
    defs: &Definitions,
    address: &str,
    operator: &str,
    served_by: &str,
) -> anyhow::Result<Read> {
    let decisions = commands::rail_read(store, defs)?;
    let at = commands::now(store)?;
    let as_of = store.read(flywheel_domain::RAIL)?.as_of;
    // The signal material read in one pass, which every read of it below
    // answers from (111, 203).
    let files = signals::snapshot(world);
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
    let index = Index::of(&objects);
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
    let mut untyped = std::collections::BTreeSet::new();
    for decision in &decisions {
        let Some(object) = objects.iter().find(|o| o.id == decision.object) else {
            continue;
        };
        let evidence = store.read(&decision.object).map(|read| read.evidence).unwrap_or_default();
        let mut said: Vec<String> = Vec::new();
        // A type the instance has no definition of is said once, as what to do
        // about it, in place of the type line: a yes waits on it (85a).
        let kind = object.record.get("type").and_then(|v| v.as_str()).map(str::trim).filter(|k| !k.is_empty());
        let undefined = decision.shows.iter().any(|name| name.ends_with(".type_defined"))
            && !flywheel_domain::blueprints::type_defined(defs, kind);
        if undefined {
            untyped.insert(decision.object.clone());
        }
        for name in &decision.shows {
            if name.ends_with(".type_defined") {
                if undefined {
                    said.push(match kind {
                        Some(kind) => format!("{kind} is not a type here · set one with type…"),
                        None => "no type named · set one with type…".to_string(),
                    });
                }
                continue;
            }
            if undefined && name.rsplit('.').next() == Some("type") {
                continue;
            }
            // An elaboration's card says its type beside the question and draws
            // the intents a gathering covers, so neither is a line as well
            // (S226, 188).
            if decision.kind == "elaboration-proposed"
                && matches!(name.rsplit('.').next(), Some("type") | Some("covers"))
            {
                continue;
            }
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
        // A proposed intent's card is its weight and what it proposes: curation
        // proposed it, the signals it cites, from how many sources, over what
        // span by event date, and the elaborations it proposes each with its
        // type. A count and an age said neither how wide the evidence was nor
        // what saying yes would start (109, 118, 10, S5).
        if decision.kind == "intent-proposed" {
            said.clear();
            said.push("curation".to_string());
            if let Some(weighs) = weight.get(&object.id) {
                said.push(format!("{} {}", weighs.count, counted("signals", weighs.count)));
                let sources = weighs.sources.len();
                if sources > 0 {
                    said.push(format!("{sources} {}", counted("sources", sources)));
                }
                if let Some(days) = weighs.span_days() {
                    said.push(match days {
                        0 => "one day".to_string(),
                        days => format!("{days}d"),
                    });
                }
            }
            let proposes: Vec<String> = object
                .record
                .get("elaborations")
                .and_then(|value| serde_json::from_value(value.clone()).ok())
                .unwrap_or_default();
            if !proposes.is_empty() {
                said.push(format!("elaborations: {}", proposes.join(", ")));
            }
        }
        // A fold of chores says who offered them; what they are is its
        // heading (S231, 11).
        if let Some((_, chores)) = chores_of(&objects, decision) {
            let mut offered_by: Vec<&str> = chores
                .iter()
                .filter_map(|chore| chore.record.get("sources")?.as_array()?.first()?.as_str())
                .filter_map(|entry| entry.split_once('#').map(|(session, _)| session))
                .collect();
            offered_by.sort_unstable();
            offered_by.dedup();
            said.clear();
            if !offered_by.is_empty() {
                said.push(format!("offered by {}", offered_by.join(", ")));
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
    let moves: BTreeMap<String, signals::Move> =
        signals::moves(&files).into_iter().map(|moved| (moved.signal.clone(), moved)).collect();
    let signal_words: BTreeMap<String, String> = signals::all_signals_from(&files)
        .into_iter()
        .filter_map(|signal| {
            let said = match signal.assertion.trim().is_empty() {
                true => signal.excerpt.trim().to_string(),
                false => signal.assertion.trim().to_string(),
            };
            (!said.is_empty()).then_some((signal.id, said))
        })
        .collect();
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
    let asks = store.asks()?;
    // The commits on each bolt's line, from the world, and every session the
    // bindings recorded, with its exit from its own thread (185, 144, S28).
    let mut commits: BTreeMap<String, Vec<CommitRef>> = BTreeMap::new();
    let mut commits_are_mains = std::collections::BTreeSet::new();
    for bolt in objects.iter().filter(|o| o.machine == "bolt") {
        let Some(repository) = bolt.record.get("repository").and_then(|v| v.as_str()) else {
            continue;
        };
        let mut held = world.line_log(repository, &bolt.id, 12).unwrap_or_default();
        if held.is_empty() && bolt.config.get("life").map(String::as_str) == Some("landed") {
            held = world.line_log(repository, "main", 8).unwrap_or_default();
            if !held.is_empty() {
                commits_are_mains.insert(bolt.id.clone());
            }
        }
        if !held.is_empty() {
            commits.insert(bolt.id.clone(), held);
        }
    }
    let mut sessions: BTreeMap<String, Session> = BTreeMap::new();
    for fact in objects.iter().filter(|o| o.machine == "fact" && o.id.starts_with("fact/session/")) {
        let id = fact.id.trim_start_matches("fact/session/").to_string();
        let text = |name: &str| fact.record.get(name).and_then(|v| v.as_str()).map(String::from);
        let item = id.rsplitn(3, '/').nth(2).unwrap_or(&id).to_string();
        let mut session = Session {
            id: id.clone(),
            item,
            host: text("host").unwrap_or_default(),
            runner: text("runner").unwrap_or_else(|| "operator".into()),
            agent: text("herdr_agent"),
            // What it was started as, from the record: the chip names the
            // program and the model and reads neither from the program (S53).
            kind: text("kind"),
            model: text("model"),
            pane: text("herdr_pane"),
            herdr_session: text("herdr_session"),
            started: text("started_at"),
            ..Default::default()
        };
        for entry in store.thread(&id).unwrap_or_default() {
            match entry.kind.as_str() {
                "exit" => {
                    session.exit = entry.fields.get("exit").and_then(|v| v.as_str()).map(String::from);
                    session.exit_at = Some(entry.at.to_rfc3339());
                    session.deliverables = entry
                        .fields
                        .get("deliverables")
                        .and_then(|v| v.as_array())
                        .map(|d| d.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();
                    if let Some(q) = entry.fields.get("question").and_then(|v| v.as_str()) {
                        session.question = Some(q.to_string());
                    }
                }
                _ => {}
            }
        }
        sessions.insert(id, session);
    }

    Ok(Read {
        generation: 0,
        decisions,
        status,
        objects,
        address: address.to_string(),
        served_by: served_by.to_string(),
        operator: operator.to_string(),
        repositories,
        asks,
        away,
        weight,
        answered,
        unmoved,
        moves,
        signal_words,
        curation,
        intents,
        why,
        delivered,
        commits,
        commits_are_mains,
        sessions,
        reading: None,
        tour: flywheel_domain::tour::read(store, instance_of(address)),
        refused: None,
        opened: None,
        untyped,
        index,
    })
}

/// The chores one decision stands for, where it is a fold of proposed chore
/// units — a bolt's, or a repository's shared line's — with the name it is
/// headed by: the bolt's, or the repository's (11, 60, S231). `None` for any
/// other decision.
pub(crate) fn chores_of<'a>(objects: &'a [Object], decision: &DecisionInstance) -> Option<(String, Vec<&'a Object>)> {
    if decision.kind != "unit-proposed" {
        return None;
    }
    let ids: Vec<&str> = match decision.folds.is_empty() {
        true => vec![decision.object.as_str()],
        false => decision.folds.iter().map(String::as_str).collect(),
    };
    let chores: Vec<&Object> = ids
        .iter()
        .filter_map(|id| objects.iter().find(|o| o.id == *id))
        .filter(|o| o.record.get("type").and_then(|v| v.as_str()) == Some("chore"))
        .collect();
    if chores.is_empty() || chores.len() != ids.len() {
        return None;
    }
    let batch = chores[0].record.get("batch").and_then(|v| v.as_str())?;
    let name = match batch.starts_with("bolt/") {
        true => name_of(batch).to_string(),
        false => batch.to_string(),
    };
    Some((name, chores))
}

/// One row of a fold of chores: the chore, the letter it is named by, and
/// whether it still stands in the fold (S232).
pub(crate) struct ChoreRow<'a> {
    pub letter: String,
    pub chore: &'a Object,
    pub standing: bool,
}

/// A fold's chores as lettered rows (S232).
///
/// The letters run over every chore of the fold's batch that stands in it or
/// has left it since it was raised, in the order their ids count them, the
/// order the engine takes among a batch's objects, so a letter stays with its
/// chore when another row is answered and leaves. A chore
/// answered before the fold was raised takes none. `None` for a decision that
/// is no fold of chores.
pub(crate) fn rows_of<'a>(objects: &'a [Object], decision: &DecisionInstance) -> Option<Vec<ChoreRow<'a>>> {
    let (_, standing) = chores_of(objects, decision)?;
    let batch = standing.first()?.record.get("batch").and_then(|v| v.as_str())?;
    let mut held: Vec<&Object> = objects
        .iter()
        .filter(|o| o.machine == "unit")
        .filter(|o| o.record.get("type").and_then(|v| v.as_str()) == Some("chore"))
        .filter(|o| o.record.get("batch").and_then(|v| v.as_str()) == Some(batch))
        .filter(|o| {
            standing.iter().any(|s| s.id == o.id)
                || o.entered_at.get(&decision.region).is_some_and(|left| *left >= decision.since)
        })
        .collect();
    held.sort_by(|a, b| flywheel_engine::rail::id_order(&a.id, &b.id));
    Some(
        held.into_iter()
            .enumerate()
            .map(|(at, chore)| ChoreRow {
                letter: letter(at),
                chore,
                standing: standing.iter().any(|s| s.id == chore.id),
            })
            .collect(),
    )
}

/// The letter of the row at a position: `a` to `z`, then `aa`.
fn letter(at: usize) -> String {
    let mut n = at + 1;
    let mut out = Vec::new();
    while n > 0 {
        n -= 1;
        out.push((b'a' + (n % 26) as u8) as char);
        n /= 26;
    }
    out.iter().rev().collect()
}

/// What a chore's row is called: its document's file name in words, without
/// the directories or the extension.
pub(crate) fn chore_words(document: &str) -> String {
    let file = document.rsplit('/').next().unwrap_or(document);
    let stem = file.rsplit_once('.').map(|(stem, _)| stem).filter(|stem| !stem.is_empty()).unwrap_or(file);
    stem.replace(['-', '_'], " ")
}

/// A fold of chores as its card lists them: a lettered row per standing chore
/// by its document, what the offer said it concerns where it said, and its own
/// drop where the fold holds more than one (S232, S231).
fn chore_rows(number: &str, rows: &[ChoreRow<'_>]) -> String {
    let standing: Vec<&ChoreRow<'_>> = rows.iter().filter(|row| row.standing).collect();
    let dropped_alone = standing.len() > 1 && !number.is_empty();
    let mut out = String::from("<ol class=\"chores\">\n");
    for row in &standing {
        let text = |name: &str| row.chore.record.get(name).and_then(|v| v.as_str());
        let document = text("document").unwrap_or_else(|| name_of(&row.chore.id));
        let _ = write!(
            out,
            "<li class=\"chore\" data-row=\"{letter}\" data-object=\"{id}\"><span class=\"lt\">{letter}</span>\
             <a class=\"nm\" href=\"#dock-{id}\" data-object=\"{id}\" title=\"{document}\">{words}</a>{about}{drop}</li>\n",
            letter = escape(&row.letter),
            id = escape(&row.chore.id),
            document = escape(document),
            words = escape(&chore_words(document)),
            about = text("about").map(|about| format!("<span class=\"f\">{}</span>", escape(about))).unwrap_or_default(),
            drop = match dropped_alone {
                true => row_drop(number, &row.letter, None),
                false => String::new(),
            },
        );
    }
    out.push_str("</ol>\n");
    out
}

/// The drop one row of a fold carries: the fold's number and the row's letter
/// posted as one word, `415b`, to the one answer tool, so the chore is dropped
/// alone and the rest stand (S232, 193). On the card it is a mark beside the
/// row; in the dock it says what it does.
pub(crate) fn row_drop(number: &str, letter: &str, label: Option<&str>) -> String {
    let tool = crate::catalogue::ANSWER;
    let (class, said) = match label {
        Some(label) => ("btn sm drop", escape(label)),
        None => ("gx", "×".to_string()),
    };
    format!(
        "<form method=\"post\" action=\"/api/tools/{tool}\" class=\"answer row-drop\">\n\
         <input type=\"hidden\" name=\"decision\" value=\"{number}{letter}\">\n\
         <input type=\"hidden\" name=\"answer\" value=\"drop\">\n\
         <button type=\"submit\" class=\"{class}\" data-row-answer=\"drop\" \
         title=\"{number}{letter} · drop this chore; the others stand\" aria-label=\"drop {number}{letter}\">{said}</button>\n\
         </form>\n",
        number = escape(number),
        letter = escape(letter),
    )
}

/// The answers a decision's controls offer: the model's, except that a fold of
/// chores is answered yes or drop, as a bolt's chores are (S231, 60), and an
/// elaboration offers pick only when it gathers several intents, whose drops
/// are on each intent's chip (188, S226).
fn offered(read: &Read, decision: &DecisionInstance) -> Vec<String> {
    if chores_of(&read.objects, decision).is_some() {
        return decision
            .answers
            .iter()
            .filter(|answer| matches!(answer.as_str(), "yes" | "drop"))
            .cloned()
            .collect();
    }
    if decision.kind == "elaboration-proposed" {
        let gathering = covered(read, &decision.object).len() > 1;
        return decision
            .answers
            .iter()
            .filter(|answer| !is_per_intent_drop(answer))
            .filter(|answer| gathering || !answer.starts_with("pick "))
            .cloned()
            .collect();
    }
    decision.answers.clone()
}

/// `<intent>: drop`: the answer that takes one intent out of a gathering.
fn is_per_intent_drop(answer: &str) -> bool {
    answer.starts_with('<') && answer.ends_with(": drop")
}

/// The intents an elaboration covers, by id: every one a gathering names, or
/// else its own intent (188).
fn covered(read: &Read, object: &str) -> Vec<String> {
    let Some(elaboration) = read.objects.iter().find(|o| o.id == object && o.machine == "elaboration") else {
        return vec![];
    };
    let covers: Vec<String> = elaboration
        .record
        .get("covers")
        .and_then(|v| v.as_array())
        .map(|held| held.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    match covers.is_empty() {
        true => elaboration.parent.iter().cloned().collect(),
        false => covers,
    }
}

/// A gathering's intents, on its card and in its dock: a chip per intent whose
/// × takes that intent out alone (188, S226). Nothing for an elaboration of
/// one intent.
fn gathered(read: &Read, decision: &DecisionInstance, number: &str) -> String {
    if decision.kind != "elaboration-proposed" {
        return String::new();
    }
    let intents = covered(read, &decision.object);
    if intents.len() < 2 {
        return String::new();
    }
    let per_intent = decision.answers.iter().find(|answer| is_per_intent_drop(answer));
    let mut out = String::from("<div class=\"gl\">");
    for intent in &intents {
        let name = name_of(intent);
        let drop = per_intent
            .map(|pattern| {
                format!(
                    "<form method=\"post\" action=\"/api/tools/{tool}\" class=\"answer\">\
                     <input type=\"hidden\" name=\"decision\" value=\"{number}\">\
                     <input type=\"hidden\" name=\"answer\" value=\"{pattern}\">\
                     <input type=\"hidden\" name=\"text\" value=\"{name}\">\
                     <button type=\"submit\" class=\"gx\" data-answer=\"{pattern}\" \
                     title=\"{number} {name}: drop · {does}\" aria-label=\"take {name} out of {number}\">×</button></form>",
                    tool = crate::catalogue::ANSWER,
                    number = escape(number),
                    pattern = escape(pattern),
                    name = escape(name),
                    does = escape(asks::does(&decision.kind, pattern).unwrap_or_default()),
                )
            })
            .unwrap_or_default();
        let _ = write!(out, "<span class=\"gi\" data-intent=\"{}\">{}{drop}</span>", escape(intent), escape(name));
    }
    out.push_str("</div>\n");
    out
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
    match said.chars().count() > 120 {
        false => said.to_string(),
        true => format!("{}…", said.chars().take(119).collect::<String>().trim_end()),
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
    ("Construction", "No bolts yet. Use build now on a note to start one."),
    ("Operation", "Nothing running."),
];

/// The machinery's own objects, which the hosts strip and the header carry
/// and the lanes do not: an instance and a curation are not work a person
/// follows on the board (141, D16).
const OFF_THE_BOARD: [&str; 5] = ["instance", "curation", "host", "rail", "signal"];

/// Render the whole page. One document, one request, nothing stored.
/// The instance this page is of: the last segment of the host's own address,
/// which is where every link the machinery writes puts it (205a, 308).
fn instance_of(address: &str) -> &str {
    address.trim_end_matches('/').rsplit('/').next().unwrap_or_default()
}

/// The two faces the mockup names, as the binary carries them: the latin
/// variable-weight files, by name, family and weights (D16;
/// `page/fonts/NOTICE`).
pub const FONTS: [(&str, &str, &str, &[u8]); 2] = [
    ("manrope-latin", "Manrope", "200 800", include_bytes!("page/fonts/manrope-latin.woff2")),
    (
        "jetbrains-mono-latin",
        "JetBrains Mono",
        "100 800",
        include_bytes!("page/fonts/jetbrains-mono-latin.woff2"),
    ),
];

/// Where the page asks its own host for one face: a name carrying the binary's
/// version, so a face cached for a year is never a face of another build
/// (291, 310a, S235).
pub fn font_address(name: &str) -> String {
    format!("/fonts/{name}.{VERSION}.woff2")
}

/// The face a request names, where it names one of this build's.
pub fn font_named(file: &str) -> Option<&'static [u8]> {
    FONTS
        .iter()
        .find(|(name, ..)| file == format!("{name}.{VERSION}.woff2"))
        .map(|(.., bytes)| *bytes)
}

/// The faces as `@font-face` rules at the host's own address: fetched once and
/// cached, so no load after the first carries them (310, 310a, S235).
fn fonts_at_the_host() -> &'static str {
    static CSS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CSS.get_or_init(|| {
        FONTS
            .iter()
            .map(|(name, family, weights, _)| {
                format!(
                    "@font-face{{font-family:\"{family}\";font-style:normal;font-weight:{weights};\
                     font-display:swap;src:url({}) format(\"woff2\")}}\n",
                    font_address(name)
                )
            })
            .collect()
    })
}

/// The same faces as data, in a stylesheet of their own after the page's, for
/// the bundle a member's client frames: a view is drawn inside the client and
/// reaches no address of the host, so what it needs is in the resource it read,
/// and a face declared later takes the place of the one at the host's address,
/// which is then never asked for (310, S230). Encoded once per process.
fn fonts_embedded() -> &'static str {
    static CSS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CSS.get_or_init(|| {
        use base64::Engine;
        let faces: String = FONTS
            .iter()
            .map(|(_, family, weights, bytes)| {
                format!(
                    "@font-face{{font-family:\"{family}\";font-style:normal;font-weight:{weights};\
                     font-display:swap;src:url(data:font/woff2;base64,{}) format(\"woff2\")}}\n",
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                )
            })
            .collect();
        format!("<style class=\"view-faces\">\n{faces}</style>")
    })
}

pub fn render(read: &Read) -> String {
    let instance = instance_of(&read.address);
    // Attention stands outside the count: it is what the machinery could not do,
    // reported and never dropped, and not a choice the operator is being asked
    // to make (S3, S8, 6).
    // Every region an update swaps, drawn once: the page is those regions in
    // their places, and the digest of each is what the page holds to ask for
    // what moved (S221, S235).
    let drawn: BTreeMap<String, String> = regions(read).into_iter().collect();
    let part = |id: &str| drawn.get(id).cloned().unwrap_or_default();
    let digests = drawn.iter().map(|(id, html)| format!("{id}:{}", digest(html))).collect::<Vec<_>>().join(",");
    let dock_digest = read
        .opened
        .as_deref()
        .and_then(|opened| dock_page(read, opened))
        .map(|page| digest(&page))
        .unwrap_or_default();
    let mut out = parts()
        .shell
        .replace("{{STYLE}}", &format!("<link rel=\"stylesheet\" href=\"{}\">", style_address()))
        .replace("{{SCRIPT}}", &format!("<script src=\"{}\"></script>", script_address()))
        .replace("{{VERSION}}", VERSION)
        .replace("{{DIGESTS}}", &digests)
        .replace("{{DOCKDIGEST}}", &dock_digest)
        .replace("{{SERVED}}", "page")
        .replace("{{GEN}}", &read.generation.to_string())
        .replace("{{VIEWFACES}}", "")
        .replace("{{INSTANCE}}", &escape(instance))
        .replace("{{OPERATOR}}", &escape(&read.operator))
        .replace("{{CLOCK}}", &part("clock"))
        .replace("{{COUNT}}", &part("count"))
        .replace("{{YESALL}}", &part("yes-all"))
        .replace("{{SENT}}", &part("sent"))
        .replace("{{OBJECTS}}", &part("bd-board"))
        .replace("{{HOSTS}}", &part("hosts"))
        .replace("{{RAIL}}", &rail_frame(&part("rail-cards"), &part("rail-since")))
        .replace("{{BOARDH}}", &part("board-h"))
        .replace("{{DOCK}}", &dock(read))
        .replace("{{SENTLIST}}", &part("pal-dyn"))
        .replace("{{PALCOMMANDS}}", &palette::template())
        .replace("{{LOG}}", &part("loglist"))
        .replace("{{TOUR}}", &tour(read))
        .replace("{{TOURHEAD}}", &tour_head(read));
    for (slot, _) in LANES.iter() {
        let id = lane_id(slot);
        out = out.replace(slot, &lane_frame(&id, &|piece| part(&format!("{id}-{piece}"))));
    }
    one_pk_field_per_name(out)
}

/// The picker's filter field, named once.
///
/// The mockup has one picker open at a time and so one `pk-q`; the page writes
/// a picker into every note, because a pick has to be one gesture with the
/// script off (S233). So the first field of the document keeps the design's own
/// name and the rest are numbered after it, and no two elements of a page share
/// an id. Nothing reads them by name — the script reaches the open picker's
/// field through that picker — so the number is only what keeps the id unique.
fn one_pk_field_per_name(html: String) -> String {
    let marker = "id=\"pk-q\"";
    if html.matches(marker).count() < 2 {
        return html;
    }
    let mut out = String::with_capacity(html.len());
    let mut rest = html.as_str();
    let mut nth = 0;
    while let Some(at) = rest.find(marker) {
        out.push_str(&rest[..at]);
        nth += 1;
        match nth {
            1 => out.push_str(marker),
            n => out.push_str(&format!("id=\"pk-q-{n}\"")),
        }
        rest = &rest[at + marker.len()..];
    }
    out.push_str(rest);
    out
}

/// A short digest of a drawn region: what an update compares, so a region that
/// did not move is not sent again and no rendering is kept (15, 310a, S235).
pub fn digest(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The regions of the page an update swaps, by the id each has on the page,
/// drawn from one read: the hosts, the rail's cards and Recently done, the
/// board's head, each lane's head, groups and empty line, the counts, the log,
/// the lists of what was sent, the clock and yes all (S221, S235).
pub fn regions(read: &Read) -> Vec<(String, String)> {
    let standing = read.decisions.iter().filter(|d| flywheel_engine::rail::counted(&d.group)).count().to_string();
    let sent: usize = read.answered.values().map(Vec::len).sum();
    let lately = sent_list(read);
    let mut out = vec![
        ("hosts".to_string(), hosts(read)),
        ("rail-cards".to_string(), rail_cards(read)),
        ("rail-since".to_string(), since(read)),
        ("board-h".to_string(), board_header(read)),
        ("count".to_string(), standing.clone()),
        ("bd-plan".to_string(), standing),
        ("bd-board".to_string(), read.status.rows.len().to_string()),
        ("sent".to_string(), sent.to_string()),
        ("loglist".to_string(), log(read)),
        ("clock".to_string(), escape(&read.status.at.format("%H:%M").to_string())),
        ("pal-dyn".to_string(), lately),
        // The control itself, which the page puts where the one it holds stands.
        ("yes-all".to_string(), yes_all(read)),
    ];
    for ((slot, machines), (title, sub)) in LANES.iter().zip(LANE_HEADS.iter()) {
        let id = lane_id(slot);
        let mine = lane_rows(read, machines);
        out.push((format!("{id}-head"), lane_head(read, title, sub, machines, &mine)));
        for group in status::GROUPS {
            out.push((format!("{id}-{}", group.replace(' ', "-")), lane_group(read, &id, group, &mine)));
        }
        out.push((format!("{id}-empty"), lane_empty(read, title, machines, &mine)));
    }
    out
}

/// What an update carries: every region whose digest is not the one the page
/// holds, with its new digest, and the open drawer's page where it moved;
/// nothing else, and never the whole page (S221, S235, 310a). `open` is the
/// object whose page the drawer holds and that page's digest.
pub fn update(
    read: &Read,
    generation: u64,
    held: &BTreeMap<String, String>,
    open: Option<(&str, &str)>,
) -> serde_json::Value {
    let mut moved = serde_json::Map::new();
    for (id, html) in regions(read) {
        let now = digest(&html);
        if held.get(&id) != Some(&now) {
            moved.insert(id, serde_json::json!({"html": html, "digest": now}));
        }
    }
    let mut out = serde_json::json!({"generation": generation, "regions": moved});
    if let Some((object, was)) = open {
        if let Some(page) = dock_page(read, object) {
            let now = digest(&page);
            if now != was {
                out["dock"] = serde_json::json!({"object": object, "html": page, "digest": now});
            }
        }
    }
    out
}

/// The bundle with nothing of the state in it: its stylesheet, its faces, its
/// script, the palette's commands and every region empty (307, 310).
///
/// The page the host serves is this, rendered from one read; and it is what a
/// member's client is handed as the resource of every view, which draws the
/// regions a tool's result carries into the same places (293a, 322, S230). So
/// there is one page and never a second implementation of it.
pub fn bundle() -> String {
    let shell = &parts().shell;
    let mut out = String::with_capacity(shell.len() + stylesheet().len() + script().len());
    let mut rest = shell.as_str();
    while let Some(open) = rest.find("{{") {
        let Some(close) = rest[open..].find("}}").map(|at| open + at) else {
            break;
        };
        out.push_str(&rest[..open]);
        match &rest[open + 2..close] {
            "VERSION" => out.push_str(VERSION),
            // Inline, the same bytes the host serves at its addresses: a
            // client's frame fetches nothing (310, S230).
            "STYLE" => out.push_str(&format!("<style>\n{}\n</style>", stylesheet())),
            "SCRIPT" => out.push_str(&format!("<script>\n{}\n</script>", script())),
            "VIEWFACES" => out.push_str(fonts_embedded()),
            "SERVED" => out.push_str("view"),
            // The catalogue as the page may invoke it, which is not state (193).
            "PALCOMMANDS" => out.push_str(&palette::template()),
            _ => {}
        }
        rest = &rest[close + 2..];
    }
    out.push_str(rest);
    out
}

/// The page's own views, which a member's client may render: the rail, the
/// board, the status view and one object's detail, which is the dock (322,
/// 307, S230).
pub const VIEWS: [&str; 4] = ["rail", "board", "status", "object"];

/// One view as a member's client is handed it: the regions of the page it
/// draws, by the id each has on the page, and the same in words for a client
/// that draws nothing (311, 322, S230).
#[derive(Debug, Clone)]
pub struct View {
    pub name: &'static str,
    /// The object an `object` view is the detail of.
    pub object: Option<String>,
    pub regions: BTreeMap<String, String>,
    pub said: String,
}

impl View {
    /// What the bundle is handed to draw: the version this was rendered under,
    /// which view it is and its regions (310, 326).
    pub fn handed(&self) -> serde_json::Value {
        let mut out = serde_json::json!({"version": VERSION, "view": self.name, "regions": self.regions});
        if let Some(object) = &self.object {
            out["object"] = serde_json::json!(object);
        }
        out
    }
}

/// Draw one of the page's views from one read, as the page draws the same
/// regions (310, 322).
pub fn view(read: &Read, name: &str, object: Option<&str>) -> Result<View, String> {
    let mut regions = BTreeMap::new();
    let (name, said) = match name {
        "rail" => {
            regions.insert("rail".to_string(), rail(read));
            ("rail", said_rail(read))
        }
        "board" => {
            board_regions(read, &mut regions);
            ("board", said_board(read))
        }
        "status" => {
            regions.insert("hosts".to_string(), hosts(read));
            board_regions(read, &mut regions);
            ("status", said_status(read))
        }
        "object" => {
            let Some(id) = object.map(str::trim).filter(|id| !id.is_empty()) else {
                return Err("`object` takes the id of the object to show".to_string());
            };
            let Some(held) = read.objects.iter().find(|o| o.id == id) else {
                return Err(format!("the instance holds no object `{id}`"));
            };
            regions.insert("dk-b".to_string(), surface(read, held, true));
            return Ok(View {
                name: "object",
                object: Some(id.to_string()),
                regions,
                said: said_object(read, held),
            });
        }
        other => {
            return Err(format!(
                "the page has no view `{other}`; its views are {}",
                VIEWS.join(", ")
            ))
        }
    };
    Ok(View {
        name,
        object: None,
        regions,
        said,
    })
}

/// The board's regions: its header and its four lanes, by their ids.
fn board_regions(read: &Read, regions: &mut BTreeMap<String, String>) {
    regions.insert("board-h".to_string(), board_header(read));
    for ((slot, machines), (title, sub)) in LANES.iter().zip(LANE_HEADS.iter()) {
        let id = lane_id(slot);
        regions.insert(id.clone(), lane(read, &id, title, sub, machines));
    }
}

/// The rail in words: every standing decision in the order the rail reads, its
/// number, what it is about, what it asks and the answers `answer` takes for it
/// (15, 311, 322).
fn said_rail(read: &Read) -> String {
    if read.decisions.is_empty() {
        return "Nothing to decide.".to_string();
    }
    let mut out = String::new();
    let mut group = String::new();
    for decision in flywheel_engine::rail::in_reading_order(&read.decisions) {
        if decision.group != group {
            group = decision.group.clone();
            let _ = writeln!(out, "{group}");
        }
        let number = decision.number.map(|n| n.to_string()).unwrap_or_else(|| "–".into());
        let _ = write!(out, "  {number} · {} · {}", decision.kind, decision.object);
        if let Some(words) = read
            .objects
            .iter()
            .find(|o| o.id == decision.object && o.machine == "signal")
            .and_then(signals::text_of)
        {
            let _ = write!(out, " “{}”", clipped(&words));
        }
        if let Some(asked) = question_of(read, decision) {
            let _ = write!(out, " — {asked}");
        }
        if let Some(why) = read.why.get(&decision.id).filter(|why| !why.is_empty()) {
            let _ = write!(out, " ({})", why.join(" · "));
        }
        let _ = writeln!(out, "\n     answers: {}", offered(read, decision).join(" | "));
        for given in decision.number.and_then(|n| read.answered.get(&n)).into_iter().flatten() {
            let _ = writeln!(out, "     answered: {}", given.said());
        }
    }
    out.trim_end().to_string()
}

/// One object of the status view in words: what it is, the group it is in,
/// what it is doing, and the host that holds it and runs it (141, 143).
fn said_row(row: &status::Row) -> String {
    let mut said = format!("{} · {} · {}", row.object, row.group, row.said);
    if let Some(holder) = &row.holder {
        let _ = write!(said, " · held by {holder}");
        if let Some(liveness) = &row.liveness {
            let _ = write!(said, " ({liveness})");
        }
    }
    if let Some(runner) = &row.runner {
        let _ = write!(said, " · run by {runner}");
    }
    said
}

/// The board in words: each lane with the objects in it (209, S10).
fn said_board(read: &Read) -> String {
    let mut out = String::new();
    for ((_, machines), (title, _)) in LANES.iter().zip(LANE_HEADS.iter()) {
        let rows: Vec<&status::Row> = read
            .status
            .rows
            .iter()
            .filter(|row| !OFF_THE_BOARD.contains(&row.machine.as_str()) && row_in_lane(read, row, machines))
            .collect();
        let _ = writeln!(out, "{title} · {}", rows.len());
        if rows.is_empty() {
            let empty = LANE_EMPTY.iter().find(|(t, _)| t == title).map(|(_, s)| *s).unwrap_or("Nothing here.");
            let _ = writeln!(out, "  {empty}");
        }
        for row in rows {
            let _ = writeln!(out, "  {}", said_row(row));
        }
    }
    out.trim_end().to_string()
}

/// The status view in words: what it is as of, every host with whether it is
/// heard from, and every object grouped by state (141, 143, 146).
fn said_status(read: &Read) -> String {
    let mut out = format!(
        "as of commit {} · {}\n",
        short_mark(&read.status.as_of.mark),
        read.status.as_of.at.format("%H:%M UTC")
    );
    let chips = host_chips(read);
    if chips.is_empty() {
        out.push_str("no host has been heard from yet\n");
    }
    for chip in &chips {
        let _ = writeln!(out, "host {} · {} · {}", chip.name, chip.liveness, chip.said);
    }
    for group in status::GROUPS {
        let rows: Vec<&status::Row> = read
            .status
            .rows
            .iter()
            .filter(|row| row.group == group && !OFF_THE_BOARD.contains(&row.machine.as_str()))
            .collect();
        let _ = writeln!(out, "{group} · {}", rows.len());
        for row in rows {
            let _ = writeln!(out, "  {}", said_row(row));
        }
    }
    out.trim_end().to_string()
}

/// One object in words: what it is and is doing, and each decision standing on
/// it with the answers `answer` takes (209, 308, 311).
fn said_object(read: &Read, object: &Object) -> String {
    let mut out = match read.status.rows.iter().find(|r| r.object == object.id) {
        Some(row) => format!("{} ({})", said_row(row), object.machine),
        None => format!("{} ({})", object.id, object.machine),
    };
    let standing: Vec<&DecisionInstance> = read.decisions.iter().filter(|d| d.object == object.id).collect();
    if standing.is_empty() {
        out.push_str("\nnothing to decide on it");
    }
    for decision in standing {
        let number = decision.number.map(|n| n.to_string()).unwrap_or_else(|| "–".into());
        let _ = write!(out, "\ndecision {number} · {}", decision.kind);
        if let Some(asked) = question_of(read, decision) {
            let _ = write!(out, " — {asked}");
        }
        let _ = write!(out, "\n  answers: {}", offered(read, decision).join(" | "));
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
    let chips = host_chips(read);
    let mut out = String::new();
    if !chips.is_empty() {
        out.push_str("<span class=\"lab\">hosts</span>");
    }
    for chip in &chips {
        let _ = write!(
            out,
            "<span class=\"host{gone}\" data-host=\"{h}\" data-liveness=\"{l}\">\
             <span class=\"dot {l}\"></span><b class=\"hn\">{h}</b><span class=\"hm\">{said}</span></span>",
            gone = match chip.liveness.as_str() {
                "alive" => "",
                _ => " gone",
            },
            h = escape(&chip.name),
            l = escape(&chip.liveness),
            said = escape(&chip.said),
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

/// One host as its chip says it: its name, whether it is heard from, and what
/// it is running, or since when it has been away (141, 146, 150a).
struct HostChip {
    name: String,
    liveness: String,
    said: String,
}

/// Every host the instance has, each as its chip says it.
fn host_chips(read: &Read) -> Vec<HostChip> {
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
    let mut chips = Vec::new();
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
        chips.push(HostChip {
            name: host.clone(),
            liveness,
            said,
        });
    }
    chips
}

/// The rail: every standing decision with its number and its answers, one tap
/// each, in the groups the model folds them into (15, 11, 311).
fn rail(read: &Read) -> String {
    rail_frame(&rail_cards(read), &since(read))
}

/// The rail's head, which says how it is walked and nothing of the state.
const RAIL_HEAD: &str = "<div class=\"rail-h\"><h2>Decisions</h2>\
     <span class=\"keys\" title=\"j and k walk the decisions; the key on a control answers it; Enter opens the one in hand\">\
     <kbd>j</kbd><kbd>k</kbd> walk <kbd>↵</kbd> open</span>\
     <a class=\"btn sm phone-only\" id=\"pal-open-rail\" href=\"#pal-scrim\">capture…</a></div>\n";

/// The rail as it stands on the page: its head, its cards and what finished
/// lately, the last two regions an update swaps apart (S221, S235).
fn rail_frame(cards: &str, since: &str) -> String {
    format!("{RAIL_HEAD}<div id=\"rail-cards\">{cards}</div>\n<div id=\"rail-since\">{since}</div>\n")
}

/// The rail's decisions, in the model's order, each group named once.
fn rail_cards(read: &Read) -> String {
    let mut out = String::new();
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
        out.push_str("<div class=\"empty\">Nothing to decide.</div>\n");
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
            let _ = write!(out, "<div class=\"grp {0}\"><span class=\"g\">{0}</span></div>\n", escape(&group));
        }
        out.push_str(&card(read, decision));
    }
    out
}

/// Whether a capture holds more than one signal, on record or in the material:
/// a transcript read or a folder imported, where a note holds one (19, 114).
fn several_signals(read: &Read, capture: &str) -> bool {
    let mut held: std::collections::BTreeSet<&str> =
        read.children(capture).filter(|o| o.machine == "signal").map(|o| o.id.as_str()).collect();
    for waiting in read.status.waiting.iter().filter(|w| w.capture == capture) {
        held.extend(waiting.signals.iter().map(|s| s.id.as_str()));
    }
    held.len() > 1
}

/// What finished lately, under the decisions: a note captured, a capture built
/// or dropped, a unit merged, a bolt landed — each with its word and when,
/// newest first (14, S59). A finished thing leaves the lane and stands here, so
/// the board is what is moving and the rail is what happened. A capture raises
/// no decision, so this is where the operator sees it was taken (19a, S9).
fn since(read: &Read) -> String {
    let items = since_items(read);
    if items.is_empty() {
        return String::new();
    }
    format!(
        "<div class=\"grp since\"><span class=\"g\">Recently done</span></div>\n<ul class=\"since\">\n{}</ul>\n",
        lists::page_with(&items, 0, None, "since", lists::Row::Li, |item| since_line(read, item))
    )
}

/// One entry of Recently done: when it finished, what, and its word.
type Since<'a> = (chrono::DateTime<chrono::Utc>, &'a Object, &'static str);

/// The entries of Recently done, newest first: every entry of today, or the
/// last twenty when today holds fewer (S9).
fn since_items(read: &Read) -> Vec<Since<'_>> {
    let mut rows: Vec<Since<'_>> = Vec::new();
    for object in &read.objects {
        let verb = match object.machine.as_str() {
            // A note taken is listed; a capture of several signals is listed in
            // the tray instead, where they wait (S9, S225).
            "capture" if several_signals(read, &object.id) => continue,
            "capture" => "captured",
            "signal" => match signal_verb(read, object) {
                Some(verb) => verb,
                None => continue,
            },
            "unit" => match object.config.get("life").map(String::as_str) {
                Some("merged") => "merged",
                // A chore of a shared line lands on the pass that merges it, so
                // it is never merged for a reader to see. It keeps the one
                // merged line its merge earned (60, S15, S9).
                Some("landed") if is_shared_line_chore(object) => "merged",
                Some("dropped") => "dropped",
                _ => continue,
            },
            "bolt" => match object.config.get("life").map(String::as_str) {
                Some("landed") => "landed",
                _ => continue,
            },
            "intent" => match object.config.get("life").map(String::as_str) {
                Some("closed") | Some("done") => "closed",
                Some("dropped") => "dropped",
                _ => continue,
            },
            _ => continue,
        };
        // A capture was captured the moment it was put; its regions moving
        // after that are its reading, not the capture.
        let at = match object.machine.as_str() {
            "capture" => object.entered_at.values().min(),
            _ => object.entered_at.values().max(),
        };
        let Some(at) = at.copied() else {
            continue;
        };
        rows.push((at, object, verb));
    }
    if rows.is_empty() {
        return Vec::new();
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    // Today is the host's own day, where the operator is (S9).
    let today = read.status.at.with_timezone(&chrono::Local).date_naive();
    let days: Vec<chrono::NaiveDate> = rows
        .iter()
        .map(|(at, _, _)| at.with_timezone(&chrono::Local).date_naive())
        .collect();
    rows.truncate(recently_done(&days, today));
    rows
}

/// What became of a signal, in the word Recently done lists it under. It is
/// read from the move record standing on it, as everything else the page says
/// about a note is, so what the operator just did is on the list at once and
/// not a tick later; the signal's own state answers where no record is here to
/// say (107, S9, S224a).
fn signal_verb(read: &Read, signal: &Object) -> Option<&'static str> {
    // A note the operator put on a bolt already open reads as added, where
    // build now reads built (S224a, S9).
    let built = |unit: &str| match dock::added_to(read, unit) {
        Some(_) => "added",
        None => "built",
    };
    if let Some(moved) = read.moves.get(&signal.id) {
        let named = moved.names();
        return match moved.word() {
            "route" if named.starts_with("ask/") => Some("asked"),
            "route" => Some(built(named)),
            "join" => Some("intent"),
            "drop" => Some("dropped"),
            _ => None,
        };
    }
    match signal.config.get("move").map(String::as_str) {
        Some("routed") if dock::routes_an_ask(signal) => Some("asked"),
        Some("routed") => signal.record.get("route").and_then(|value| value.as_str()).map(built),
        Some("joined") => Some("intent"),
        Some("dropped") => Some("dropped"),
        _ => None,
    }
}

/// One line of Recently done, drawn when it is shown.
fn since_line(read: &Read, (at, object, verb): &Since<'_>) -> String {
    // A note is its signal's words, and so is the capture that holds it.
    let said = match object.machine.as_str() {
        "signal" => signals::text_of(object),
        "capture" => read
            .children(&object.id)
            .find(|o| o.machine == "signal")
            .and_then(signals::text_of)
            .or_else(|| object.record.get("raw").and_then(|v| v.as_str()).map(pointed)),
        _ => None,
    };
    // An object named by its id says which repository it is in, greyed, as a
    // slip does: two chores merged onto two shared lines are both `chore-1` by
    // name (209, S231).
    let pre = match said {
        Some(_) => None,
        None => repository_of(&object.id),
    };
    let name = said.map(|s| clipped_to(&s, 56)).unwrap_or_else(|| name_of(&object.id).to_string());
    format!(
        "<li><span class=\"v {verb}\">{verb}</span><a class=\"grow\" href=\"#dock-{id}\">{pre}{name}</a><span class=\"t\">{when}</span></li>\n",
        id = escape(&object.id),
        pre = pre
            .map(|r| format!("<span class=\"pre\">{} · </span>", escape(r)))
            .unwrap_or_default(),
        name = escape(&name),
        when = escape(&at.format("%H:%M").to_string()),
    )
}

/// How many of what finished the Recently done list shows, newest first:
/// every entry of today, or the last twenty when today holds fewer. What falls
/// off stays on record (S9). `days` is each entry's local day, newest first.
pub(crate) fn recently_done(days: &[chrono::NaiveDate], today: chrono::NaiveDate) -> usize {
    const THE_LAST: usize = 20;
    let of_today = days.iter().filter(|day| **day == today).count();
    of_today.max(days.len().min(THE_LAST))
}

/// A text cut at a length, from its beginning (S215).
fn clipped_to(said: &str, at: usize) -> String {
    match said.chars().count() > at {
        false => said.trim().to_string(),
        true => format!("{}…", said.chars().take(at - 1).collect::<String>().trim_end()),
    }
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
         data-number=\"{number}\" data-object=\"{object}\" data-board=\"{board}\" data-group=\"{group}\" \
         aria-label=\"decision {number}\">\n",
        group = escape(&decision.group),
        object = escape(&decision.object),
        board = escape(board_object(read, &decision.object)),
    );
    // What it is, and where it is: the kind is the object's own machine, and
    // the phase is the lane it sits in on the board. The group is the heading
    // the card is filed under and is not repeated here (209, D16).
    let chores = chores_of(&read.objects, decision);
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
        kind = escape(match (machine, &chores) {
            (_, Some(_)) => "chores",
            ("signal", _) => "capture",
            (other, _) => other,
        }),
        phase = escape(&phase_of(machine).to_lowercase()),
    );
    // The object it concerns, as the board draws it: the repository it is in,
    // greyed, and the name. The whole id is what the link carries and what the
    // dock says; a card is read for which thing this is, and an id repeated
    // down the rail is the same four words over and over (209, 308).
    // A signal's card says what was captured, in the operator's own words
    // from the start; every other card names its object (19a, 209).
    let captured = read
        .objects
        .iter()
        .find(|o| o.id == decision.object && o.machine == "signal")
        .and_then(flywheel_domain::signals::text_of);
    match (captured, &chores) {
        // A fold of chores is headed by the name it folds by, the bolt's or
        // the repository's, and how many it holds (S231).
        (_, Some((name, held))) => {
            let _ = write!(
                out,
                "<div class=\"title\"><a class=\"object\" href=\"{link}\" data-object=\"{object}\">{name} · {n} {chores}</a></div>\n",
                link = escape(&link),
                object = escape(&decision.object),
                name = escape(name),
                n = held.len(),
                chores = counted("chores", held.len()),
            );
        }
        (Some(said), None) => {
            let _ = write!(
                out,
                "<div class=\"title\"><a class=\"object said\" href=\"{link}\" data-object=\"{object}\">“{said}”</a></div>\n",
                link = escape(&link),
                object = escape(&decision.object),
                said = escape(&clipped(&said)),
            );
        }
        (None, None) => {
            let _ = write!(
                out,
                "<div class=\"title\"><a class=\"object\" href=\"{link}\" data-object=\"{object}\">{pre}{name}</a></div>\n",
                link = escape(&link),
                object = escape(&decision.object),
                pre = repository_of(&decision.object)
                    .map(|r| format!("<span class=\"pre\">{} · </span>", escape(r)))
                    .unwrap_or_default(),
                name = escape(&shown_name(read, &decision.object)),
            );
        }
    }
    // Why it is being asked: what the machine's own `shows:` names for this
    // decision kind, and how long it has stood (15, 11, 18).
    if let Some(said) = read.why.get(&decision.id).filter(|said| !said.is_empty()) {
        // What the card is read by carries the weight of its line: a proposed
        // intent is read by how many signals it cites (S5, D16).
        let parts: Vec<String> = said
            .iter()
            .map(|part| match part.ends_with(" signals") || part.ends_with(" signal") {
                true => format!("<b>{}</b>", escape(part)),
                false => escape(part),
            })
            .collect();
        let _ = write!(out, "<p class=\"tail why\">{}</p>\n", parts.join(" · "));
    }
    if let Some(away) = read.away.get(&decision.object) {
        let _ = write!(
            out,
            "<p class=\"away\" data-away-host=\"{}\">{}</p>\n",
            escape(&away.host),
            escape(&away_said(away))
        );
    }
    // A fold of chores lists them as lettered rows, each its chore's own
    // decision (S232).
    if let Some(rows) = rows_of(&read.objects, decision) {
        out.push_str(&chore_rows(&number, &rows));
    }
    // What is being asked, as a sentence, above the controls that answer it
    // (S220), and the intents a gathering covers (188).
    out.push_str(&asks_line(read, decision));
    out.push_str(&gathered(read, decision, &number));
    out.push_str("<div class=\"answers\">\n");
    let answers = offered(read, decision);
    let keys = asks::keys(&answers);
    for (answer, key) in answers.iter().zip(keys) {
        out.push_str(&card_control(&number, answer, &decision.object, &decision.kind, key));
    }
    out.push_str("</div>\n");
    // What has already been answered, with who gave it and when: the response
    // is recorded when it is given and applied on the next tick, so a reload
    // shows it before the decision is retracted (153, 154, 310).
    out.push_str(&answered(read, decision.number));
    out.push_str("</article>\n");
    out
}

/// The sentence a decision asks, with what the page knows of its object
/// (S220): a bolt's close says how many units are merged and asks whether to
/// land it.
fn question_of(read: &Read, decision: &DecisionInstance) -> Option<String> {
    let units_merged = read
        .objects
        .iter()
        .filter(|o| o.machine == "unit" && o.parent.as_deref() == Some(decision.object.as_str()))
        .filter(|o| o.config.get("life").map(String::as_str) == Some("merged"))
        .count();
    let intents = match decision.kind.as_str() {
        "elaboration-proposed" => covered(read, &decision.object)
            .iter()
            .map(|intent| name_of(intent).to_string())
            .collect(),
        _ => vec![],
    };
    asks::question(
        &decision.kind,
        &shown_name(read, &decision.object),
        &asks::Facts { units_merged, intents },
    )
}

/// The question as the card and the dock say it: for an elaboration, with its
/// name and its type beside it, and no type where the instance has none of
/// that name (S220, S226, 85a).
fn asks_line(read: &Read, decision: &DecisionInstance) -> String {
    let Some(asked) = question_of(read, decision) else {
        return String::new();
    };
    let typed = match decision.kind.as_str() {
        "elaboration-proposed" if !read.untyped.contains(&decision.object) => read
            .objects
            .iter()
            .find(|o| o.id == decision.object)
            .and_then(|o| o.record.get("type")?.as_str().map(str::trim).filter(|t| !t.is_empty()))
            // Its name begins with its type, so the name is the whole line
            // (S226).
            .map(|_| format!(" <span class=\"ty\">{}</span>", escape(&shown_name(read, &decision.object)))),
        _ => None,
    };
    format!("<p class=\"asks\">{}{}</p>\n", escape(&asked), typed.unwrap_or_default())
}

/// An answer on the rail's card. A bare answer is the one-tap control; an
/// answer that takes an argument — `redo: <notes>`, `bolt <name>`, `type
/// <name>` — is one tap too, opening the object in the dock where the field
/// for it is (311, S6, D16). A card with six text fields on it read as a form
/// and not as a decision; the mockup keeps the card to its words and takes
/// the argument in a box the tap opens.
fn card_control(number: &str, answer: &str, object: &str, kind: &str, key: Option<char>) -> String {
    if takes_an_argument(answer).is_none() {
        return control(number, answer, kind, key);
    }
    format!(
        "<a class=\"btn sm to-dock\" href=\"#dock-{object}\" data-answer=\"{whole}\" \
         data-decision=\"{number}\"{key_attr}>{said}…{key_hint}</a>\n",
        object = escape(object),
        whole = escape(answer),
        said = escape(&asks::label(kind, answer)),
        key_attr = key.map(|k| format!(" data-key=\"{k}\"")).unwrap_or_default(),
        key_hint = key.map(|k| format!("<span class=\"k\">{k}</span>")).unwrap_or_default(),
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
fn control(number: &str, answer: &str, kind: &str, key: Option<char>) -> String {
    let tool = crate::catalogue::ANSWER;
    // The control says what it does and how it is pressed: the label is the
    // verb, the key is on it, and the tooltip is the one line of what follows
    // (S220, S218). The value posted is still the model's own word.
    let label = escape(&asks::label(kind, answer));
    let key_attr = key.map(|k| format!(" data-key=\"{k}\"")).unwrap_or_default();
    let key_hint = key.map(|k| format!("<span class=\"k\">{k}</span>")).unwrap_or_default();
    let title = asks::does(kind, answer)
        .map(|does| format!(" title=\"{}\"", escape(does)))
        .unwrap_or_default();
    let tone = match (asks::primary(answer), asks::dismisses(answer)) {
        (true, _) => " pri",
        (_, true) => " drop",
        _ => "",
    };
    let Some(argument) = takes_an_argument(answer) else {
        // One tap, and nothing behind a hover or a keyboard (311). The control
        // posts to the one tool the chat's numbered reply grammar calls, so the
        // two surfaces share a write path (193, 194).
        return format!(
            "<form method=\"post\" action=\"/api/tools/{tool}\" class=\"answer\">\n\
             <input type=\"hidden\" name=\"decision\" value=\"{number}\">\n\
             <input type=\"hidden\" name=\"answer\" value=\"{0}\">\n\
             <button type=\"submit\" class=\"btn sm{tone}\" data-answer=\"{0}\"{key_attr}{title}>{label}{key_hint}</button>\n</form>\n",
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
         <button type=\"submit\" class=\"btn sm{tone}\" data-answer=\"{whole}\"{key_attr}{title}>{label}{key_hint}</button>\n\
         </form>\n",
        argument = escape(&argument),
        whole = escape(answer),
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
fn lane(read: &Read, id: &str, title: &str, sub: &str, machines: &[&str]) -> String {
    let mine = lane_rows(read, machines);
    lane_frame(id, &|piece| match piece {
        "head" => lane_head(read, title, sub, machines, &mine),
        "empty" => lane_empty(read, title, machines, &mine),
        group => lane_group(read, id, &group.replace('-', " "), &mine),
    })
}

/// A lane as it stands on the page: its head, a region per group and the line
/// an empty lane says, each swapped apart by an update (S221, S235).
fn lane_frame(id: &str, piece: &dyn Fn(&str) -> String) -> String {
    let mut out = format!("<div id=\"{id}-head\">{}</div>\n<div class=\"status-groups\">\n", piece("head"));
    for group in status::GROUPS {
        let group = group.replace(' ', "-");
        let _ = write!(out, "<div id=\"{id}-{group}\">{}</div>\n", piece(&group));
    }
    let _ = write!(out, "<div id=\"{id}-empty\">{}</div>\n</div>\n", piece("empty"));
    out
}

/// A lane's head: its title, how much it draws and, in Inception, the counter
/// that opens the tray, since what waits for curation belongs where curation
/// reads it (118, 110, S225).
fn lane_head(read: &Read, title: &str, sub: &str, machines: &[&str], mine: &[&status::Row]) -> String {
    let mut out = format!(
        "<div class=\"lane-h\"><h2 class=\"lane-title\">{}</h2>{}<span class=\"n\">{}</span></div>\n",
        escape(title),
        match sub.is_empty() {
            true => String::new(),
            false => format!("<span class=\"sub\">{}</span>", escape(sub)),
        },
        mine.len()
    );
    if machines.contains(&"signal") {
        out.push_str(&tray::counter(read));
    }
    out
}

/// One group of a lane, its objects fifty at a time. A group with nothing in it
/// keeps its heading and says nothing under it: four "nothing"s down a lane
/// read as the machine talking to itself, and the grouping is what 141 asks for
/// (141, D16).
fn lane_group(read: &Read, id: &str, group: &str, mine: &[&status::Row]) -> String {
    // A chore stands under its head until it merges, which on the shared line
    // is its landing; it then leaves the lane as a landed bolt's units do, and
    // Recently done carries its one merged line (S15, S9, 60).
    let held: Vec<&status::Row> = group_rows(read, group, mine)
        .into_iter()
        .filter(|row| !landed_shared_line_chore(read, row))
        .collect();
    // An accepted chore is work on a repository's shared line, not a bolt, so
    // it stands under a head of its own rather than in a ledger (S15, S75, 60).
    let (chores, rows): (Vec<&status::Row>, Vec<&status::Row>) =
        held.into_iter().partition(|row| accepted_shared_line_chore(read, row));
    let slug = group.replace(' ', "-");
    format!(
        "<section class=\"sec-h-group{empty}\" data-group=\"{slug}\">\n<h2>{group}</h2>\n{rows}{chores}</section>\n",
        empty = match rows.is_empty() && chores.is_empty() {
            true => " empty",
            false => "",
        },
        rows = lists::page_with(&rows, 0, None, &format!("{id}.{slug}"), lists::Row::Div, |row| {
            object_on_the_board(read, row, mine)
        }),
        chores = chores_heads(read, &chores),
    )
}

/// An accepted chore of a repository's shared line: a unit of the chore type
/// that hangs off no bolt and is past its proposal. A proposed one is a slip,
/// answered or dropped; an accepted one is an item on that line (60, S14, S75).
fn accepted_shared_line_chore(read: &Read, row: &status::Row) -> bool {
    let Some(object) = read.object(&row.object).filter(|o| is_shared_line_chore(o)) else {
        return false;
    };
    !matches!(
        object.config.get("life").map(String::as_str),
        None | Some("in-proposal")
            | Some("proposed")
            | Some("deferred")
            | Some("withdrawn")
            | Some("superseded")
            | Some("dropped")
            | Some("retired")
    )
}

/// A chore on a repository's or the instance's shared line: a unit of the chore
/// type hanging off no bolt, wherever it stands in its life (60, 62).
fn is_shared_line_chore(object: &Object) -> bool {
    object.machine == "unit"
        && object.record.get("type").and_then(|v| v.as_str()) == Some("chore")
        && !object.parent.as_deref().is_some_and(|parent| parent.starts_with("bolt/"))
}

/// The same chore once it has merged: on the shared line that merge is its
/// landing, so it is done and has left the lane (60, S15).
fn landed_shared_line_chore(read: &Read, row: &status::Row) -> bool {
    read.object(&row.object)
        .filter(|o| is_shared_line_chore(o))
        .is_some_and(|object| {
            matches!(object.config.get("life").map(String::as_str), Some("merged") | Some("landed"))
        })
}

/// The accepted chores of a lane, under a "chores" head per repository and
/// never drawn as a bolt: two repositories' chores read apart, and neither is a
/// ledger with a chain (S15, S75, 209).
fn chores_heads(read: &Read, chores: &[&status::Row]) -> String {
    if chores.is_empty() {
        return String::new();
    }
    let mut by_repository: BTreeMap<&str, Vec<&status::Row>> = BTreeMap::new();
    for row in chores {
        let repository = read
            .object(&row.object)
            .and_then(|o| o.record.get("repository"))
            .and_then(|v| v.as_str())
            .or_else(|| repository_of(&row.object))
            .unwrap_or("the instance");
        by_repository.entry(repository).or_default().push(row);
    }
    let mut out = String::new();
    for (repository, rows) in by_repository {
        let _ = write!(
            out,
            "<section class=\"chores-lane\" data-repository=\"{repository}\">\n\
             <div class=\"ch-head\"><span class=\"ch-k\">chores</span>\
             <span class=\"ch-repo\">{repository}</span>\
             <span class=\"ch-n\">{how_many} {counted}</span></div>\n",
            repository = escape(repository),
            how_many = rows.len(),
            counted = counted("chores", rows.len()),
        );
        for row in rows {
            out.push_str(&chore_item(read, row));
        }
        out.push_str("</section>\n");
    }
    out
}

/// One accepted chore as an item on its line: what it fixes, in the words its
/// document is named by, and what it is doing (S75, S232).
fn chore_item(read: &Read, row: &status::Row) -> String {
    let named = read
        .object(&row.object)
        .and_then(|o| o.record.get("document"))
        .and_then(|v| v.as_str())
        .map(chore_words)
        .unwrap_or_else(|| name_of(&row.object).to_string());
    let (state, _) = state_and_rest(&row.said);
    let item = format!(
        "<div class=\"ch-item\"{attributes}>\
         <a class=\"ch-nm\" href=\"#dock-{object}\">{named}</a>\
         <span class=\"ch-st\">{state}</span></div>\n",
        attributes = board_attributes(row),
        object = escape(&row.object),
        named = escape(&named),
        state = escape(state),
    );
    with_marks(item, marks(read, &row.object))
}

/// What a lane with nothing in it says, which is what to do next (S214, S225).
fn lane_empty(read: &Read, title: &str, machines: &[&str], mine: &[&status::Row]) -> String {
    let drawn: usize = status::GROUPS.iter().map(|group| group_rows(read, group, mine).len()).sum();
    if drawn > 0 {
        return String::new();
    }
    let waiting = read.status.waiting.iter().any(|w| !w.signals.is_empty());
    let said = match (machines.contains(&"signal"), waiting) {
        // What arrived is not nothing: it waits for curation, one tap up
        // (S214, S225).
        (true, true) => "No notes yet. What arrived waits for curation above; type what you noticed in the box to add a note.",
        _ => LANE_EMPTY.iter().find(|(t, _)| *t == title).map(|(_, s)| *s).unwrap_or("Nothing here."),
    };
    format!("<div class=\"quiet empty\">{}</div>\n", escape(said))
}

/// The rows a lane draws: the objects whose phase it is. A capture of several
/// signals — a transcript read, a folder imported — waits in the signals tray,
/// which the counter opens; the lane draws notes and counts what it draws (S13,
/// S225).
fn lane_rows<'a>(read: &'a Read, machines: &[&str]) -> Vec<&'a status::Row> {
    read.status
        .rows
        .iter()
        .filter(|row| !OFF_THE_BOARD.contains(&row.machine.as_str()) && row_in_lane(read, row, machines))
        .filter(|row| !(row.machine == "capture" && several_signals(read, &row.object)))
        .collect()
}

/// The rows one group of a lane draws, each object once and a part inside its
/// whole. A capture that is done stands in the rail's since list, with what
/// became of it on its page; the lane is what is moving.
fn group_rows<'a>(read: &Read, group: &str, mine: &[&'a status::Row]) -> Vec<&'a status::Row> {
    mine.iter()
        .filter(|row| row.group == group && !nested_in_its_parent(read, row, mine))
        .filter(|row| !(group == "done" && row.machine == "capture"))
        .copied()
        .collect()
}

/// A lane's id on the page, from its slot in the template.
fn lane_id(slot: &str) -> String {
    slot.trim_matches(|c| c == '{' || c == '}').to_ascii_lowercase().replace('_', "-")
}

/// A proposed unit that hangs off no bolt: a slip, which Bolt plan holds.
///
/// Every slip is read in the plan, whatever its type, and a chore of a
/// repository's shared line is one until it is accepted: only then has it a
/// line to be worked on, and only then does it stand under Construction's
/// chores head (S14, S15, 60).
fn slip_in_the_plan(read: &Read, row: &status::Row) -> bool {
    read.object(&row.object).is_some_and(|object| {
        object.machine == "unit"
            && !object.parent.as_deref().is_some_and(|parent| parent.starts_with("bolt/"))
            && matches!(
                object.config.get("life").map(String::as_str),
                Some("proposed") | Some("in-proposal")
            )
    })
}

/// Which lane this row sits in: its machine's, except for a slip, which is
/// Bolt plan's wherever its machine would otherwise put it (S14).
fn row_in_lane(read: &Read, row: &status::Row, machines: &[&str]) -> bool {
    match slip_in_the_plan(read, row) {
        true => machines.contains(&"proposal"),
        false => in_lane(&row.machine, machines),
    }
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
    read.object(object).and_then(|o| o.parent.as_deref())
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
    // Who holds an object is the hosts strip's to say, as a chip; a board that
    // says "held by" under every line reads as the machine talking to itself
    // (141, D16). A host that is not alive is raised there too.
    let _ = row;
    String::new()
}

/// What an object is doing, split into the one word a head has room for and
/// the rest of it.
///
/// `said` is one sentence, the object's own state first — "open, a claim it
/// cites moved, ready to land" for a bolt. The state is what a head says; the
/// rest belongs under it, where it can wrap. Running the whole sentence into a
/// head that is `nowrap` by design pushed it off the lane.
pub(crate) fn state_and_rest(said: &str) -> (&str, &str) {
    match said.split_once(", ") {
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
/// The board object a decision lights: the object itself where the board
/// draws it, else the nearest parent it draws — a signal's decision lights its
/// capture, which is the card the operator typed (S219).
pub(crate) fn board_object<'a>(read: &'a Read, object: &'a str) -> &'a str {
    let drawn = |id: &str| {
        read.status
            .rows
            .iter()
            .any(|r| r.object == id && !OFF_THE_BOARD.contains(&r.machine.as_str()))
    };
    let mut at = object;
    loop {
        if drawn(at) {
            return at;
        }
        match read.objects.iter().find(|o| o.id == at).and_then(|o| o.parent.as_deref()) {
            Some(parent) => at = parent,
            None => return object,
        }
    }
}

fn marks(read: &Read, object: &str) -> String {
    let standing: Vec<&DecisionInstance> = read
        .decisions
        .iter()
        .filter(|d| d.number.is_some() && board_object(read, &d.object) == object)
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
            kind = escape(asks::short(&decision.kind)),
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
        "thread bead" => format!("<div class=\"thread\">{}</div>\n", bead(read, row)),
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
    let beads: Vec<String> = children_of(read, row, lane).into_iter().map(|bead| self::bead(read, bead)).collect();
    if beads.is_empty() {
        out.push_str("<div class=\"bead queued\"><span class=\"leave\">no elaboration yet</span></div>\n");
    }
    out.push_str(&lists::page(&beads, 0, Some(&row.object), "beads", lists::Row::Div));
    out.push_str("</div>\n</article>\n");
    out
}

/// One elaboration, as a bead on its intent's thread. Each bead is its own
/// target and opens that elaboration's surface (210, S13).
fn bead(read: &Read, row: &status::Row) -> String {
    format!(
        "<div class=\"bead {state}\"{attributes}>\
         <span class=\"bk\">{machine}</span>\
         <a class=\"en\" href=\"#dock-{object}\">{name}</a>\
         <span class=\"bs\">{said}</span></div>\n",
        state = escape(&bead_state(row)),
        attributes = board_attributes(row),
        machine = escape(&row.machine),
        object = escape(&row.object),
        name = escape(&shown_name(read, &row.object)),
        said = escape(&row.said),
    )
}

/// What an object is called where it is shown: an elaboration by its type and
/// the material it was proposed from (S226), anything else by the last segment
/// of its id.
pub(crate) fn shown_name(read: &Read, object: &str) -> String {
    match read.objects.iter().find(|o| o.id == object && o.machine == "elaboration") {
        Some(elaboration) => flywheel_domain::effects::elaboration_name(elaboration, |signal| {
            read.objects
                .iter()
                .find(|o| o.id == signal)
                .and_then(signals::text_of)
                .or_else(|| read.signal_words.get(signal).cloned())
        }),
        None => name_of(object).to_string(),
    }
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
    out.push_str(&lists::page(&chain(read, &units, lane), 0, Some(&row.object), "chain", lists::Row::Span));
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

/// A bolt's units as its chain draws them, an arrow before every one but the
/// first (S15).
fn chain(read: &Read, units: &[&status::Row], lane: &[&status::Row]) -> Vec<String> {
    units
        .iter()
        .enumerate()
        .map(|(at, unit)| match at {
            0 => lg_unit(read, unit, lane),
            _ => format!("<span class=\"lg-arrow\">→</span>\n{}", lg_unit(read, unit, lane)),
        })
        .collect()
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
            // Each work item is a board object of its own, so a click on it
            // opens the item and not the unit around it (S219).
            // The session running the item, where one is: its chip carries the
            // link that says where its pane is (S53, S234).
            let session = read.sessions.values().find(|s| s.item == item.object);
            let _ = write!(
                out,
                "<span class=\"it\"{attributes}><span class=\"wi\">{name}</span>\
                 <span class=\"sc\">{program}<span>{said}</span>{pane}</span></span>",
                attributes = board_attributes(item),
                name = escape(name_of(&item.object)),
                said = escape(&item.said),
                // The program and the model the session runs, where the page
                // has a record of one (S53).
                program = match session.map(said_program).filter(|said| !said.is_empty()) {
                    Some(named) => format!("<span class=\"ag\">{}</span>", escape(&named)),
                    None => String::new(),
                },
                pane = match session {
                    Some(session) => pane_link(session, &read.served_by),
                    None => String::new(),
                },
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
    // A slip hangs off no ledger, so it says which repository it is in, greyed,
    // as a card's title does: two chores of two shared lines are both
    // `chore-1` by name (209, S231).
    let _ = write!(
        out,
        "<span class=\"sl-k\">{machine}</span>\
         <a class=\"sl-n\" href=\"#dock-{object}\">{pre}{name}</a>\
         <span class=\"sl-to\">{said}</span>\n",
        machine = escape(&row.machine),
        object = escape(&row.object),
        pre = repository_of(&row.object)
            .map(|r| format!("<span class=\"pre\">{} · </span>", escape(r)))
            .unwrap_or_default(),
        name = escape(name_of(&row.object)),
        said = escape(&row.said),
    );
    let items = children_of(read, row, lane);
    for item in items {
        let _ = write!(
            out,
            "<span class=\"sl-to\"{attributes}><span class=\"arrow\">→</span> {name} · {said}</span>\n",
            attributes = board_attributes(item),
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
        .and_then(|r| {
            r.get("assertion")
                .or_else(|| r.get("excerpt"))
                .and_then(|value| value.as_str())
                .map(String::from)
                .or_else(|| r.get("raw").and_then(|value| value.as_str()).map(pointed))
        })
        .filter(|said| !said.trim().is_empty())
        .map(|said| clipped(&said))
        .unwrap_or_else(|| row.object.clone());
    let by = record
        .and_then(|r| r.get("asserted_by"))
        .and_then(|value| value.as_str())
        .filter(|by| !by.trim().is_empty());
    // Where it came from and who said it; what to do with it is the rail's
    // (19a, 141).
    let source = record
        .and_then(|r| r.get("source"))
        .and_then(|value| value.as_str())
        .filter(|s| !s.trim().is_empty());
    let mut under = match source {
        Some(source) => format!("from {}", escape(source_name(source))),
        None => escape(&row.said),
    };
    if let Some(by) = by {
        let _ = write!(&mut under, " · {}", escape(by));
    }
    // A note nothing has moved carries the line saying who reads it next and
    // its four controls, and asks nothing (19a, S224).
    let hand = match row.machine.as_str() {
        "capture" => hand::controls(read, &row.object),
        _ => String::new(),
    };
    let next = match hand.is_empty() {
        true => String::new(),
        false => format!("<span class=\"next\">{}</span>", escape(&hand::line(read))),
    };
    let reader = match row.machine.as_str() {
        "capture" => reader_chip(read, &row.object),
        _ => String::new(),
    };
    // A note the operator can act on takes the focus: Tab reaches it, its own
    // letters press its verbs and Enter opens its page (S233, S218).
    let focus = match hand.is_empty() {
        true => String::new(),
        false => format!(" tabindex=\"0\" data-note=\"{}\"", escape(&row.object)),
    };
    format!(
        "<div class=\"quote\"{attributes}{focus}>\
         <q><a href=\"#dock-{object}\">{said}</a></q>\
         <span class=\"qm\">{under}</span>{reader}{next}{hand}</div>\n",
        attributes = board_attributes(row),
        object = escape(&row.object),
        said = escape(&said),
    )
}

/// What a capture with no words of its own is called: the material it points
/// at, and a path by its file (111).
pub(crate) fn pointed(raw: &str) -> String {
    match raw.starts_with('/') {
        true => raw.rsplit('/').next().unwrap_or(raw).to_string(),
        false => raw.to_string(),
    }
}

/// A session chip's pane link: where the pane is, and how to reach it (S53,
/// S234, 174, 196, 232).
///
/// The page cannot drive a terminal and never tries to: the link opens a
/// popover naming the herdr session the pane is in, the host it runs on and the
/// pane by its session id, with the two lines to copy. Everything it says is
/// written here, into the link's own attributes, so the popover fetches nothing
/// and the page carries no second copy of the session (310a).
///
/// A pane that is gone says so and offers nothing to copy (68).
pub(crate) fn pane_link(session: &Session, served_by: &str) -> String {
    let gone = match (&session.pane, session.exit.as_deref()) {
        (None, _) => Some("lost".to_string()),
        (Some(_), Some(exit @ ("done" | "stalled" | "invalid"))) => Some(exit.to_string()),
        _ => None,
    };
    if let Some(exit) = gone {
        let said = match (&session.exit_at, exit.as_str()) {
            (Some(at), "done") => format!("no pane · the session exited at {}", when_short(at)),
            (Some(at), "lost") => format!("no pane · the session was lost at {}", when_short(at)),
            (Some(at), other) => format!("no pane · the session {other} at {}", when_short(at)),
            (None, "lost") => "no pane · the session was lost without an exit".to_string(),
            (None, other) => format!("no pane · the session {other}"),
        };
        return format!(
            "<button type=\"button\" class=\"pane\" data-pane=\"\" data-sess-id=\"{id}\" \
             data-sess-host=\"{host}\" data-sess-gone=\"{said}\" \
             title=\"where this pane is, and how to reach it\">pane ↗</button>",
            id = escape(&session.id),
            host = escape(&session.host),
            said = escape(&said),
        );
    }
    // A session recorded before panes were addressed by name names none: it is
    // in the operator's own session, which the page does not presume to name.
    let named = session.herdr_session.clone().unwrap_or_default();
    let remote = !served_by.is_empty() && !session.host.is_empty() && session.host != served_by;
    let attach = match (remote, named.is_empty()) {
        (_, true) => String::new(),
        (true, false) => format!("herdr --remote {} --session {named}", session.host),
        (false, false) => format!("herdr session attach {named}"),
    };
    format!(
        "<button type=\"button\" class=\"pane\" data-pane=\"{pane}\" data-sess-id=\"{id}\" \
         data-sess-host=\"{host}\" data-sess-session=\"{named}\"{remote} \
         data-sess-attach=\"{attach}\" data-sess-focus=\"herdr agent focus {id}\" \
         title=\"where this pane is, and how to reach it\">pane ↗</button>",
        pane = escape(session.pane.as_deref().unwrap_or_default()),
        id = escape(&session.id),
        host = escape(&session.host),
        named = escape(&named),
        remote = match remote {
            true => " data-sess-remote=\"1\"",
            false => "",
        },
        attach = escape(&attach),
    )
}

/// What a session chip names it by: the program it runs and the model it was
/// started with, by their short names — `claude · fable` — read from the
/// session's record and never from the program itself (S53, 173, 183).
///
/// A record naming no model says the program alone, and one naming neither
/// says nothing: a chip never names a model a session is not running.
pub(crate) fn said_program(session: &Session) -> String {
    let Some(kind) = session.kind.as_deref().filter(|kind| !kind.is_empty()) else {
        return String::new();
    };
    match session.model.as_deref().filter(|model| !model.is_empty()) {
        Some(model) => format!("{kind} · {}", flywheel_domain::sessions::model_short(kind, model)),
        None => kind.to_string(),
    }
}

/// A moment in the operator's own terms, short enough for a chip.
fn when_short(at: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(at)
        .map(|t| t.with_timezone(&chrono::Utc).format("%H:%M").to_string())
        .unwrap_or_else(|_| at.to_string())
}

/// The chip a capture carries while its reader reads it, as a session's chip
/// shows on the board; it goes when the reader has delivered and the capture's
/// signals stand in its place (115, 217e).
fn reader_chip(read: &Read, capture: &str) -> String {
    let Some(object) = read.objects.iter().find(|o| o.id == capture) else {
        return String::new();
    };
    if object.config.get("reading").map(String::as_str) != Some("reading") {
        return String::new();
    }
    let at = |ending: &str| {
        object
            .config
            .iter()
            .find(|(region, _)| region.starts_with("reading.") && region.ends_with(ending))
            .map(|(_, state)| state.as_str())
    };
    let (dot, said) = match at(".activity").or_else(|| at(".life")) {
        Some("working") => ("working", "reading"),
        Some("blocked") => ("blocked", "blocked"),
        Some("exited") => ("working", "delivered"),
        _ => ("working", "starting"),
    };
    format!(
        "<span class=\"sc {dot}\"><span class=\"dot {dot}\"></span>\
         <span class=\"ag\">capture-reader</span><span class=\"ac\">{said}</span></span>"
    )
}


/// Where a capture came from, as a person names it: a capture typed on this
/// page is from the console, which is what tells it from the dispatch agent
/// and the chat sinks (S223).
pub(crate) fn source_name(source: &str) -> &str {
    match source {
        "page" | "console" => "the console",
        other => other,
    }
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
    out.push_str(&lists::page(&curator_rows(read), 0, Some(tray::ID), "curate", lists::Row::Div));
    out.push_str(
        "<button class=\"btn pri\" type=\"submit\" id=\"curate-go\">submit the moves</button>\n\
         </form>\n</section>\n",
    );
    out
}

/// The curator's surface's rows: each unmoved signal with the standing moves as
/// controls, fifty at a time (110, 116, 310a, S235).
fn curator_rows(read: &Read) -> Vec<String> {
    read.unmoved
        .iter()
        .map(|signal| {
            let mut out = String::new();
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
                "<input type=\"text\" class=\"cur-target\" name=\"target.{0}\" list=\"curate-intents\" \
                 aria-label=\"what the move for {0} names\" \
                 placeholder=\"the intent or claim it names\">\n",
                escape(&signal.id)
            );
            out.push_str(&ask_fields(read, signal));
            out.push_str("</div>\n");
    
            out
        })
        .collect()
}

/// Where a route's ask is given on the curator's surface, shown when the move
/// is route: the repository it is for — the one the instance tracks as itself,
/// several to pick from — and the words, which start as the signal's own
/// (28, 116).
fn ask_fields(read: &Read, signal: &signals::Signal) -> String {
    let id = escape(&signal.id);
    let repository = match read.repositories.as_slice() {
        [] => {
            return "<span class=\"cur-ask\"><span class=\"none\">no repository to ask in yet · \
                    add one to flywheel.yaml</span></span>\n"
                .to_string()
        }
        [one] => format!(
            "<span class=\"repo\">{0}</span><input type=\"hidden\" name=\"repository.{id}\" value=\"{0}\">",
            escape(one)
        ),
        many => {
            let mut picked = format!(
                "<select name=\"repository.{id}\" aria-label=\"the repository the ask for {id} is for\">"
            );
            for repository in many {
                let _ = write!(picked, "<option value=\"{0}\">{0}</option>", escape(repository));
            }
            picked.push_str("</select>");
            picked
        }
    };
    format!(
        "<span class=\"cur-ask\">ask in {repository}<input type=\"text\" name=\"ask.{id}\" \
         aria-label=\"what the ask for {id} asks for\" placeholder=\"what to ask for\" \
         value=\"{}\"></span>\n",
        escape(&signal.excerpt)
    )
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
    // The dock page a link named, and no other: every other page is fetched
    // from the host when its object is opened, so what a load costs is what is
    // on screen and never the instance (308, 310a, S235).
    if let Some(page) = read.opened.as_deref().and_then(|opened| dock_surface(read, opened, true)) {
        out.push_str(&page);
    }
    out
}

/// The next rows of one list from `from`, as its `more` fetches them: the page's
/// own lists by name, a lane's group as `<lane>.<group>`, and an object's lists
/// by the object and the list's name; `None` for a list there is not (310a,
/// S235).
pub fn list_part(read: &Read, object: Option<&str>, list: &str, from: usize) -> Option<String> {
    use lists::{page, Row};
    match (object, list) {
        (None, "since") => Some(lists::page_with(&since_items(read), from, None, "since", Row::Li, |item| since_line(read, item))),
        (None, "log") => Some(page(&log_rows(read), from, None, "log", Row::Li)),
        (None, named) => {
            let (lane, group) = named.split_once('.')?;
            let (_, machines) = LANES.iter().find(|(slot, _)| lane_id(slot) == lane)?;
            let group = status::GROUPS.iter().find(|g| g.replace(' ', "-") == group)?;
            let mine = lane_rows(read, machines);
            let rows = group_rows(read, group, &mine);
            Some(lists::page_with(&rows, from, None, named, Row::Div, |row| object_on_the_board(read, row, &mine)))
        }
        (Some(tray::ID), "rows") => Some(tray::rows_part(read, from)),
        (Some(tray::ID), "curate") => Some(page(&curator_rows(read), from, Some(tray::ID), "curate", Row::Div)),
        (Some(id), "beads" | "chain") => {
            let row = read.status.rows.iter().find(|r| r.object == id)?;
            let (_, machines) = LANES.iter().find(|(_, machines)| in_lane(&row.machine, machines))?;
            let mine = lane_rows(read, machines);
            let children = children_of(read, row, &mine);
            match list {
                "beads" => {
                    let beads: Vec<String> = children.iter().map(|bead| self::bead(read, bead)).collect();
                    Some(page(&beads, from, Some(id), list, Row::Div))
                }
                _ => Some(page(&chain(read, &children, &mine), from, Some(id), list, Row::Span)),
            }
        }
        (Some(id), other) => dock::list_part(read, id, other, from),
    }
}

/// One object's dock page, or the signals tray's, as the drawer fetches it when
/// its object is opened; `None` for an id the instance does not hold (310a,
/// S235, S225).
pub fn dock_page(read: &Read, id: &str) -> Option<String> {
    dock_surface(read, id, false)
}

fn dock_surface(read: &Read, id: &str, opened: bool) -> Option<String> {
    if id == tray::ID {
        return Some(tray::surface(read, opened));
    }
    read.objects.iter().find(|o| o.id == id).map(|object| surface(read, object, opened))
}

/// One object's surface in the dock: its header in the form of the object, its
/// body and its answers (S27, S28).
fn surface(read: &Read, object: &Object, opened: bool) -> String {
    let mut out = String::new();
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
    );
    out.push_str(&dock_head(read, object, row));
    out.push_str(&dock::body(read, object, row));
    // A note's verbs stand at rest in the drawer's footer, where a decision's
    // page carries its answers: everywhere else they wait for the hand, and on
    // a phone this is the only place they are (S233, S28, 311).
    if matches!(object.machine.as_str(), "capture" | "signal") {
        let hand = hand::controls(read, &object.id);
        if !hand.is_empty() {
            let _ = write!(
                out,
                "<div class=\"dk-f\"><div class=\"dk-answers hand-h\" data-answerable=\"false\">\n\
                 <div class=\"fl\">your hand<span class=\"r\">or leave it to curation</span></div>\n\
                 {hand}</div></div>\n"
            );
        }
    }
    out.push_str("</article>\n");
    out
}

/// A surface's header takes the form of the object (S27): the mockup's own
/// `dk-` header for the silhouette its kind has, with the kind, the phase it
/// sits in, the standing decision's number as a marker (S17), the name, and
/// what the object is doing under it.
fn dock_head(read: &Read, object: &Object, row: Option<&status::Row>) -> String {
    let mut out = String::new();
    let form = match silhouette(&object.machine) {
        // A landed bolt's header is the record's, as its board form is (S28).
        "ledger" if dock::is_landed(object) => "dk-record",
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
        machine = escape(match object.machine.as_str() {
            "signal" => "capture",
            "bolt" if dock::is_landed(object) => "landed",
            other => other,
        }),
        // Where it sits is the phase it is in, so a slip says the plan's, as
        // the board puts it there (209, S14).
        phase = escape(
            &match row.filter(|row| slip_in_the_plan(read, row)) {
                Some(_) => "Bolt plan".to_string(),
                None => phase_of(&object.machine).to_string(),
            }
            .to_lowercase()
        ),
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
    let _ = write!(out, "<h2>{}</h2>\n", escape(&dock::title(read, object)));
    let said = dock::subtitle(read, object, row);
    if !said.is_empty() {
        let _ = write!(out, "<div class=\"tail\">{}</div>\n", escape(&said));
    }
    // What is asked of this object and the controls that answer it, under the
    // title where the number already is: one place to read, one place to
    // press (S220, S27, 308).
    out.push_str(&dock_answers(read, &dock::answers_object(read, object)));
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
        return String::from("<div class=\"dk-answers none\" data-answerable=\"false\"></div>\n");
    }
    let mut out = String::from("<div class=\"dk-answers\" data-answerable=\"true\">\n");
    for decision in standing {
        let number = decision.number.map(|n| n.to_string()).unwrap_or_default();
        out.push_str(&asks_line(read, decision));
        out.push_str(&gathered(read, decision, &number));
        let _ = write!(out, "<div class=\"answers\" data-number=\"{number}\">\n");
        let answers = offered(read, decision);
        let keys = asks::keys(&answers);
        for (answer, key) in answers.iter().zip(keys) {
            out.push_str(&control(&number, answer, &decision.kind, key));
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
            "<div class=\"pal-empty\">nothing sent yet</div>",
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
    let rows = log_rows(read);
    match rows.is_empty() {
        true => "<li class=\"say\"><span>nothing sent yet</span></li>\n".to_string(),
        false => lists::page(&rows, 0, None, "log", lists::Row::Li),
    }
}

fn log_rows(read: &Read) -> Vec<String> {
    read.answered
        .iter()
        .flat_map(|(number, given)| {
            given.iter().map(move |one| {
                format!(
                    "<li class=\"say\" data-decision=\"{number}\"><time>{}</time>\
                     <span>{number} → {} · {}</span></li>\n",
                    escape(&short(&one.given_at)),
                    escape(&one.answer),
                    escape(&one.given_by)
                )
            })
        })
        .collect()
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
