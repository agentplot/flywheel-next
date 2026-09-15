//! The status view: every object grouped, with its holder, its runner and that
//! host's liveness, in one place for the instance (141, 143, 146).
//!
//! It is derived from `list` and `read` alone, so it is the same view whatever
//! serves it — the page, a running host, or the committed file read with no
//! host running at all (132, 145, D12). The projection stores nothing it did
//! not read: drift between it and its source is rewritten from the source on
//! the next tick (77, 142).

use chrono::{DateTime, Duration, Utc};
use flywheel_atoms::{ReadPoint, Records, Scope, StatusView};
use flywheel_engine::{Definitions, Object};
use std::collections::BTreeMap;

/// The four groups of 141, in the order the view shows them.
pub const GROUPS: [&str; 4] = ["queued", "in progress", "waiting on the operator", "done"];

/// One object as the status view shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub object: String,
    pub machine: String,
    pub group: String,
    pub states: Vec<String>,
    /// The same, as a person reads it: what the object is doing, in the
    /// model's own words and without the dotted paths the config is keyed by
    /// (141, 310).
    pub said: String,
    /// The host holding this object's lease, where one does (128, 143).
    pub holder: Option<String>,
    /// What runs the session under it, where one runs (93b, 217c).
    pub runner: Option<String>,
    /// The holder's liveness: alive, stale, away or gone (146, 150, 150a).
    pub liveness: Option<String>,
    /// What the object's lease is in: `free`, `uncovered`, `acknowledged`,
    /// `held`, `stale` or `expired` (`engine/lease.yaml`).
    ///
    /// A lease is the machinery's own bookkeeping and raises no decision on the
    /// rail, so this is where it is read: an object no host's declaration
    /// covers says `uncovered` here rather than taking a number the operator
    /// cannot answer with anything but "seen" (79, 141, 149, 310).
    pub lease: Option<String>,
    /// The question, the answer and the note kept on the object, in order (144).
    pub discussion: Vec<(String, String)>,
    /// The object this one is part of, where it is part of one: an
    /// elaboration's intent, a unit's bolt, a signal's capture. What the view
    /// draws it inside (209).
    pub parent: Option<String>,
    /// When it was made among its siblings, so a thread's beads and a ledger's
    /// units are drawn in order (209).
    pub created: u64,
    /// What a capture or a signal says, in the words it was said in (19a, 209).
    pub words: Option<String>,
}

/// The whole view, before it is a page.
#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    pub as_of: ReadPoint,
    pub rows: Vec<Row>,
    /// The signals with no move, by source, with the oldest one's date. They
    /// are shown here and none is discarded (118).
    pub unmoved: Vec<crate::signals::UnmovedSource>,
    /// When the view was read, which is what an unmoved signal's age is
    /// counted against (118).
    pub at: DateTime<Utc>,
}

impl Status {
    pub fn group(&self, name: &str) -> Vec<&str> {
        self.rows
            .iter()
            .filter(|r| r.group == name)
            .map(|r| r.object.as_str())
            .collect()
    }
}

/// What an object is doing, as a person reads it: one sentence, its own state
/// first and then what is moving under it, in the operator's words (141, 310,
/// S214).
///
/// The config is keyed by the dotted path of every region and state above an
/// entry, and printing its states puts "landed · line removed · place removed ·
/// services gone" under a bolt, which is the machine talking to itself. So each
/// machine's own state is said in words, and of what stands under it only what
/// someone would act on or wait for is said: a session working or lost, a
/// branch in conflict, a close offered. A branch current, a place ready and
/// services declared are the quiet a sentence leaves out.
pub fn said(object: &Object) -> String {
    let (state, rest) = told(object);
    match rest.is_empty() {
        true => state,
        false => format!("{state}, {rest}"),
    }
}

