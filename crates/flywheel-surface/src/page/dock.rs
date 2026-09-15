//! The dock's pages, one per kind (S28): what a developer wants to know about
//! the object opened — its words, its branch, its host, its sessions, its
//! commits — and nothing about how the machinery works (S214).

use super::{deliverables, discussion, escape, name_of, sec, Read};
use flywheel_atoms::CommitRef;
use flywheel_domain::status;
use flywheel_engine::Object;
use std::fmt::Write as _;

/// A session as the dock shows it, read from the sessions binding's fact and
/// the session's own thread (141, 144, S28).
#[derive(Debug, Clone, Default)]
pub struct Session {
    pub id: String,
    pub item: String,
    pub host: String,
    pub runner: String,
    pub agent: Option<String>,
    pub pane: Option<String>,
    pub started: Option<String>,
    pub exit: Option<String>,
    pub exit_at: Option<String>,
    pub deliverables: Vec<String>,
    pub question: Option<String>,
}

/// A moment as a person reads it.
fn when(at: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(at)
        .map(|t| t.with_timezone(&chrono::Utc).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| at.to_string())
}

fn field<'a>(object: &'a Object, name: &str) -> Option<&'a str> {
    object.record.get(name).and_then(|v| v.as_str()).filter(|s| !s.is_empty())
}

fn row(label: &str, value: &str) -> String {
    match value.is_empty() {
        true => String::new(),
        false => format!(
            "<div class=\"row\"><span class=\"st\">{}</span><span class=\"grow\">{}</span></div>\n",
            escape(label),
            value
        ),
    }
}

fn mono(text: &str) -> String {
    format!("<span class=\"mono\">{}</span>", escape(text))
}

fn link(read: &Read, id: &str, text: &str) -> String {
    let _ = read;
    format!("<a class=\"elaboration\" href=\"#dock-{}\">{}</a>", escape(id), escape(text))
}

/// Whether a signal's route names an ask for planning rather than a unit that
/// was built (116, 28).
pub fn routes_an_ask(signal: &Object) -> bool {
    field(signal, "route")
        .and_then(flywheel_domain::asks::id_named)
        .is_some()
}

/// The object whose answers the page's foot carries: a capture answers
/// through its signal, which is where the decision stands (19a).
pub fn answers_object(read: &Read, object: &Object) -> String {
    match object.machine.as_str() {
        "capture" => read
            .objects
            .iter()
            .find(|o| o.machine == "signal" && o.parent.as_deref() == Some(object.id.as_str()))
            .map(|o| o.id.clone())
            .unwrap_or_else(|| object.id.clone()),
        _ => object.id.clone(),
    }
}

/// What stands under the title: for a capture, what became of it or that it
/// waits on the operator; for everything else, the status view's sentence.
pub fn subtitle(read: &Read, object: &Object, row: Option<&status::Row>) -> String {
    match object.machine.as_str() {
        "capture" | "signal" => {
            let (_, signal) = capture_and_signal(read, object);
            match signal.and_then(|s| s.config.get("move").map(String::as_str)) {
                Some("routed") if signal.is_some_and(routes_an_ask) => "asked".into(),
                Some("routed") => "built".into(),
                Some("joined") | Some("attached") => "an intent".into(),
                Some("dropped") => "dropped".into(),
                _ => "waiting on you".into(),
            }
        }
        _ => row.map(|r| r.said.clone()).unwrap_or_default(),
    }
}

/// The page's title: a capture's words, everything else its name.
pub fn title(read: &Read, object: &Object) -> String {
    match object.machine.as_str() {
        "capture" | "signal" => {
            let (capture, signal) = capture_and_signal(read, object);
            let said = signal
                .and_then(flywheel_domain::signals::text_of)
                .or_else(|| capture.and_then(|c| field(c, "raw").map(String::from)))
                .unwrap_or_else(|| name_of(&object.id).to_string());
            format!("“{}”", super::clipped(&said))
        }
        _ => name_of(&object.id).to_string(),
    }
}

