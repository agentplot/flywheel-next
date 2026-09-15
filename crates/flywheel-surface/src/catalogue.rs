//! The tool catalogue: the one write path into the flywheel (193, D9).
//!
//! One registry, one definition per tool, each tool's arguments named by object
//! id. The page's controls, the chat and the machinery's own commands call
//! these functions and nothing else, so no caller has an operation the others
//! lack (193). The transport is a transport: this phase serves the catalogue
//! over HTTP for the page, calls it in-process for the machinery, and serves it
//! at the host's address as a remote server of the model context protocol for
//! a member's own client (`protocol.rs`), and every one of them enumerates the
//! same list from `CATALOGUE` (193, 293; proposal, What must not be
//! foreclosed).
//!
//! The names and the argument lists are `profiles/surfaces.yaml` `tools:`
//! filtered to what phase 1 carries; a later phase adds rows here and no
//! second path anywhere.

use serde_json::{json, Value};

/// One tool of the catalogue: what it is called, the arguments it takes by
/// object id, and what it does in the operator's words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tool {
    pub name: &'static str,
    /// The arguments, named by object id (193).
    pub args: &'static [&'static str],
    pub doc: &'static str,
}

impl Tool {
    /// The tool's schema, as the in-process and HTTP callers serve it.
    pub fn schema(&self) -> Value {
        json!({"name": self.name, "args": self.args, "doc": self.doc})
    }

    /// The same tool as the model context protocol declares it to a member's
    /// client: its name, what it does, and its arguments as the input schema,
    /// each named as the schema names it (193, 293).
    pub fn declaration(&self) -> Value {
        let properties: serde_json::Map<String, Value> = self
            .args
            .iter()
            .map(|arg| (arg.to_string(), argument(arg)))
            .collect();
        let mut declared = json!({
            "name": self.name,
            "description": self.doc,
            "inputSchema": {"type": "object", "properties": properties},
        });
        // A tool whose result is one of the page's views names the view's
        // resource here, where the client reads it before it calls, and never
        // on a result (293a, S230, D18).
        if query(self.name).is_some() {
            declared["annotations"] = json!({"readOnlyHint": true});
            declared["_meta"] = json!({"ui": {"resourceUri": view_address(self.name)}});
        }
        declared
    }
}

/// The read-only tools. Each answers with one of the page's own views, read on
/// the call, and writes nothing and records nothing (193, 322, `surfaces.yaml`
/// tools.queries). A member's client that renders such resources draws the
/// view, and one that does not reads the same in words (311, S230).
pub const QUERIES: &[Tool] = &[
    Tool {
        name: "rail",
        args: &[],
        doc: "the decisions standing now, each with its number and the answers it takes; \
              `answer` answers one (15, 311, 322)",
    },
    Tool {
        name: "board",
        args: &[],
        doc: "the board: every object in its phase, grouped by what it is waiting on (209, 322, S10)",
    },
    Tool {
        name: "status",
        args: &[],
        doc: "the status view: every host and whether it is heard from, and every object \
              grouped by state, as of the last commit (141, 143, 322)",
    },
    Tool {
        name: "object",
        args: &["object"],
        doc: "one object's detail: what it is doing, the decision standing on it and its \
              answers (209, 308, 322)",
    },
];

/// The read-only tools, in the order they are served.
pub fn queries() -> &'static [Tool] {
    QUERIES
}

/// One read-only tool by name.
pub fn query(name: &str) -> Option<&'static Tool> {
    QUERIES.iter().find(|t| t.name == name)
}

/// The address of a view's resource, which carries the version of the binary
/// that serves it, so a newer binary names a newer address (326, S230).
pub fn view_address(view: &str) -> String {
    format!("ui://flywheel/{}/{view}", crate::page::VERSION)
}

/// What one argument holds, as an input schema says it: a decision is the
/// number the register gave it, the curator's moves are a list, and every other
/// argument is an object id or words (15, 193).
fn argument(name: &str) -> Value {
    match name {
        "decision" => json!({"type": "integer"}),
        "moves" => json!({"type": "array", "items": {"type": "object"}}),
        _ => json!({"type": "string"}),
    }
}

/// The one tool that answers a numbered decision. The page's controls, the
/// chat's numbered reply grammar and the machinery's own command all name this
/// and there is no second way to answer (193, 194).
pub const ANSWER: &str = "answer";

/// The catalogue. Phase 1's rows of `profiles/surfaces.yaml` `tools:`.
///
/// A tool that would assert work was done is not here and does not exist; a
/// response that arrives claiming one is recorded unapplicable and reported
/// under attention (4, 6).
pub const CATALOGUE: &[Tool] = &[
    Tool {
        name: ANSWER,
        args: &["decision", "answer", "text", "row"],
        doc: "answer a numbered decision; the deterministic path, `yes 412` or \
              `421: <text>` in chat is this tool (194); a row's letter names one \
              chore of a fold, `drop 415b`, and the answer is that chore's alone (S232)",
    },
    Tool {
        name: "capture",
        args: &["text", "source"],
        doc: "a capture with one signal of kind ask (19)",
    },
    Tool {
        name: "propose-unit",
        args: &["bolt", "text", "type", "capture"],
        doc: "a unit in approved on the bolt, with the call as its approval (34, \
              12); a bolt the name gives that does not exist is made first, on \
              the repository the instance tracks; the type defaults to chore; \
              the capture it came from is the unit's document and its text the \
              job (19); the capture's signal is routed to the unit (116)",
    },
    Tool {
        name: "open-intent",
        args: &["capture"],
        doc: "an intent in open at once, named from the capture's first words, with the \
              capture's signal attached to it: the call is its approval and the dictation \
              skips proposed (12, 19a), and the intent's material proposes its first \
              elaboration from the signal (21)",
    },
    Tool {
        name: "attach-signal",
        args: &["signal", "intent"],
        doc: "the signal's move is attach to the open intent named, and the intent proposes \
              from it (19a, 116)",
    },
    Tool {
        name: "drop-signal",
        args: &["signal"],
        doc: "the signal's move is drop, the call as its reason (19a, 116); `revive` clears \
              it (107)",
    },
    Tool {
        name: "ask",
        args: &["repository", "text"],
        doc: "an ask record for planning holding the words, by the operator or by \
              curation routing a signal that argues with no claim (28, 116); a \
              session reaches it as `flywheel ask <repository> <text>` run in its \
              place (67, 197)",
    },
    Tool {
        name: "open-session",
        args: &["repository", "text"],
        doc: "the operator's own session (69)",
    },
    Tool {
        name: "curate",
        args: &["session", "moves"],
        doc: "the curator's moves on the unmoved signals — attach, join, route, \
              challenge, drop — delivered as the charged curation session's \
              `move` deliverable with its exit, the same record `flywheel exit \
              done --deliverable move` writes; the operator running the session \
              is the operator-as-session (93b, 107, 116, 67)",
    },
    Tool {
        name: "drop",
        args: &["object"],
        doc: "undo work (4)",
    },
    Tool {
        name: "later",
        args: &["decision"],
        doc: "defer a proposal a week (172)",
    },
    Tool {
        name: "hold",
        args: &["object"],
        doc: "hold a place or a bolt's close (55)",
    },
    Tool {
        name: "release",
        args: &["object"],
        doc: "release what a hold held (55)",
    },
    Tool {
        name: "rename",
        args: &["bolt", "name"],
        doc: "a bolt's name is never its identity (29)",
    },
    Tool {
        name: "start",
        args: &["service"],
        doc: "start a service",
    },
    Tool {
        name: "stop",
        args: &["service"],
        doc: "start and stop are the pair 47 grants beyond undo-or-defer",
    },
    Tool {
        name: "finish",
        args: &["elaboration"],
        doc: "a standing session, without waiting for the idle decision (26)",
    },
    Tool {
        name: "end",
        args: &["session"],
        doc: "a with-operator session (25)",
    },
    Tool {
        name: "close",
        args: &["object"],
        doc: "a bolt or an intent whose close is offered (22, 39)",
    },
    Tool {
        name: "retire",
        args: &["object"],
        doc: "an item, planning, curation or capture session (4)",
    },
    Tool {
        name: "takeover",
        args: &["host"],
        doc: "a stale host before the 30m bound (150)",
    },
    Tool {
        name: "revive",
        args: &["signal"],
        doc: "clears the move (107)",
    },
    Tool {
        name: "take",
        args: &["line"],
        doc: "order a take now (50)",
    },
    Tool {
        name: "remove-instance",
        args: &["instance"],
        doc: "the instance's removal by response (221): sessions ended, places \
              removed, state archived, git repositories left on disk",
    },
];