/// The sentence in its two parts: the object's own state, which a head has room
/// for, and what else is so, which goes under it.
pub fn told(object: &Object) -> (String, String) {
    let config = &object.config;
    let live = |region: &str| config.get(region).filter(|_| entered(config, region)).map(String::as_str);
    let mut rest: Vec<String> = Vec::new();

    let own = ["life", "run", "move", "reading", "standing"]
        .into_iter()
        .find_map(|region| config.get(region).map(|state| (region, state.as_str())))
        .or_else(|| object.top_state().map(|state| ("", state)));
    let state = match own {
        // A work item in its type's stages is in the stage it stands at.
        Some(("life", "in-type")) => match live("life.in-type.stages") {
            Some(stage @ ("passed" | "stopped")) => stage.to_string(),
            Some(stage) => format!("in {stage}"),
            None => "started".to_string(),
        },
        Some((_, state)) => state_in_words(&object.machine, state),
        None => String::new(),
    };

    // A stage's run, where it is doing something other than running sessions.
    for run in config.iter().filter(|(region, _)| region.ends_with(".run") && entered(config, region)).map(|(_, run)| run) {
        match run.as_str() {
            "resolving" => rest.push("starting".into()),
            "sent-back" => rest.push("sent back".into()),
            _ => {}
        }
    }
    match (object.machine.as_str(), live("life.open.citations"), live("life.open.close")) {
        ("bolt", citations, close) => {
            if citations == Some("moved") {
                rest.push("a claim it cites moved".into());
            }
            match close {
                Some("offered") => rest.push("ready to land".into()),
                Some("held") => rest.push("held open".into()),
                _ => {}
            }
        }
        ("intent", _, close) => match close {
            Some("offered") => rest.push("ready to close".into()),
            Some("declined") => rest.push("kept open".into()),
            _ => {}
        },
        _ => {}
    }
    if live("life.working.work") == Some("stalled") {
        rest.push("stalled".into());
    }
    if let Some(said) = session_in_words(config) {
        rest.push(said);
    }
    if let Some(said) = live("line.line.life").and_then(|line| match line {
        "taking" => Some("bringing main into its branch"),
        "conflict" => Some("its branch conflicts with main"),
        "conflict-stalled" => Some("its branch is stuck on a conflict"),
        "request-open" => Some("its pull request is open"),
        "land-failed" if state != "landing failed" => Some("landing failed"),
        _ => None,
    }) {
        rest.push(said.into());
    }
    for place in config.iter().filter(|(region, _)| region.ends_with("place.life") && entered(config, region)).map(|(_, place)| place) {
        let said = match place.as_str() {
            "preparing" => "its place is being made",
            "behind" => "its place is behind its branch",
            "conflict" => "its place has a conflict",
            "conflict-stalled" => "its place is stuck on a conflict",
            "merging" => "merging its place",
            "held" => "its place is held",
            _ => continue,
        };
        rest.push(said.into());
    }
    (state, rest.join(", "))
}

/// An object's own state, in words.
fn state_in_words(machine: &str, state: &str) -> String {
    let said = match (machine, state) {
        ("bolt", "land-failed") => "landing failed",
        ("unit", "in-proposal") => "in a proposal",
        ("unit", "waiting") => "waiting on what it depends on",
        ("unit", "claim-moved") => "held for a moved claim",
        ("unit", "in-flight") => "being built",
        ("work-item", "placing") | ("elaboration", "placing") => "making its place",
        ("work-item", "archiving") => "archiving its change",
        ("elaboration", "writing-back") => "writing back",
        ("intent", "archive-failed") => "archiving failed",
        ("capture", "reading") => "being read",
        ("signal", "unmoved") => "not moved yet",
        ("signal", "challenging") => "challenging a claim",
        ("signal", "joined") => "joined into an intent",
        ("signal", "attached") => "attached to an intent",
        ("curation", "applying") | ("planning", "applying") => "applying what it found",
        ("repository", "creating") => "being created",
        ("repository", "registering") => "being registered",
        ("repository", "covering") => "being added to the App",
        ("repository", "uncovered") => "not covered by the App",
        ("instance", "absent") => "not made yet",
        ("instance", "awaiting-app") => "waiting for the App's key",
        ("package", "needs-secret") => "waiting for a secret",
        ("package", "checking-secrets") => "checking its secrets",
        (_, other) => return other.replace('-', " "),
    };
    said.to_string()
}

/// The session running under an object, where one is and it says something:
/// working, idle, asking, finished or lost (65–72, 146, `session.yaml`).
fn session_in_words(config: &BTreeMap<String, String>) -> Option<String> {
    let (region, life) = config
        .iter()
        .filter(|(region, _)| region.ends_with("session.life") || region.ends_with("sessions.life"))
        .find(|(region, _)| entered(config, region))?;
    let under = |name: &str| config.get(&format!("{region}.alive.{name}")).map(String::as_str);
    Some(
        match life.as_str() {
            "requested" => "waiting for its session",
            "starting" => "its session is starting",
            "alive" => match (under("presence"), under("activity")) {
                (Some("gone"), _) => "its session's pane is gone",
                (_, Some("idle")) => "its session is idle",
                (_, Some("blocked")) => "its session is asking a question",
                (_, Some("exited")) => "its session finished",
                _ => "its session is working",
            },
            "exited" => "its session finished",
            "lost" => "its session was lost",
            _ => return None,
        }
        .to_string(),
    )
}