fn capture_and_signal<'a>(read: &'a Read, object: &'a Object) -> (Option<&'a Object>, Option<&'a Object>) {
    match object.machine.as_str() {
        "capture" => {
            let signal = read
                .objects
                .iter()
                .find(|o| o.machine == "signal" && o.parent.as_deref() == Some(object.id.as_str()));
            (Some(object), signal)
        }
        "signal" => {
            let capture = object
                .parent
                .as_deref()
                .and_then(|p| read.objects.iter().find(|o| o.id == p));
            (capture, Some(object))
        }
        _ => (None, None),
    }
}

/// The body of one page (S28).
pub fn body(read: &Read, object: &Object, row: Option<&status::Row>) -> String {
    let mut out = String::from("<div class=\"dk-b\">\n");
    if let Some(away) = read.away.get(&object.id) {
        let _ = write!(
            out,
            "<p class=\"away\" data-away-host=\"{}\">{}</p>\n",
            escape(&away.host),
            escape(&away.said())
        );
    }
    // Why the decision on it is being asked, in the machine's own words; the
    // rail card carries the same line (15, 308).
    let why: Vec<String> = read
        .decisions
        .iter()
        .filter(|d| d.object == object.id)
        .filter_map(|d| read.why.get(&d.id))
        .filter(|said| !said.is_empty())
        .map(|said| format!("<div class=\"dtext\">{}</div>\n", escape(&said.join(" · "))))
        .collect();
    out.push_str(&sec("the decision", "", &why.join("")));

    match object.machine.as_str() {
        "capture" | "signal" => out.push_str(&capture_page(read, object)),
        "bolt" => out.push_str(&bolt_page(read, object, row)),
        "unit" => out.push_str(&unit_page(read, object, row)),
        "work-item" => out.push_str(&item_page(read, object, row)),
        "elaboration" => out.push_str(&elaboration_page(read, object, row)),
        _ => out.push_str(&plain_page(read, object)),
    }

    out.push_str(&sec("delivered", "", &deliverables(read, &object.id)));
    let said = row.map(discussion).unwrap_or_default();
    out.push_str(&sec("notes and questions", "", &said));
    out.push_str("</div>\n");
    out
}

// -------------------------------------------------------------- elaboration

/// An elaboration is a surface of its own, reached from its intent: its type
/// and where it stands, the document it writes and what its sessions left in
/// the intent's change directory, its sessions with what they last did, and
/// the intents a gathering covers (210, 187, 188, 65–68). Its decision, when
/// one is pending, is the page's own head and "the decision" above.
fn elaboration_page(read: &Read, elaboration: &Object, row_: Option<&status::Row>) -> String {
    let mut out = String::new();
    let mut facts = String::new();
    if let Some(kind) = field(elaboration, "type") {
        facts.push_str(&row("type", &escape(kind)));
    }
    if let Some(said) = row_.map(|r| r.said.as_str()).filter(|s| !s.is_empty()) {
        facts.push_str(&row("stands", &escape(said)));
    }
    if let Some(intent) = elaboration.parent.as_deref() {
        facts.push_str(&row("intent", &link(read, intent, name_of(intent))));
    }
    if let Some(document) = field(elaboration, "document") {
        facts.push_str(&row("document", &mono(document)));
    }
    if let Some(host) = row_.and_then(|r| r.holder.as_deref()) {
        facts.push_str(&row("host", &mono(host)));
    }
    out.push_str(&sec("the elaboration", "", &facts));

    // A gathering is one elaboration over several intents, proposed on the
    // first and covering the rest (188).
    let covers: Vec<String> = elaboration
        .record
        .get("covers")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    if covers.len() > 1 {
        let listed: Vec<String> = covers.iter().map(|i| link(read, i, name_of(i))).collect();
        out.push_str(&sec("gathers", "", &row("intents", &listed.join(" · "))));
    }

    let sessions: Vec<&Session> = read
        .sessions
        .values()
        .filter(|s| s.item == elaboration.id)
        .collect();
    match sessions.is_empty() {
        true => out.push_str(&sec("sessions", "", "<p class=\"none\">No session yet: one starts when it is approved.</p>\n")),
        false => out.push_str(&sec("sessions", "", &session_rows(&sessions))),
    }
    out
}