/// The catalogue, in the order it is served.
pub fn catalogue() -> &'static [Tool] {
    CATALOGUE
}

/// One tool by name, or nothing where the catalogue has no such operation.
pub fn tool(name: &str) -> Option<&'static Tool> {
    CATALOGUE.iter().find(|t| t.name == name)
}

/// The enumeration every caller serves. The in-process caller reads this value,
/// the HTTP route writes it, and the protocol declares the same tools from the
/// same list, so a further transport adds a client and not an operation (193).
pub fn enumerate() -> Value {
    json!({
        "tools": CATALOGUE.iter().map(Tool::schema).collect::<Vec<_>>(),
        "queries": QUERIES.iter().map(Tool::schema).collect::<Vec<_>>(),
    })
}

// ---------------------------------------------------------------- the bodies

use anyhow::{bail, Result};
use flywheel_atoms::{Ask, Received, Scope, StateStore, World};
use flywheel_domain::asks;
use flywheel_domain::commands::{self, CallRecord, Called};
use flywheel_domain::signals;
use flywheel_engine::Definitions;
use std::collections::BTreeMap;

/// One invocation, whoever made it: a page control, a chat reply, the
/// machinery's own command. The arguments are named as the tool's schema names
/// them, by object id (193).
#[derive(Debug, Clone)]
pub struct Call {
    pub tool: String,
    pub args: BTreeMap<String, Value>,
    /// The identity every response records as given by (153, 236a, D10).
    pub by: String,
    /// Where the call came from: `page`, `chat`, `machinery`.
    pub delivery: String,
    /// The delivery's own id where the caller has one, so the same delivery
    /// twice is one record (137).
    pub delivery_id: Option<String>,
    /// The source event this call captures, where it captures one: an adapter
    /// keys its capture by the event, so the same event twice yields one
    /// capture (111). It is not the delivery's id — one is the message that
    /// arrived, the other the thing it is about.
    pub event_key: Option<String>,
    /// The host's agent that proposed the call; nothing when a control was
    /// used (194).
    pub proposed_by: Option<String>,
}

impl Call {
    /// A call with its arguments given as pairs, from the machinery or a test.
    pub fn new(tool: &str, by: &str, delivery: &str) -> Call {
        Call {
            tool: tool.to_string(),
            args: BTreeMap::new(),
            by: by.to_string(),
            delivery: delivery.to_string(),
            delivery_id: None,
            event_key: None,
            proposed_by: None,
        }
    }

    pub fn arg(mut self, name: &str, value: Value) -> Call {
        self.args.insert(name.to_string(), value);
        self
    }

    /// The delivery's own id, so a repeat is recognised (137).
    pub fn delivered(mut self, id: &str) -> Call {
        self.delivery_id = Some(id.to_string());
        self
    }

    /// The source event this call captures, so capturing it twice yields one
    /// capture (111).
    pub fn keyed(mut self, event: &str) -> Call {
        self.event_key = Some(event.to_string());
        self
    }

    fn text(&self, name: &str) -> Option<String> {
        self.args.get(name).and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            Value::Null => None,
            other => Some(other.to_string()),
        })
    }

    /// A number an argument carries, however the caller sent it. The page's own
    /// controls are plain forms so that a control works with nothing fetched
    /// and no script running (310, 311), and a form sends every field as text;
    /// a client sending JSON sends a number. One tool serves both (193).
    fn number(&self, name: &str) -> Option<u64> {
        match self.args.get(name)? {
            Value::Number(n) => n.as_u64(),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        }
    }
}

/// What a call did. `Recorded` carries the response's id; the machinery's next
/// tick applies it.
pub type Outcome = Called;

/// The operations of a line a session may never take: it commits inside its
/// place only, and never merges, lands, or holds and releases a place (43, 55).
pub const LINE_OPERATIONS: &[&str] = &["take", "close", "hold", "release"];

/// The session a call comes from, where it comes from one: a session's own
/// command names itself as the caller, and its identity is a session id.
fn session_caller(call: &Call) -> Option<String> {
    (call.delivery == "session" || call.by.starts_with("session/")).then(|| call.by.clone())
}