/// Whether every state above this region is the one the object is in.
///
/// A key alternates region and state — `life.in-type.stages.build.run` is the
/// region `run` under the state `build` of the region `stages` under the state
/// `in-type` of the region `life` — so the states above it are at the odd
/// positions, and each one has to be what the config records for the path that
/// reaches it.
fn entered(config: &std::collections::BTreeMap<String, String>, region: &str) -> bool {
    let segments: Vec<&str> = region.split('.').collect();
    let mut at = 1;
    while at < segments.len() {
        let above = segments[..at].join(".");
        if config.get(&above).map(String::as_str) != Some(segments[at]) {
            return false;
        }
        at += 2;
    }
    true
}

/// Which group an object is in, from its states and what holds it.
///
/// Done first, because an object that has finished is finished whatever else is
/// recorded about it; then the operator's own queue, because a decision
/// standing on an object is the thing a person came to the view to see; then
/// what a host is actually working; and everything else is queued.
pub fn group_of(defs: &Definitions, object: &Object, held: bool, working: bool) -> String {
    let machine = defs
        .for_object(&object.machine)
        .or_else(|| defs.get(&object.machine));
    let mut any = false;
    let mut all_final = true;
    let mut decides = false;
    let mut done = false;
    if let Some(machine) = machine {
        for (name, region) in &machine.regions {
            let Some(state) = object.config.get(name).and_then(|s| region.states.get(s)) else {
                continue;
            };
            any = true;
            if !state.is_final {
                all_final = false;
            }
            if state.decision.is_some() {
                decides = true;
            }
            // What the state puts on the SINCE list says what became of the
            // object: done, merged, landed or dropped is finished work (14).
            if state
                .tail
                .as_deref()
                .is_some_and(|t| matches!(t, "done" | "merged" | "landed" | "dropped"))
            {
                done = true;
            }
        }
        // A decision raised in a nested region is the operator's too.
        for (path, name) in &object.config {
            if let Some((_region, state)) = flywheel_engine::tick::state_def(defs, object, path) {
                let _ = name;
                if state.decision.is_some() {
                    decides = true;
                }
            }
        }
    }
    if any && (all_final || done) {
        return "done".into();
    }
    if decides {
        return "waiting on the operator".into();
    }
    if held || working {
        return "in progress".into();
    }
    "queued".into()
}

/// The status of one instance, read through the record operations alone.
pub fn read<S: Records>(
    store: &S,
    defs: &Definitions,
    as_of: &ReadPoint,
    now: DateTime<Utc>,
    stale: Duration,
    gone: Duration,
) -> anyhow::Result<Status> {
    // With no material in hand there are no unmoved signals to count; a caller
    // that has the blueprints uses `read_with` (118).
    let none: std::collections::BTreeMap<String, String> = Default::default();
    read_with(store, defs, as_of, now, stale, gone, &none)
}