// ------------------------------------------------------------------ capture

fn capture_page(read: &Read, object: &Object) -> String {
    let (capture, signal) = capture_and_signal(read, object);
    let mut out = String::new();
    let said = signal
        .and_then(flywheel_domain::signals::text_of)
        .or_else(|| capture.and_then(|c| field(c, "raw").map(String::from)))
        .unwrap_or_default();
    if !said.is_empty() {
        let _ = write!(out, "<blockquote class=\"dtext said\">{}</blockquote>\n", escape(&said));
    }
    let mut from = Vec::new();
    if let Some(source) = capture.and_then(|c| field(c, "source")) {
        from.push(format!("from {}", escape(super::source_name(source))));
    }
    if let Some(by) = capture.and_then(|c| field(c, "captured_by")) {
        from.push(format!("by {}", escape(by)));
    }
    if let Some(at) = capture.and_then(|c| field(c, "event_at")) {
        from.push(when(at));
    }
    if !from.is_empty() {
        let _ = write!(out, "<div class=\"row\"><span class=\"grow\">{}</span></div>\n", from.join(" · "));
    }
    let mut became = String::new();
    if let Some(signal) = signal {
        match signal.config.get("move").map(String::as_str) {
            Some("routed") => match field(signal, "route") {
                // An ask holds its words, so what became of the signal is
                // those words and the repository they were asked of (28, 116).
                Some(route) if routes_an_ask(signal) => {
                    let ask = flywheel_domain::asks::id_named(route)
                        .and_then(|id| read.asks.iter().find(|ask| ask.id == id));
                    became = match ask {
                        Some(ask) => format!(
                            "asked in <span class=\"repo\">{}</span> <q>{}</q>",
                            escape(&ask.repository),
                            escape(&ask.text)
                        ),
                        None => format!("asked: {}", mono(route)),
                    };
                }
                Some(unit) => became = format!("built: {}", link(read, unit, unit)),
                None => {}
            },
            Some("joined") | Some("attached") => {
                let intent = read.objects.iter().find(|o| {
                    o.machine == "intent"
                        && o.record
                            .get("signals")
                            .and_then(|v| v.as_array())
                            .is_some_and(|s| s.iter().any(|v| v.as_str() == Some(signal.id.as_str())))
                });
                if let Some(intent) = intent {
                    became = format!("intent: {}", link(read, &intent.id, name_of(&intent.id)));
                }
            }
            Some("dropped") => became = "dropped".into(),
            _ => {}
        }
    }
    out.push_str(&sec("what became of it", "", &row("", &became)));
    out
}

// --------------------------------------------------------------------- bolt

fn session_rows(sessions: &[&Session]) -> String {
    let mut out = String::from("<div class=\"rows\">\n");
    for s in sessions {
        let mut bits: Vec<String> = Vec::new();
        if let Some(agent) = &s.agent {
            bits.push(format!("agent {}", mono(agent)));
        }
        if let Some(pane) = &s.pane {
            bits.push(format!("pane {}", mono(pane)));
        }
        bits.push(format!("host {}", mono(&s.host)));
        if let Some(started) = &s.started {
            bits.push(format!("started {}", when(started)));
        }
        match (&s.exit, &s.exit_at) {
            (Some(exit), Some(at)) => bits.push(format!("<b>{}</b> {}", escape(exit), when(at))),
            (Some(exit), None) => bits.push(format!("<b>{}</b>", escape(exit))),
            _ => bits.push("running".into()),
        }
        if !s.deliverables.is_empty() {
            bits.push(format!("delivered {}", escape(&s.deliverables.join(", "))));
        }
        let _ = write!(
            out,
            "<div class=\"row\"><span class=\"st\">{}</span><span class=\"grow\">{}</span></div>\n",
            escape(s.id.rsplit('/').nth(1).unwrap_or("session")),
            bits.join(" · ")
        );
        if let Some(q) = &s.question {
            let _ = write!(out, "<div class=\"row\"><span class=\"st\">asks</span><span class=\"grow\">{}</span></div>\n", escape(q));
        }
    }
    out.push_str("</div>\n");
    out
}