/// Invoke one tool of the catalogue, writing through the state store (193).
///
/// Every caller ends here. A tool the catalogue lacks is not performed and is
/// not silently dropped either: the call is recorded as the response it was,
/// and the response machine reports it unapplicable under attention (4, 6).
pub fn call<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    call: &Call,
) -> Result<Outcome> {
    // A session that attempts a line operation is refused, and the refusal is
    // on its own thread for `record_refusals` to carry to the run record and
    // attention on the next tick (43, 79, 81).
    if let Some(session) = session_caller(call) {
        if LINE_OPERATIONS.contains(&call.tool.as_str()) {
            let reason = format!(
                "a session commits inside its place only and never creates a line of work, \
                 merges or lands: `{}` is refused (43)",
                call.tool
            );
            refuse(store, &session, call, first_object_argument(call).as_deref(), &reason)?;
            bail!("{reason}");
        }
        // The ask is the curation session's and the operator's own session's,
        // and any other session is refused it as a line operation is (43, 69,
        // 197, `sessions.yaml` commands.ask).
        if call.tool == "ask" && !asks::granted(&session) {
            let reason = format!(
                "only the curation session and the operator's own session file an ask: \
                 `ask` is refused to {session} (43, 69, 197)"
            );
            refuse(store, &session, call, call.text("repository").as_deref(), &reason)?;
            bail!("{reason}");
        }
    }

    let Some(tool) = tool(&call.tool) else {
        // No such operation exists. Record it and let the response machine say
        // so (4, 6).
        let object = first_object_argument(call);
        return commands::record_call(
            store,
            defs,
            &CallRecord {
                tool: &call.tool,
                decision: None,
                object: object.as_deref(),
                answer: &call.tool,
                args: Some(Value::Object(call.args.clone().into_iter().collect())),
                by: &call.by,
                delivery: &call.delivery,
                delivery_id: call.delivery_id.as_deref(),
                proposed_by: call.proposed_by.as_deref(),
            },
        );
    };

    match tool.name {
        ANSWER => answer(store, defs, call),
        "ask" => ask(store, world, defs, call),
        "capture" => capture(store, world, defs, call),
        "later" => later(store, defs, call),
        "open-session" => open_session(store, defs, call),
        "propose-unit" => propose_unit(store, world, defs, call),
        "open-intent" => open_intent(store, world, defs, call),
        "attach-signal" => attach_signal(store, world, defs, call),
        "drop-signal" => drop_signal(store, world, defs, call),
        "revive" => revive(store, world, defs, call),
        "curate" => curate(store, world, defs, call),
        // Every other tool takes the transition its decision would, on the
        // object its first argument names (4, 12).
        _ => {
            let name = tool.args[0];
            let Some(object) = call.text(name) else {
                bail!("`{}` takes {name}, and the call names none", tool.name);
            };
            dictate(store, defs, call, tool, &object)
        }
    }
}

/// A query of the catalogue: the view it names, read from the store on this call
/// and stored nowhere, with nothing recorded (15, 193, 310, 322).
pub fn view<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &W,
    defs: &Definitions,
    address: &str,
    call: &Call,
) -> Result<crate::page::View> {
    let Some(tool) = query(&call.tool) else {
        bail!("`{}` is no view of the page", call.tool);
    };
    let read = crate::page::read(store, world, defs, address, &call.by)?;
    crate::page::view(&read, tool.name, call.text("object").as_deref()).map_err(|refused| anyhow::anyhow!(refused))
}

/// A refusal of a session's call, on the session's own thread: the operation,
/// what it named, and why. `record_refusals` carries it to the run record and
/// to attention on the next tick (43, 79, 81).
fn refuse<S: StateStore>(
    store: &mut S,
    session: &str,
    call: &Call,
    object: Option<&str>,
    reason: &str,
) -> Result<()> {
    let at = commands::now(store)?;
    let mut fields: BTreeMap<String, Value> = BTreeMap::new();
    fields.insert("operation".into(), json!(call.tool));
    if let Some(object) = object.filter(|o| !o.is_empty()) {
        fields.insert("object".into(), json!(object));
    }
    fields.insert("reason".into(), json!(reason));
    store.append(
        session,
        &flywheel_atoms::ThreadEntry {
            at,
            kind: "refusal".into(),
            by: Some(session.to_string()),
            fields,
        },
    )
}

/// An ask for planning (28, 116): the words and the repository they name,
/// written as the dictation's effect through the store's commit path, with
/// the call recorded once as the response it was (153, 193).
///
/// The operator's dictation and a session's own command are the one call. A
/// session's is recorded `by` the session and writes nothing on its thread;
/// what refuses it is an entry there, like every refusal of a session's call
/// (67, 197, `sessions.yaml` commands.ask). The same delivery twice is one ask
/// (137). The ask's name is in the journal, for the caller that prints it and
/// the route that names it (`asked`).
fn ask<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    call: &Call,
) -> Result<Outcome> {
    let session = session_caller(call);
    let repository = call.text("repository").map(|r| r.trim().to_string()).unwrap_or_default();
    let text = call.text("text").map(|t| t.trim().to_string()).unwrap_or_default();
    if let Some(reason) = ask_refused(&built_repositories(world)?, &repository, &text) {
        if let Some(session) = &session {
            refuse(store, session, call, Some(&repository), &reason)?;
        }
        bail!("{reason}");
    }
    let by = match &session {
        Some(session) => asks::by_session(session),
        None => call.by.clone(),
    };
    let at = commands::now(store)?;
    let id = asks::next_id(store, &repository)?;
    let mut record = commands::record_call(
        store,
        defs,
        &CallRecord {
            tool: "ask",
            decision: None,
            object: Some(&asks::name_of(&id)),
            answer: &text,
            args: Some(Value::Object(call.args.clone().into_iter().collect())),
            by: &by,
            delivery: &call.delivery,
            delivery_id: call.delivery_id.as_deref(),
            proposed_by: call.proposed_by.as_deref(),
        },
    )?;
    let name = match record.outcome {
        // The delivery was recorded before: the ask it wrote then is the one,
        // and nothing is written again (137).
        Received::AlreadyApplied { .. } => store
            .get(&format!("response/{}", record.id))?
            .and_then(|o| o.record.get("object").and_then(|v| v.as_str()).map(String::from))
            .unwrap_or_else(|| asks::name_of(&id)),
        _ => {
            store.put_ask(&Ask {
                id: id.clone(),
                repository: repository.clone(),
                text: text.clone(),
                by,
                at,
                consumed_by: None,
            })?;
            asks::name_of(&id)
        }
    };
    record
        .journal
        .push(noted("ask", &name, format!("in {repository}: {text}")));
    Ok(record)
}

/// Why an ask would be refused, where it would be: it names a repository the
/// instance tracks, with the tracked ones named when it does not, and it gives
/// words (28, 205, 206).
fn ask_refused(tracked: &[String], repository: &str, text: &str) -> Option<String> {
    if !tracked.iter().any(|t| t == repository) {
        return Some(match (tracked, repository.is_empty()) {
            ([], _) => "this instance tracks no repository to ask in (205, 206)".to_string(),
            (_, true) => format!(
                "an ask names the repository it is for: this instance tracks {} (205, 206)",
                tracked.join(", ")
            ),
            (_, false) => format!(
                "`{repository}` is no repository this instance tracks; it tracks {} (205, 206)",
                tracked.join(", ")
            ),
        });
    }
    if text.is_empty() {
        return Some("an ask holds the words to ask for, and this one gives none (28)".to_string());
    }
    None
}

/// The ask a call filed, by the name a route move gives it: `ask/<id>`.
pub fn asked(outcome: &Outcome) -> Option<String> {
    outcome
        .journal
        .iter()
        .find(|noted| noted.kind == "ask")
        .map(|noted| noted.object.clone())
}

