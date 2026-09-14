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

/// What an object is doing, as a person reads it (141, 310).
///
/// The config is keyed by the dotted path of every region and state above an
/// entry, and printing those paths verbatim puts
/// `life.in-type.stages.build.run.sessions.life.alive.presence: unknown` on a
/// card, which says nothing to anyone. Three rules make it a sentence.
///
/// A state whose ancestors are not the ones the object is actually in is not
/// what it is doing: a machine keeps what a branch it has left last stood in,
/// and `session.life: lost` beside `session.life.alive.activity: idle` reads as
/// a contradiction because only the first is live. An entry whose value merely
/// names which sub-machine runs — `place: place`, `work: session` — says
/// nothing the entries under it do not say better. And what is left is said by
/// the region it is in rather than by the path that reaches it.
pub fn said(object: &Object) -> String {
    let mut out: Vec<String> = Vec::new();
    for (region, state) in &object.config {
        if !entered(&object.config, region) || selector(region, state) {
            continue;
        }
        // A life whose stages are running is saying only that they are; the
        // stage itself is the answer, and it is on the next line.
        if object.config.contains_key(&format!("{region}.{state}.stages")) {
            continue;
        }
        // A presence nobody has looked for says nothing either way (146).
        if region.ends_with(".presence") && state == "unknown" {
            continue;
        }
        // A nested region standing where it stands when nothing is happening
        // — a close not offered, material settled, a line current, services
        // declared — is the quiet a sentence leaves out; the object's own
        // state and what is moving are what it says (141, D16).
        if region.contains('.') && matches!(state.as_str(), "not-offered" | "settled" | "current" | "declared" | "none") {
            continue;
        }
        out.push(match noun(region) {
            Some(noun) => format!("{noun} {state}"),
            None => state.clone(),
        });
    }
    out.join(" · ")
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

/// Whether an entry only names which sub-machine is running, which the entries
/// under it say better: `place: place`, `line: line`, `work: session`,
/// `run: sessions`.
fn selector(region: &str, state: &str) -> bool {
    let leaf = region.rsplit('.').next().unwrap_or(region);
    leaf == state || leaf == "work" || leaf == "run"
}

/// What to call a state, from the region it is in. `life` is the word every
/// machine uses for its own, so the state above it is what names it: the region
/// `place.place.life` is the place, and `…sessions.life` is the session.
fn noun(region: &str) -> Option<String> {
    let segments: Vec<&str> = region.split('.').collect();
    let leaf = match segments.len() {
        1 => segments[0],
        len if segments[len - 1] == "life" => segments[len - 2],
        len => segments[len - 1],
    };
    match leaf {
        // The object's own life needs no word in front of it.
        "life" => None,
        // A stage is what the item is in.
        "stages" => Some("in".into()),
        // The activity is the session's doing, and saying so twice is noise.
        "activity" => None,
        "sessions" => Some("session".into()),
        other => Some(other.replace('_', " ")),
    }
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

    // A capture whose signal has no move yet is waiting on the operator: its
    // own reading is over the moment the signal exists, but the move is a
    // person's — a curation session's, or the control on the capture — and
    // the board groups it by what it waits on, not by the reading (19, 107,
    // 116).
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
            true => "waiting on the operator".to_string(),
            false => group_of(defs, object, holder.is_some(), runner.is_some()),
        };
        let mut states: Vec<String> = object
            .config
            .iter()
            .map(|(region, state)| format!("{region}: {state}"))
            .collect();
        states.sort();
        rows.push(Row {
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
        for row in rows {
            body.push_str(&format!(
                "<article id=\"{}\" data-machine=\"{}\" data-holder=\"{}\" data-runner=\"{}\" data-liveness=\"{}\" data-lease=\"{}\">\n",
                escape(&row.object),
                escape(&row.machine),
                escape(row.holder.as_deref().unwrap_or("none")),
                escape(row.runner.as_deref().unwrap_or("none")),
                escape(row.liveness.as_deref().unwrap_or("none")),
                escape(row.lease.as_deref().unwrap_or("free")),
            ));
            body.push_str(&format!("<h3>{}</h3>\n", escape(&row.object)));
            body.push_str(&format!("<p class=\"state\">{}</p>\n", escape(&row.said)));
            body.push_str(&format!(
                "<p class=\"holder\">held by {} ({})</p>\n",
                escape(row.holder.as_deref().unwrap_or("no host")),
                escape(row.liveness.as_deref().unwrap_or("no host"))
            ));
            if let Some(runner) = &row.runner {
                body.push_str(&format!("<p class=\"runner\">run by the {}</p>\n", escape(runner)));
            }
            // An object no host's declaration covers is said here, in words, and
            // not as a number on the rail: nothing about it is the operator's
            // to answer beyond widening a declaration (79, 141, 149).
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
            body.push_str("</article>\n");
        }
        body.push_str("</section>\n");
    }
    body.push_str("</body>\n</html>\n");
    StatusView {
        as_of: status.as_of.clone(),
        body,
    }
}