/// The same, with the signal material the unmoved signals are counted from
/// (118, `blueprints.yaml` layout).
#[allow(clippy::too_many_arguments)]
pub fn read_with<S: Records, R: crate::signals::Reads + ?Sized>(
    store: &S,
    defs: &Definitions,
    as_of: &ReadPoint,
    now: DateTime<Utc>,
    stale: Duration,
    gone: Duration,
    files: &R,
) -> anyhow::Result<Status> {
    let objects = store.list_records(&Scope::All)?;
    let hosts = store.hosts()?;
    let liveness = |host: &str| -> Option<String> {
        let record = hosts.iter().find(|h| h.host == host)?;
        let since = now - record.last_seen;
        Some(
            match (
                since < stale,
                since < gone,
                record.intermittent,
            ) {
                (true, _, _) => "alive",
                (false, true, _) => "stale",
                (false, false, true) => "away",
                (false, false, false) => "gone",
            }
            .to_string(),
        )
    };
    // The sessions running, by the object they are under.
    let mut runners: BTreeMap<String, String> = BTreeMap::new();
    for fact in objects.iter().filter(|o| o.id.starts_with("fact/session/")) {
        let running = fact.record.get("started_at").is_some_and(|v| !v.is_null())
            && !fact.record.get("ended_at").is_some_and(|v| !v.is_null());
        if !running {
            continue;
        }
        let Some(place) = fact.record.get("place").and_then(|v| v.as_str()) else {
            continue;
        };
        let runner = fact
            .record
            .get("runner")
            .and_then(|v| v.as_str())
            .unwrap_or("operator");
        runners.insert(place.to_string(), runner.to_string());
    }

    // A capture whose signal has no move yet is not finished: its own reading
    // is over the moment the signal exists, but nothing has been made of it.
    // It stands queued in inception, a quote, until curation or the operator's
    // hand on the capture moves its signal, and it asks the operator nothing
    // (19a, 107, 110, 116).
    let awaiting_a_move: std::collections::BTreeSet<String> = objects
        .iter()
        .filter(|o| o.machine == "signal" && o.config.get("move").map(String::as_str) == Some("unmoved"))
        .filter_map(|o| o.parent.clone())
        .collect();

    let mut rows = Vec::new();
    for object in &objects {
        // The status view is of the work. The machinery's own objects — the
        // rail, the sinks, the hosts, the leases and the bindings' facts — are
        // not what a person came to it for; the hosts have a surface of their
        // own (239, 240).
        if !crate::leases::leasable(object) {
            continue;
        }
        let lease = store.leases(&object.id)?;
        let lease_state = lease.as_ref().map(|l| l.state.clone());
        let holder = lease.map(|l| l.holder);
        let runner = runners.get(&object.id).cloned();
        let group = match object.machine == "capture" && awaiting_a_move.contains(&object.id) {
            true => "queued".to_string(),
            false => group_of(defs, object, holder.is_some(), runner.is_some()),
        };
        let mut states: Vec<String> = object
            .config
            .iter()
            .map(|(region, state)| format!("{region}: {state}"))
            .collect();
        states.sort();
        // A note is its signal's words and a quote its own (19a, 209).
        let words = match object.machine.as_str() {
            "signal" => crate::signals::text_of(object),
            "capture" => objects
                .iter()
                .find(|o| o.machine == "signal" && o.parent.as_deref() == Some(object.id.as_str()))
                .and_then(crate::signals::text_of),
            _ => None,
        };
        rows.push(Row {
            parent: object.parent.clone(),
            created: object.created,
            words,
            object: object.id.clone(),
            machine: object.machine.clone(),
            group,
            said: said(object),
            states,
            liveness: holder.as_deref().and_then(liveness),
            lease: lease_state,
            holder,
            runner,
            discussion: discussion(store, &object.id),
        });
    }
    Ok(Status {
        as_of: as_of.clone(),
        rows,
        unmoved: crate::signals::unmoved_by_source(files),
        at: now,
    })
}