/// The object a call names, for a tool the catalogue does not carry: whichever
/// argument reads like an object id.
fn first_object_argument(call: &Call) -> Option<String> {
    for name in ["object", "item", "session", "unit", "elaboration", "bolt", "line", "place"] {
        if let Some(value) = call.text(name) {
            return Some(value);
        }
    }
    call.args.values().find_map(|v| match v {
        Value::String(s) if s.contains('/') => Some(s.clone()),
        _ => None,
    })
}

/// Answer a numbered decision: the deterministic path, and the same tool the
/// chat's numbered reply grammar calls (129, 153, 194).
fn answer<S: StateStore>(store: &mut S, defs: &Definitions, call: &Call) -> Result<Outcome> {
    let Some(number) = call.number("decision") else {
        bail!("`answer` takes the decision's number, and the call names none");
    };
    // An answer whose pattern takes an argument arrives as two fields, because
    // a control on the page is a plain form and a form posts its fields: the
    // pattern's head, and the text the operator typed (S6, 311). What is
    // recorded is the one string the machine's pattern matches — `redo: the
    // rows lose their numbers`, `bolt atlas/plan-rows` — so the chat's reply
    // grammar and the page's control still write the same answer (193, 194).
    let answer = match (call.text("answer"), call.text("text")) {
        (Some(pattern), Some(text)) if pattern.contains('<') && !text.trim().is_empty() => {
            filled(&pattern, text.trim())
        }
        (Some(said), _) => said,
        (None, Some(text)) => text,
        (None, None) => String::new(),
    };
    // A row of a fold of chores is its chore's own decision: named by the
    // fold's number and the row's letter, the answer is recorded on that chore
    // and applies to it alone (S232, 11).
    let row = call.text("row").map(|row| row.trim().to_ascii_lowercase()).filter(|row| !row.is_empty());
    let object = match &row {
        Some(letter) => Some(row_object(store, defs, number as u32, letter)?),
        None => None,
    };
    let mut record = commands::record_call(
        store,
        defs,
        &CallRecord {
            tool: ANSWER,
            decision: Some(number as u32),
            object: object.as_deref(),
            answer: &answer,
            args: Some(Value::Object(call.args.clone().into_iter().collect())),
            by: &call.by,
            delivery: &call.delivery,
            delivery_id: call.delivery_id.as_deref(),
            proposed_by: call.proposed_by.as_deref(),
        },
    )?;
    record.journal.push(noted(
        "response",
        &record.id,
        format!("{number}{} → {answer}", row.as_deref().unwrap_or_default()),
    ));
    Ok(record)
}