fn commit_rows(commits: &[CommitRef]) -> String {
    if commits.is_empty() {
        return String::new();
    }
    let mut out = String::from("<table class=\"deliv commits\"><thead><tr><th>commit</th><th>subject</th><th>by</th><th>when</th></tr></thead><tbody>\n");
    for c in commits {
        let _ = write!(
            out,
            "<tr><td class=\"mono\">{}</td><td>{}</td><td>{}</td><td class=\"mono\">{}</td></tr>\n",
            escape(&c.hash),
            escape(&c.subject),
            escape(&c.author),
            escape(&when(&c.at))
        );
    }
    out.push_str("</tbody></table>\n");
    out
}

fn place_rows(read: &Read, object: &str) -> String {
    place_rows_of(read, object, true)
}

/// The place an object works in: its directory, and its branch where the
/// branch is not the object's own — a bolt's place is the checkout of the
/// bolt's branch, and saying the branch twice under two names confused the
/// reader (S222).
fn place_rows_of(read: &Read, object: &str, with_branch: bool) -> String {
    let key = format!("fact/place/{object}#own");
    let Some(fact) = read.objects.iter().find(|o| o.id == key) else {
        return String::new();
    };
    let mut out = String::new();
    if let Some(dir) = field(fact, "dir") {
        let standing = match fact.record.get("exists").and_then(|v| v.as_bool()) {
            Some(true) => "",
            _ => " · removed",
        };
        out.push_str(&row("place", &format!("{}{}", mono(dir), standing)));
    }
    if let Some(branch) = field(fact, "branch").filter(|_| with_branch) {
        out.push_str(&row("branch", &mono(branch)));
    }
    out
}

/// What a bolt's commit list is: its branch's own, or main's latest once the
/// branch has landed and holds nothing main does not.
fn commits_title(read: &Read, bolt: &str) -> &'static str {
    match read.commits_are_mains.contains(bolt) {
        true => "landed · main's latest commits",
        false => "commits on the branch",
    }
}

fn bolt_page(read: &Read, bolt: &Object, row_: Option<&status::Row>) -> String {
    let mut out = String::new();
    let repository = field(bolt, "repository").unwrap_or_default();
    let mut line = String::new();
    line.push_str(&row("repository", &mono(repository)));
    line.push_str(&row("branch", &mono(&bolt.id)));
    if let Some(host) = row_.and_then(|r| r.holder.as_deref()) {
        line.push_str(&row("host", &mono(host)));
    }
    line.push_str(&place_rows_of(read, &bolt.id, false));
    out.push_str(&sec("the branch", "", &line));

    let units: Vec<&Object> = read
        .objects
        .iter()
        .filter(|o| o.machine == "unit" && o.parent.as_deref() == Some(bolt.id.as_str()))
        .collect();
    let mut chain = String::from("<div class=\"rows\">\n");
    for unit in &units {
        let state = unit.config.get("life").cloned().unwrap_or_default();
        let kind = field(unit, "type").unwrap_or("unit");
        let _ = write!(
            chain,
            "<div class=\"row\"><span class=\"st\">{}</span><span class=\"grow\"><span class=\"nm\">{}</span> <span class=\"m\">{}</span></span></div>\n",
            escape(&state),
            link(read, &unit.id, name_of(&unit.id)),
            escape(kind)
        );
    }
    chain.push_str("</div>\n");
    if !units.is_empty() {
        out.push_str(&sec("units", "", &chain));
    }

    let sessions: Vec<&Session> = read
        .sessions
        .values()
        .filter(|s| {
            read.objects
                .iter()
                .find(|o| o.id == s.item)
                .and_then(|item| item.parent.clone())
                .and_then(|unit| read.objects.iter().find(|o| o.id == unit))
                .and_then(|unit| unit.parent.clone())
                .as_deref()
                == Some(bolt.id.as_str())
        })
        .collect();
    if !sessions.is_empty() {
        out.push_str(&sec("sessions", "", &session_rows(&sessions)));
    }
    if let Some(commits) = read.commits.get(&bolt.id) {
        out.push_str(&sec(commits_title(read, &bolt.id), "", &commit_rows(commits)));
    }
    out
}