/// The question, the answer and the note kept on an object, in the order they
/// were said. They stay with the object, so they are still there once the
/// session that said them is gone (144).
pub fn discussion<S: Records>(store: &S, object: &str) -> Vec<(String, String)> {
    store
        .thread(object)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| matches!(e.kind.as_str(), "question" | "answer" | "note"))
        .map(|e| {
            let said = e
                .fields
                .get("text")
                .or_else(|| e.fields.get("question"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            (e.kind, said)
        })
        .collect()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// The view as the one page an instance has (141): the as-of point first, then
/// the four groups in order, each object with what holds it.
pub fn render(status: &Status) -> StatusView {
    let mut body = String::new();
    body.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n");
    body.push_str("<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    body.push_str("<title>flywheel · status</title>\n</head>\n<body>\n");
    body.push_str(&format!(
        "<p class=\"as-of\">as of commit {} at {}</p>\n",
        escape(&status.as_of.mark),
        status.as_of.at.to_rfc3339()
    ));
    // The signals with no move, by source and by age. Nothing here is
    // discarded: a signal nobody has judged is one the operator has not seen
    // yet (118).
    body.push_str("<section id=\"unmoved-signals\">\n<h2>unmoved signals</h2>\n");
    if status.unmoved.is_empty() {
        body.push_str("<p class=\"none\">nothing unmoved</p>\n");
    }
    for source in &status.unmoved {
        let age = source
            .oldest
            .as_deref()
            .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
            .map(|at| (status.at - at.with_timezone(&Utc)).num_days())
            .map(|days| format!(", oldest {days}d"))
            .unwrap_or_default();
        body.push_str(&format!(
            "<p class=\"unmoved\" data-source=\"{}\" data-count=\"{}\">{} from {}{age}</p>\n",
            escape(&source.source),
            source.count,
            source.count,
            escape(&source.source)
        ));
    }
    body.push_str("</section>\n");
    for group in GROUPS {
        let rows: Vec<&Row> = status.rows.iter().filter(|r| r.group == group).collect();
        body.push_str(&format!(
            "<section id=\"{}\">\n<h2>{group}</h2>\n",
            group.replace(' ', "-")
        ));
        if rows.is_empty() {
            body.push_str("<p class=\"none\">nothing</p>\n");
        }
        // An object drawn inside the one it is part of is not drawn again on
        // its own; one whose parent sits in another group is drawn where it
        // sits, since the group is its phase and a form never says that (209).
        for row in rows.iter().filter(|r| !part_of_one_here(r, &rows)) {
            render_row(&mut body, row, &rows, false);
        }
        body.push_str("</section>\n");
    }
    body.push_str("</body>\n</html>\n");
    StatusView {
        as_of: status.as_of.clone(),
        body,
    }
}

/// The form an object takes on the status view (209, S62). A decision is the
/// rail's and never drawn here; a proposal is a sheet with its unit proposals
/// hanging off it, an intent a thread with its elaborations as beads in order,
/// a bolt a ledger with its units as slips in order and a landed bolt a record
/// of them, a capture a note with its words and a signal a quote. A work item
/// is a line under its unit, a session a row of its own, and what the
/// machinery keeps beside the work — curation, a place, a line — a plain entry.
pub fn form_of(row: &Row) -> &'static str {
    match (row.machine.as_str(), row.group.as_str()) {
        ("bolt", "done") => "record",
        ("bolt", _) => "ledger",
        ("intent", _) => "thread",
        ("elaboration", _) => "bead",
        ("proposal", _) => "sheet",
        ("unit", _) => "slip",
        ("work-item", _) => "item",
        ("capture", _) => "note",
        ("signal", _) => "quote",
        ("session", _) => "session",
        _ => "entry",
    }
}

/// What a form's parts are called when they hang off it.
fn parts_of(form: &str) -> &'static str {
    match form {
        "thread" => "beads",
        "ledger" | "record" => "slips",
        "slip" => "items",
        "sheet" => "unit-proposals",
        "note" => "quotes",
        _ => "parts",
    }
}

/// Whether a row is drawn inside another row of the same group.
fn part_of_one_here(row: &Row, group: &[&Row]) -> bool {
    row.parent
        .as_deref()
        .is_some_and(|parent| group.iter().any(|r| r.object == parent))
}

/// One object in its form, with its parts inside it in the order they were
/// made (209).
fn render_row(body: &mut String, row: &Row, group: &[&Row], nested: bool) {
    let form = form_of(row);
    let tag = match (nested, form) {
        (true, _) => "li",
        (false, "quote") => "blockquote",
        (false, _) => "article",
    };
    body.push_str(&format!(
        "<{tag} class=\"{form}\" id=\"{}\" data-machine=\"{}\" data-holder=\"{}\" data-runner=\"{}\" data-liveness=\"{}\" data-lease=\"{}\">\n",
        escape(&row.object),
        escape(&row.machine),
        escape(row.holder.as_deref().unwrap_or("none")),
        escape(row.runner.as_deref().unwrap_or("none")),
        escape(row.liveness.as_deref().unwrap_or("none")),
        escape(row.lease.as_deref().unwrap_or("free")),
    ));
    body.push_str(&format!("<h3>{}</h3>\n", escape(&row.object)));
    // A note and a quote are their words, as they were said (19a, 209).
    if let Some(words) = row.words.as_deref().filter(|_| matches!(form, "note" | "quote")) {
        body.push_str(&format!("<p class=\"words\">“{}”</p>\n", escape(words)));
    }
    body.push_str(&format!("<p class=\"state\">{}</p>\n", escape(&row.said)));
    body.push_str(&format!(
        "<p class=\"holder\">held by {} ({})</p>\n",
        escape(row.holder.as_deref().unwrap_or("no host")),
        escape(row.liveness.as_deref().unwrap_or("no host"))
    ));
    if let Some(runner) = &row.runner {
        body.push_str(&format!("<p class=\"runner\">run by the {}</p>\n", escape(runner)));
    }
    // An object no host's declaration covers is said here, in words, and not
    // as a number on the rail: nothing about it is the operator's to answer
    // beyond widening a declaration (79, 141, 149).
    if row.lease.as_deref() == Some("uncovered") {
        body.push_str(
            "<p class=\"lease\">no host's declaration covers this; it waits until one \
             does (149)</p>\n",
        );
    }
    for (kind, said) in &row.discussion {
        body.push_str(&format!(
            "<p class=\"said\" data-kind=\"{}\">{}</p>\n",
            escape(kind),
            escape(said)
        ));
    }
    let mut parts: Vec<&&Row> = group
        .iter()
        .filter(|r| r.parent.as_deref() == Some(row.object.as_str()))
        .collect();
    parts.sort_by_key(|r| (r.created, r.object.clone()));
    if !parts.is_empty() {
        body.push_str(&format!("<ol class=\"{}\">\n", parts_of(form)));
        for part in parts {
            render_row(body, part, group, true);
        }
        body.push_str("</ol>\n");
    }
    body.push_str(&format!("</{tag}>\n"));
}