/// The chore a row of a standing fold names, by the fold's number and the
/// row's letter; refused with the rows it has where the letter names none that
/// stands (S232).
fn row_object<S: StateStore>(store: &mut S, defs: &Definitions, number: u32, letter: &str) -> Result<String> {
    let standing = commands::rail(store, defs)?;
    let Some(decision) = standing.iter().find(|d| d.number == Some(number)) else {
        bail!("{number} is not on the rail, so it has no row {letter}");
    };
    let objects = flywheel_atoms::Records::list_records(store, &flywheel_atoms::Scope::All)?;
    let Some(rows) = crate::page::rows_of(&objects, decision) else {
        bail!("{number} is not a fold of chores; a letter names a row of one (S232)");
    };
    let standing: Vec<_> = rows.iter().filter(|row| row.standing).collect();
    match standing.iter().find(|row| row.letter == letter) {
        Some(row) => Ok(row.chore.id.clone()),
        None => bail!(
            "{number} has no row {letter} standing; its rows are {}",
            standing.iter().map(|row| format!("{number}{}", row.letter)).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// One answer's pattern with the operator's text in place of its placeholder.
///
/// `redo: <notes>` and "the rows lose their numbers" make
/// `redo: the rows lose their numbers`; `bolt <name>` and "atlas/plan-rows"
/// make `bolt atlas/plan-rows`; `<intent>: drop` and "atlas-provider-limits"
/// make `atlas-provider-limits: drop`. Each is the one string the machine's own
/// pattern matches, so the page's control and the chat's reply grammar record
/// the same answer (193, 194, `eval::match_answer`).
fn filled(pattern: &str, text: &str) -> String {
    match (pattern.find('<'), pattern.find('>')) {
        (Some(open), Some(close)) if close > open => {
            format!("{}{text}{}", &pattern[..open], &pattern[close + 1..])
        }
        _ => format!("{pattern} {text}"),
    }
}

/// `filled`, for the tier that holds it.
#[cfg(test)]
pub fn filled_for_tests(pattern: &str, text: &str) -> String {
    filled(pattern, text)
}

/// Defer the proposal a decision stands on, a week (172). The argument is the
/// decision's number; the object it acts on is that decision's own.
fn later<S: StateStore>(store: &mut S, defs: &Definitions, call: &Call) -> Result<Outcome> {
    let Some(number) = call.number("decision") else {
        bail!("`later` takes the decision's number, and the call names none");
    };
    let register = commands::register(store)?;
    let Some(decision) = register.decision_of(number as u32).map(String::from) else {
        bail!("`later` names decision {number}, which the register never gave");
    };
    // A decision's id is `<object>/<kind>`: the object is what defers.
    let object = decision
        .rsplit_once('/')
        .map(|(object, _)| object.to_string())
        .unwrap_or(decision);
    let tool = tool("later").expect("`later` is in the catalogue");
    dictate(store, defs, call, tool, &object)
}

/// A dictation: applied directly, never proposed, taking the same transition
/// the decision would have taken (4, 12).
fn dictate<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    call: &Call,
    tool: &Tool,
    object: &str,
) -> Result<Outcome> {
    let mut record = commands::record_call(
        store,
        defs,
        &CallRecord {
            tool: tool.name,
            decision: None,
            object: Some(object),
            answer: tool.name,
            args: Some(Value::Object(call.args.clone().into_iter().collect())),
            by: &call.by,
            delivery: &call.delivery,
            delivery_id: call.delivery_id.as_deref(),
            proposed_by: call.proposed_by.as_deref(),
        },
    )?;
    record
        .journal
        .push(noted("dictation", object, tool.name.to_string()));
    Ok(record)
}


/// The curator's moves, as the charged curation session's delivery (93b, 107,
/// 116). Each move is written into the blueprints under the machinery's prefix,
/// and the session reports its exit with `move` as what it delivered — the
/// same entry `flywheel exit done --deliverable move` writes, so what a person
/// submits on the page and what a session's command writes are one record
/// (67, D8). `record_moves` on the next tick applies them, unchanged (110).
///
/// The page's form and a client's JSON name the moves differently and are one
/// tool: `moves` as a list of `{signal, move, target}`, or one `move.<signal>`
/// field per signal with an optional `target.<signal>` beside it. A signal the
/// operator left alone stays unmoved: nothing is judged by omission (107, 118).
/// The session is the one charged where the call names none.
///
/// A route names what was offered for a signal that argues with no claim, and
/// on this surface that is an ask: `ask.<signal>` gives the words and
/// `repository.<signal>` the repository (or a listed move's `ask` and
/// `repository`), and the `ask` tool files it before the route names it — the
/// same call a curation session makes through `flywheel ask` (116, 67,
/// `sessions.yaml` commands.ask). Every ask is checked before anything is
/// written, so a refused one leaves no move and no exit behind.
fn curate<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    call: &Call,
) -> Result<Outcome> {
    let at = commands::now(store)?;
    let session = match call.text("session").filter(|s| !s.trim().is_empty()) {
        Some(session) => session,
        None => {
            let objects = store.list_records(&Scope::All)?;
            crate::page::curating(&objects).ok_or_else(|| {
                anyhow::anyhow!(
                    "no curation session is charged; the tick charges one when the unmoved \
                     signals cross the threshold or the cadence says so (110)"
                )
            })?
        }
    };
    let reason = format!("the operator curated it on the {}", call.delivery);
    // Each move, with the ask a route files where it files one: the repository
    // and the words.
    let mut moves: Vec<(signals::Move, Option<(String, String)>)> = Vec::new();
    let mut push = |signal: &str, word: &str, names: &str, asking: Option<(String, String)>| {
        let word = word.trim();
        if signal.is_empty() || word.is_empty() {
            return;
        }
        let names = names.trim();
        let asking = asking
            .filter(|(_, words)| word == "route" && !words.trim().is_empty())
            .map(|(repository, words)| (repository.trim().to_string(), words.trim().to_string()));
        moves.push((
            signals::Move {
                signal: signal.to_string(),
                target: match names.is_empty() {
                    true => word.to_string(),
                    false => format!("{word} {names}"),
                },
                reason: reason.clone(),
                at: at.to_rfc3339(),
            },
            asking,
        ));
    };
    if let Some(Value::Array(listed)) = call.args.get("moves") {
        for item in listed {
            let text = |name: &str| item.get(name).and_then(Value::as_str).unwrap_or_default();
            let signal = text("signal");
            let asking = Some((text("repository").to_string(), text("ask").to_string()));
            // `move` is the word and `target` what it names; a `target` alone
            // carries both, as the record does (`attach <intent>`).
            match text("move") {
                "" => {
                    let mut parts = text("target").splitn(2, char::is_whitespace);
                    let word = parts.next().unwrap_or_default().to_string();
                    let names = parts.next().unwrap_or_default().to_string();
                    push(signal, &word, &names, asking);
                }
                word => push(signal, word, text("target"), asking),
            }
        }
    }
    for (name, value) in &call.args {
        let Some(signal) = name.strip_prefix("move.") else {
            continue;
        };
        let word = value.as_str().unwrap_or_default();
        let names = call.text(&format!("target.{signal}")).unwrap_or_default();
        let asking = Some((
            call.text(&format!("repository.{signal}")).unwrap_or_default(),
            call.text(&format!("ask.{signal}")).unwrap_or_default(),
        ));
        push(signal, word, &names, asking);
    }
    // A route names what was offered for its signal (116). Where it offers an
    // ask, the ask is checked here with every other, before anything is
    // written; the only repository the instance tracks is the one meant when
    // none is picked.
    let tracked = built_repositories(world)?;
    for (moved, asking) in &mut moves {
        match asking {
            Some((repository, words)) => {
                if let ([one], true) = (tracked.as_slice(), repository.is_empty()) {
                    *repository = one.clone();
                }
                if let Some(refused) = ask_refused(&tracked, repository, words) {
                    bail!("the ask routing {} was refused: {refused}", moved.signal);
                }
            }
            None if moved.word() == "route" && moved.names().is_empty() => bail!(
                "a route names what was offered for {}: give the words to ask for (116)",
                moved.signal
            ),
            None => {}
        }
    }
    let mut journal = Vec::new();
    for (mut moved, asking) in moves {
        if let Some((repository, words)) = asking {
            let mut asked_for = Call::new("ask", &call.by, &call.delivery)
                .arg("repository", json!(repository))
                .arg("text", json!(words));
            asked_for.proposed_by = call.proposed_by.clone();
            // Through the catalogue's door, so a caller refused the ask is
            // refused it here too (43, 197).
            let filed = self::call(store, world, defs, &asked_for)?;
            let Some(name) = asked(&filed) else {
                bail!("the ask routing {} was recorded and named nothing", moved.signal);
            };
            moved.target = format!("route {name}");
            journal.extend(filed.journal);
        }
        signals::write_move(world, &moved)?;
        journal.push(noted("move", &moved.signal, moved.target.clone()));
    }
    flywheel_domain::report::write_report(
        store,
        &session,
        &call.by,
        at,
        &flywheel_domain::report::Report::Exit {
            kind: "done".into(),
            deliverables: vec!["move".into()],
            question: None,
            text: None,
        },
    )?;
    let moved = journal.iter().filter(|noted| noted.kind == "move").count();
    journal.push(noted("exit", &session, format!("done: {moved} move(s)")));
    Ok(Called {
        id: session.clone(),
        outcome: Received::Recorded { id: session },
        journal,
    })
}

/// The operator's own session (69): opened by dictation at any time, on no
/// thread, with no intent behind it. The machinery opens nothing here that the
/// operator did not ask for, and raises no decision about it (12, 69).
fn open_session<S: StateStore>(store: &mut S, defs: &Definitions, call: &Call) -> Result<Outcome> {
    let at = commands::now(store)?;
    let ordinal = next_of(store, "operator-session", &format!("operator-session/{}-", call.by))?;
    let id = format!("operator-session/{}-{ordinal}", call.by);
    let repository = call
        .text("repository")
        .filter(|r| !r.is_empty())
        .unwrap_or_else(|| "blueprints".to_string());
    let record: BTreeMap<String, Value> = [
        ("opened_by", json!(call.by)),
        ("opened_at", json!(at.to_rfc3339())),
        ("repository", json!(repository)),
        // The type it runs, which its machine fixes: a with-operator session on
        // no thread (69, `operator-session.yaml`). Its sessions are named under
        // that, as every object's are named under the type it runs.
        ("type", json!("with-operator")),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    commands::put_new(store, defs, &id, "operator-session", None, record, at)?;
    let mut record = commands::record_call(
        store,
        defs,
        &CallRecord {
            tool: "open-session",
            decision: None,
            object: Some(&id),
            answer: &call.text("text").unwrap_or_default(),
            args: Some(Value::Object(call.args.clone().into_iter().collect())),
            by: &call.by,
            delivery: &call.delivery,
            delivery_id: call.delivery_id.as_deref(),
            proposed_by: call.proposed_by.as_deref(),
        },
    )?;
    record
        .journal
        .push(noted("open-session", &id, format!("opened by {}", call.by)));
    Ok(record)
}

/// A unit in `approved` on a bolt, the call as its approval — the operator's
/// dictation naming a bolt, applied directly (34, 12, `surfaces.yaml`
/// propose-unit, model.md §5). No decision is raised: the operator gave one.
///
/// The bolt is `bolt/<repository>/<name>`, given whole or as a bare name on the
/// repository the call names or the one the instance tracks; a bolt that does
/// not exist yet is made here, which is 34's "dictation naming a bolt" and
/// `bolt.yaml`'s "or by dictation". The bolt and the items are made in this
/// call rather than left to the machine's `proposed → approved` transition,
/// because that transition is a decision's and this is not one; their proofs
/// (`unit.bolt_exists`, `unit.items_exist`) hold at once, so the tick has
/// nothing to repeat (73, 127). The type defaults to `chore`, the one stage,
/// one session type (60); the capture the call came from is the unit's
/// `document`, and its text is the job the session is handed (19, 89).
fn propose_unit<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    call: &Call,
) -> Result<Outcome> {
    let at = commands::now(store)?;
    let capture = call.text("capture").map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
    // Built from a capture with no bolt named, the bolt is named from the
    // capture's first words: the operator types no name (S217).
    let (named, from_words) = match call.text("bolt").map(|b| b.trim().to_string()).filter(|b| !b.is_empty()) {
        Some(named) => (named, false),
        None => match capture.as_deref().and_then(|c| words_of(store, c)) {
            Some(words) => (signals::name_from_words(&words), true),
            None => bail!("`propose-unit` takes the bolt's name, and the call names none"),
        },
    };
    let (repository, name) = match named.strip_prefix("bolt/").and_then(|rest| rest.split_once('/')) {
        Some((repository, name)) => (repository.to_string(), slug(name)),
        None => {
            let repository = match call.text("repository").filter(|r| !r.trim().is_empty()) {
                Some(repository) => repository.trim().to_string(),
                None => {
                    let tracked = built_repositories(world)?;
                    match tracked.as_slice() {
                        [one] => one.clone(),
                        [] => bail!(
                            "`propose-unit`: the instance tracks no repository for a bolt to land on (205, 206)"
                        ),
                        many => bail!(
                            "`propose-unit`: the instance tracks {} repositories, so the call names one: {}",
                            many.len(),
                            many.join(", ")
                        ),
                    }
                }
            };
            (repository, slug(&named))
        }
    };
    if name.is_empty() {
        bail!("`propose-unit`: `{named}` leaves no name for the bolt");
    }
    let kind = call
        .text("type")
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "chore".to_string());
    let Some(machine) = defs.get(&kind).filter(|m| m.regions.contains_key("stages")) else {
        bail!("`propose-unit`: `{kind}` is no unit type this set carries (37, 57)");
    };
    // A name taken from the words takes the next number where a unit or a bolt
    // holds it already; a name the operator gave is given once (I1).
    let name = match from_words {
        true => {
            let taken = |name: &str| -> Result<bool> {
                Ok(store.get(&format!("unit/{repository}/{name}"))?.is_some()
                    || store.get(&format!("bolt/{repository}/{name}"))?.is_some())
            };
            let mut free = name.clone();
            let mut nth = 1;
            while taken(&free)? {
                nth += 1;
                free = format!("{name}-{nth}");
            }
            free
        }
        false => name,
    };
    let unit = format!("unit/{repository}/{name}");
    if store.get(&unit)?.is_some() {
        bail!("`{unit}` already exists; a name is given once (I1)");
    }
    // The capture's signals are routed to the unit, as curation's route would
    // have: a signal already moved keeps its move, and a capture none of whose
    // signals is left unmoved is refused before anything is made (107, 116).
    let routed = match &capture {
        Some(capture) => {
            let held = signals_named(store, world, capture)?;
            match held.is_empty() {
                true => vec![],
                false => unmoved_of(capture, held)?,
            }
        }
        None => vec![],
    };
    let subject = call
        .text("text")
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .or_else(|| capture.as_deref().and_then(|c| words_of(store, c)));
    let mut record: BTreeMap<String, Value> = BTreeMap::new();
    record.insert("repository".into(), json!(repository));
    record.insert("type".into(), json!(kind));
    record.insert("type_version".into(), json!(machine.version));
    record.insert("target".into(), json!({"new_name": name}));
    record.insert("items".into(), json!(1));
    record.insert("proposed_by".into(), json!(call.by));
    if let Some(capture) = &capture {
        record.insert("document".into(), json!(capture));
    }
    if let Some(subject) = &subject {
        record.insert("subject".into(), json!(subject));
    }
    commands::put_new(store, defs, &unit, "unit", None, record, at)?;
    let mut record = commands::record_call(
        store,
        defs,
        &CallRecord {
            tool: "propose-unit",
            decision: None,
            object: Some(&unit),
            answer: subject.as_deref().unwrap_or(""),
            args: Some(Value::Object(call.args.clone().into_iter().collect())),
            by: &call.by,
            delivery: &call.delivery,
            delivery_id: call.delivery_id.as_deref(),
            proposed_by: call.proposed_by.as_deref(),
        },
    )?;
    // Approved, by this call: the record names the response that approved it,
    // which is what I1 asks of every unit (unit.yaml record.approval).
    let (bolt, items) = approve_unit(store, defs, &unit, &record.id, &name, &repository, at)?;
    record.journal.push(noted(
        "propose-unit",
        &unit,
        format!("approved on {bolt} by {}, {items} item(s)", call.by),
    ));
    let reason = format!("{} built {unit} from it on the {} ({})", call.by, call.delivery, record.id);
    let moved = move_signals(store, world, &routed, &format!("route {unit}"), &reason, at)?;
    record.journal.extend(moved);
    Ok(record)
}