// --------------------------------------------------------------------- unit

fn unit_page(read: &Read, unit: &Object, row_: Option<&status::Row>) -> String {
    let mut out = String::new();
    if let Some(job) = field(unit, "subject") {
        let _ = write!(out, "<blockquote class=\"dtext said\">{}</blockquote>\n", escape(job));
    }
    let mut facts = String::new();
    if let Some(kind) = field(unit, "type") {
        facts.push_str(&row("type", &escape(kind)));
    }
    if let Some(bolt) = unit.parent.as_deref() {
        facts.push_str(&row("bolt", &link(read, bolt, bolt)));
    }
    if let Some(repository) = field(unit, "repository") {
        facts.push_str(&row("repository", &mono(repository)));
    }
    if let Some(host) = row_.and_then(|r| r.holder.as_deref()) {
        facts.push_str(&row("host", &mono(host)));
    }
    if let Some(document) = field(unit, "document") {
        facts.push_str(&row("from", &link(read, document, document)));
    }
    out.push_str(&sec("the unit", "", &facts));

    let items: Vec<&Object> = read
        .objects
        .iter()
        .filter(|o| o.machine == "work-item" && o.parent.as_deref() == Some(unit.id.as_str()))
        .collect();
    let mut rows = String::from("<div class=\"rows\">\n");
    for item in &items {
        let stage = item.config.get("life.in-type.stages").cloned().unwrap_or_default();
        let life = item.config.get("life").cloned().unwrap_or_default();
        let _ = write!(
            rows,
            "<div class=\"row\"><span class=\"st\">{}</span><span class=\"grow\">{} · {}</span></div>\n",
            escape(&life),
            link(read, &item.id, name_of(&item.id)),
            match stage.is_empty() {
                true => String::new(),
                false => format!("stage {}", escape(&stage)),
            }
        );
    }
    rows.push_str("</div>\n");
    if !items.is_empty() {
        out.push_str(&sec("work items", "", &rows));
    }
    let sessions: Vec<&Session> = read
        .sessions
        .values()
        .filter(|s| items.iter().any(|i| i.id == s.item))
        .collect();
    if !sessions.is_empty() {
        out.push_str(&sec("sessions", "", &session_rows(&sessions)));
    }
    if let Some((bolt, commits)) = unit.parent.as_deref().and_then(|b| read.commits.get(b).map(|c| (b, c))) {
        out.push_str(&sec(commits_title(read, bolt), "", &commit_rows(commits)));
    }
    out
}

// ---------------------------------------------------------------- work item