// ------------------------------------------------ the operator's hand on a signal

/// The signals a capture's control moves, each with the move standing on it
/// where one stands: the one signal a signal's id names, or every signal of the
/// capture a capture's id names, which for a capture no reader has read yet is
/// none (19a, 107, 115).
fn signals_named<S: StateStore, W: World + ?Sized>(
    store: &S,
    world: &W,
    named: &str,
) -> Result<Vec<(String, Option<signals::Move>)>> {
    let ids = match named.starts_with(signals::PREFIX) {
        true => match store.get(named)? {
            Some(_) => vec![named.to_string()],
            None => bail!("`{named}` is no signal on record"),
        },
        false => signals::of_capture(store, named)?,
    };
    ids.into_iter()
        .map(|id| {
            let standing = signals::standing_move(world, &id)?;
            Ok((id, standing))
        })
        .collect()
}

/// The signals still unmoved, or a refusal naming the moves that stand: a signal
/// takes one move, and a control on a capture moves only what nothing has moved
/// yet (107, 19a).
fn unmoved_of(named: &str, held: Vec<(String, Option<signals::Move>)>) -> Result<Vec<String>> {
    if held.is_empty() {
        bail!("`{named}` holds no signal yet: its reader has not read it (115)");
    }
    let unmoved: Vec<String> = held.iter().filter(|(_, moved)| moved.is_none()).map(|(id, _)| id.clone()).collect();
    if unmoved.is_empty() {
        let standing: Vec<String> = held
            .iter()
            .filter_map(|(id, moved)| moved.as_ref().map(|moved| format!("{id} is {}", moved.target)))
            .collect();
        bail!(
            "`{named}` has no signal left to move — {}; a signal takes one move, and `revive` clears one (107)",
            standing.join(", ")
        );
    }
    Ok(unmoved)
}

/// Write one move on each signal, with its stated consequence (107, 116).
fn move_signals<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    moving: &[String],
    target: &str,
    reason: &str,
    at: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<commands::Noted>> {
    let mut journal = Vec::new();
    for signal in moving {
        let moved = signals::Move {
            signal: signal.clone(),
            target: target.to_string(),
            reason: reason.to_string(),
            at: at.to_rfc3339(),
        };
        signals::apply_move(store, world, &moved, at)?;
        journal.push(noted("move", signal, target.to_string()));
    }
    Ok(journal)
}

/// A call whose delivery was recorded before: acknowledged and changing
/// nothing, rather than refused for what its first delivery did (137).
fn delivered_before<S: StateStore>(store: &S, call: &Call) -> Result<bool> {
    Ok(match &call.delivery_id {
        Some(id) => store.get(&format!("response/{id}"))?.is_some(),
        None => false,
    })
}

/// Record a call of one of the operator's hands on a signal, once (153, 193).
fn record_hand<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    call: &Call,
    object: &str,
    answer: &str,
) -> Result<Outcome> {
    commands::record_call(
        store,
        defs,
        &CallRecord {
            tool: &call.tool,
            decision: None,
            object: Some(object),
            answer,
            args: Some(Value::Object(call.args.clone().into_iter().collect())),
            by: &call.by,
            delivery: &call.delivery,
            delivery_id: call.delivery_id.as_deref(),
            proposed_by: call.proposed_by.as_deref(),
        },
    )
}

/// An argument the call must name.
fn named_argument(call: &Call, name: &str) -> Result<String> {
    match call.text(name).map(|v| v.trim().to_string()).filter(|v| !v.is_empty()) {
        Some(value) => Ok(value),
        None => bail!("`{}` takes the {name}, and the call names none", call.tool),
    }
}

/// `open-intent`: the operator makes an intent of a capture (12, 19a, 21,
/// `intent.yaml`). The intent is named from the capture's first words and opens
/// at once, the call as its approval, since the operator's own dictation skips
/// the proposal curation's joins stand in; the capture's signals are attached to
/// it, and its material region proposes the first elaboration from them on the
/// next tick.
fn open_intent<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    call: &Call,
) -> Result<Outcome> {
    let capture = named_argument(call, "capture")?;
    if delivered_before(store, call)? {
        return record_hand(store, defs, call, &capture, "");
    }
    let attaching = unmoved_of(&capture, signals_named(store, world, &capture)?)?;
    let words = attaching
        .iter()
        .find_map(|signal| store.get(signal).ok().flatten().as_ref().and_then(signals::text_of))
        .unwrap_or_default();
    let intent = unused_id(store, &format!("intent/{}", signals::name_from_words(&words)))?;
    let at = commands::now(store)?;
    let record: BTreeMap<String, Value> =
        [("subject".to_string(), json!(words)), ("opened_by".to_string(), json!(call.by))].into_iter().collect();
    commands::put_new(store, defs, &intent, "intent", None, record, at)?;
    let mut record = record_hand(store, defs, call, &intent, &words)?;
    // Open, by this call: the record names the response that opened it (I1).
    let mut held = store.get(&intent)?.expect("the intent was just written");
    let base = held.seq;
    held.record.insert("approval".into(), json!(record.id));
    held.config.insert("life".into(), "open".into());
    held.entered_at.insert("life".into(), at);
    flywheel_engine::initialise(defs, &mut held, at);
    store.put(&intent, &held, base)?;
    record
        .journal
        .push(noted("open-intent", &intent, format!("opened by {} from {capture}", call.by)));
    let reason = format!("{} opened {intent} from it on the {} ({})", call.by, call.delivery, record.id);
    let moved = move_signals(store, world, &attaching, &format!("attach {intent}"), &reason, at)?;
    record.journal.extend(moved);
    Ok(record)
}

/// `attach-signal`: the signal is evidence on an open intent the operator
/// picked, and the intent proposes from it (19a, 21, 116).
fn attach_signal<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    call: &Call,
) -> Result<Outcome> {
    let signal = named_argument(call, "signal")?;
    let intent = named_argument(call, "intent")?;
    if delivered_before(store, call)? {
        return record_hand(store, defs, call, &signal, "");
    }
    let attaching = unmoved_of(&signal, signals_named(store, world, &signal)?)?;
    let open = store
        .get(&intent)?
        .is_some_and(|held| held.machine == "intent" && held.config.get("life").map(String::as_str) == Some("open"));
    if !open {
        let open: Vec<String> = store
            .list_records(&Scope::All)?
            .into_iter()
            .filter(|o| o.machine == "intent" && o.config.get("life").map(String::as_str) == Some("open"))
            .map(|o| o.id)
            .collect();
        bail!(
            "`{intent}` is no open intent; a signal attaches to one that is open: {}",
            match open.is_empty() {
                true => "none is".to_string(),
                false => open.join(", "),
            }
        );
    }
    let target = format!("attach {intent}");
    let at = commands::now(store)?;
    let mut record = record_hand(store, defs, call, &signal, &target)?;
    let reason = format!("{} attached it on the {} ({})", call.by, call.delivery, record.id);
    let moved = move_signals(store, world, &attaching, &target, &reason, at)?;
    record.journal.extend(moved);
    Ok(record)
}

/// `drop-signal`: the signal's move is drop, the call its reason; `revive`
/// clears it (19a, 107, 116).
fn drop_signal<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    call: &Call,
) -> Result<Outcome> {
    let signal = named_argument(call, "signal")?;
    if delivered_before(store, call)? {
        return record_hand(store, defs, call, &signal, "");
    }
    let dropping = unmoved_of(&signal, signals_named(store, world, &signal)?)?;
    let at = commands::now(store)?;
    let mut record = record_hand(store, defs, call, &signal, "drop")?;
    let reason = format!("{} dropped it on the {} ({})", call.by, call.delivery, record.id);
    let moved = move_signals(store, world, &dropping, "drop", &reason, at)?;
    record.journal.extend(moved);
    Ok(record)
}