fn item_page(read: &Read, item: &Object, row_: Option<&status::Row>) -> String {
    let mut out = String::new();
    let mut facts = String::new();
    if let Some(unit) = item.parent.as_deref() {
        facts.push_str(&row("unit", &link(read, unit, unit)));
        if let Some(job) = read.objects.iter().find(|o| o.id == unit).and_then(|u| field(u, "subject")) {
            let _ = write!(out, "<blockquote class=\"dtext said\">{}</blockquote>\n", escape(job));
        }
    }
    if let Some(stage) = item.config.get("life.in-type.stages") {
        facts.push_str(&row("stage", &escape(stage)));
    }
    if let Some(kind) = field(item, "type") {
        facts.push_str(&row("type", &escape(kind)));
    }
    if let Some(host) = row_.and_then(|r| r.holder.as_deref()) {
        facts.push_str(&row("host", &mono(host)));
    }
    facts.push_str(&place_rows(read, &item.id));
    out.push_str(&sec("the work item", "", &facts));
    let sessions: Vec<&Session> = read.sessions.values().filter(|s| s.item == item.id).collect();
    if !sessions.is_empty() {
        out.push_str(&sec("sessions", "", &session_rows(&sessions)));
    }
    let bolt = item
        .parent
        .as_deref()
        .and_then(|u| read.objects.iter().find(|o| o.id == u))
        .and_then(|u| u.parent.clone());
    if let Some((bolt, commits)) = bolt.as_deref().and_then(|b| read.commits.get(b).map(|c| (b, c))) {
        out.push_str(&sec(commits_title(read, bolt), "", &commit_rows(commits)));
    }
    out
}

// -------------------------------------------------------------------- plain

fn plain_page(read: &Read, object: &Object) -> String {
    let mut out = String::new();
    // A proposed intent shows its weight: the signals it cites, in their own
    // words (109, 118).
    if let Some(weight) = read.weight.get(&object.id) {
        let mut cited = format!(
            "<p class=\"weight\" data-signals=\"{}\" data-sources=\"{}\">{} signal{}{}{}</p>\n<ul class=\"cited\">\n",
            weight.count,
            escape(&weight.sources.join(" ")),
            weight.count,
            match weight.count {
                1 => "",
                _ => "s",
            },
            match weight.sources.is_empty() {
                true => String::new(),
                false => format!(" from {}", escape(&weight.sources.join(", "))),
            },
            weight.span().map(|s| format!(", {}", escape(&s))).unwrap_or_default()
        );
        for signal in &weight.signals {
            let said = read
                .objects
                .iter()
                .find(|o| &o.id == signal)
                .and_then(flywheel_domain::signals::text_of)
                .unwrap_or_else(|| signal.clone());
            let _ = write!(cited, "<li class=\"signal\">{}</li>\n", link(read, signal, &super::clipped(&said)));
        }
        cited.push_str("</ul>\n");
        out.push_str(&sec("what it cites", "", &cited));
    }
    // What it holds, in the order it was made: an intent lists its
    // elaborations in order, each with its type, and each opens its own
    // surface (210).
    let mut children: Vec<&Object> = read
        .objects
        .iter()
        .filter(|o| o.parent.as_deref() == Some(object.id.as_str()) && o.machine != "signal")
        .collect();
    children.sort_by_key(|o| (o.created, o.id.clone()));
    if !children.is_empty() {
        let mut held = String::from("<ol class=\"elaborations\">\n");
        for child in &children {
            let said = read
                .status
                .rows
                .iter()
                .find(|r| r.object == child.id)
                .map(|r| r.said.clone())
                .unwrap_or_default();
            let kind = field(child, "type").map(|t| format!("{} · ", escape(t))).unwrap_or_default();
            let _ = write!(
                held,
                "<li>{}<span class=\"r\">{kind}{}</span></li>\n",
                link(read, &child.id, name_of(&child.id)),
                escape(&said)
            );
        }
        held.push_str("</ol>\n");
        let title = match object.machine.as_str() {
            "intent" => "elaborations",
            _ => "holds",
        };
        out.push_str(&sec(title, "", &held));
    }
    if let Some(parent) = &object.parent {
        out.push_str(&sec("part of", "", &row("", &link(read, parent, parent))));
    }
    out
}