/// `revive`: the signal's move is cleared and it is unmoved again, for the next
/// curation run to judge; no decision is raised for it (107, 12, S24).
fn revive<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    call: &Call,
) -> Result<Outcome> {
    let signal = named_argument(call, "signal")?;
    let mut record = record_hand(store, defs, call, &signal, "revive")?;
    if matches!(record.outcome, Received::AlreadyApplied { .. }) {
        return Ok(record);
    }
    signals::clear_move(world, &signal)?;
    if let Some(mut held) = store.get(&signal)? {
        let base = held.seq;
        if held.record.remove("move").is_some() {
            store.put(&signal, &held, base)?;
        }
    }
    record.journal.push(noted("dictation", &signal, "revive"));
    Ok(record)
}

/// An id no object holds: the one given, or the next numbered after it, since a
/// name is given once (I1).
fn unused_id<S: StateStore>(store: &S, stem: &str) -> Result<String> {
    let mut id = stem.to_string();
    let mut nth = 1;
    while store.get(&id)?.is_some() {
        nth += 1;
        id = format!("{stem}-{nth}");
    }
    Ok(id)
}

/// A unit in `proposed` stands `approved` by one response: the bolt it names
/// is made when absent and its items are made (12, 34, I1).
fn approve_unit<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    unit: &str,
    approval: &str,
    name: &str,
    repository: &str,
    at: chrono::DateTime<chrono::Utc>,
) -> Result<(String, usize)> {
    let mut held = store.get(unit)?.expect("the unit was just written");
    let base = held.seq;
    held.record.insert("approval".into(), json!(approval));
    held.config.insert("life".into(), "approved".into());
    held.entered_at.insert("life".into(), at);
    store.put(unit, &held, base)?;
    let bolt = flywheel_domain::effects::create_bolt(store, defs, unit, name, repository, at)?
        .unwrap_or_else(|| format!("bolt/{repository}/{name}"));
    let items = flywheel_domain::effects::create_items(store, defs, unit, at)?;
    Ok((bolt, items))
}

/// The built repositories the instance tracks: what a bolt lands on. The
/// state and the blueprints are the machinery's own and never a bolt's
/// (205, 206).
fn built_repositories<W: World + ?Sized>(world: &W) -> Result<Vec<String>> {
    Ok(world
        .repositories()?
        .into_iter()
        .map(|r| r.name)
        .filter(|name| name != "flywheel-state" && name != "flywheel-blueprints")
        .collect())
}

/// A name as an id segment: lower-case words joined by hyphens.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars() {
        match c {
            c if c.is_ascii_alphanumeric() => out.push(c.to_ascii_lowercase()),
            '-' | '_' | '.' | ' ' | '/' => {
                if !out.ends_with('-') && !out.is_empty() {
                    out.push('-');
                }
            }
            _ => {}
        }
    }
    out.trim_end_matches('-').to_string()
}

/// What a capture said, as the quote on the board shows it: its first signal's
/// assertion or excerpt, or the signal's own where a signal is named (19, 113).
fn words_of<S: StateStore>(store: &S, named: &str) -> Option<String> {
    let signal = match named.starts_with(signals::PREFIX) {
        true => named.to_string(),
        false => signals::of_capture(store, named).ok()?.into_iter().next()?,
    };
    store.get(&signal).ok().flatten().as_ref().and_then(signals::text_of)
}

/// The next ordinal under a prefix. The objects are the count, so it comes from
/// `list` and never from a counter one store knows about (15).
fn next_of<S: StateStore>(store: &S, machine: &str, prefix: &str) -> Result<u64> {
    Ok(store
        .list(&Scope::Machine(machine.to_string()))?
        .objects
        .iter()
        .filter_map(|o| o.id.strip_prefix(prefix).and_then(|n| n.parse::<u64>().ok()))
        .max()
        .unwrap_or(0)
        + 1)
}

/// The capture box and the chat forward: the text is captured whole, and no
/// part of it is interpreted (19, 194).
///
/// One keyed capture per source event, with its provenance and a pointer to
/// raw material that stays outside every repository: capturing the same event
/// twice finds the capture that exists and writes nothing (111).
fn capture<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    call: &Call,
) -> Result<Outcome> {
    let text = call.text("text").unwrap_or_default();
    let source = call.text("source").unwrap_or_else(|| call.delivery.clone());
    let at = commands::now(store)?;
    // The source event's key where the caller has one; otherwise this delivery
    // is the event, which is what the page's box is (111, 19).
    let key = match &call.event_key {
        Some(event) => event.clone(),
        None => format!(
            "{}-{}",
            call.delivery,
            next_of(store, "capture", &format!("capture/{}-", call.delivery))?
        ),
    };
    let id = signals::object_of(&key);
    // The record itself goes under the machinery's prefix in the blueprints,
    // where a person reads and writes the same material by hand (111, 203,
    // 110). Capturing the same source event twice finds the record that is
    // already there and writes nothing.
    let capture = signals::Capture {
        key: key.clone(),
        source: source.clone(),
        event_at: at.to_rfc3339(),
        captured_by: call.by.clone(),
        // A pointer to the raw material, which stays outside every repository
        // (111): what the caller handed us is the pointer, not the transcript.
        raw: text.clone(),
    };
    signals::write_capture(world, &capture)?;
    if store.get(&id)?.is_none() {
        let record: BTreeMap<String, Value> = [
            ("source", json!(source)),
            ("event_key", json!(key)),
            ("event_at", json!(at.to_rfc3339())),
            ("captured_by", json!(call.by)),
            ("raw", json!(text)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        commands::put_new(store, defs, &id, "capture", None, record, at)?;
        // A forwarded single message is its own excerpt and needs no judgment,
        // and the capture machine writes its one signal through `ensure_signal`
        // (`capture.yaml` reading.captured, S21). Every other capture the page's
        // box makes carries the one ask signal the control asked for, which is
        // a control and not a judgment (19, 115, D13).
        if source != crate::chat::FORWARD {
            signals::ensure_signal(store, world, defs, &id, &call.by, at)?;
        }
    }
    // The submission is the delivery, recorded once like any response (19).
    let mut record = commands::record_call(
        store,
        defs,
        &CallRecord {
            tool: "capture",
            decision: None,
            object: Some(&id),
            answer: &text,
            args: Some(Value::Object(call.args.clone().into_iter().collect())),
            by: &call.by,
            delivery: &call.delivery,
            delivery_id: call.delivery_id.as_deref(),
            proposed_by: call.proposed_by.as_deref(),
        },
    )?;
    record.journal.push(noted("capture", &id, text));
    Ok(record)
}

fn noted(kind: &str, object: &str, text: impl Into<String>) -> commands::Noted {
    commands::Noted {
        kind: kind.to_string(),
        object: object.to_string(),
        text: text.into(),
    }
}

/// Whether a call was recorded, for a caller that wants to say so.
pub fn recorded(outcome: &Outcome) -> bool {
    matches!(outcome.outcome, Received::Recorded { .. })
}
