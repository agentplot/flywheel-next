//! The tool catalogue: the one write path into the flywheel (193, D9).
//!
//! One registry, one definition per tool, each tool's arguments named by object
//! id. The page's controls, the chat and the machinery's own commands call
//! these functions and nothing else, so no caller has an operation the others
//! lack (193). The transport is a transport: this phase serves the catalogue
//! over HTTP for the page and calls it in-process for the machinery, and both
//! enumerate the same list from `CATALOGUE` (193; proposal, What must not be
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
    /// The tool's schema, as either caller serves it.
    pub fn schema(&self) -> Value {
        json!({"name": self.name, "args": self.args, "doc": self.doc})
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
        args: &["decision", "answer", "text"],
        doc: "answer a numbered decision; the deterministic path, `yes 412` or \
              `421: <text>` in chat is this tool (194)",
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
              job (19)",
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

/// The enumeration both callers serve. The in-process caller reads this value
/// and the HTTP route writes this value, so a further transport adds a client
/// and not an operation (193).
pub fn enumerate() -> Value {
    json!({"tools": CATALOGUE.iter().map(Tool::schema).collect::<Vec<_>>()})
}

// ---------------------------------------------------------------- the bodies

use anyhow::{bail, Result};
use flywheel_atoms::{Received, Scope, StateStore, World};
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
            let at = commands::now(store)?;
            let reason = format!(
                "a session commits inside its place only and never creates a line of work, \
                 merges or lands: `{}` is refused (43)",
                call.tool
            );
            let mut fields: BTreeMap<String, Value> = BTreeMap::new();
            fields.insert("operation".into(), json!(call.tool));
            if let Some(object) = first_object_argument(call) {
                fields.insert("object".into(), json!(object));
            }
            fields.insert("reason".into(), json!(reason));
            store.append(
                &session,
                &flywheel_atoms::ThreadEntry {
                    at,
                    kind: "refusal".into(),
                    by: Some(session.clone()),
                    fields,
                },
            )?;
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
        "capture" => capture(store, world, defs, call),
        "later" => later(store, defs, call),
        "open-session" => open_session(store, defs, call),
        "propose-unit" => propose_unit(store, world, defs, call),
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
    let mut record = commands::record_call(
        store,
        defs,
        &CallRecord {
            tool: ANSWER,
            decision: Some(number as u32),
            object: None,
            answer: &answer,
            args: Some(Value::Object(call.args.clone().into_iter().collect())),
            by: &call.by,
            delivery: &call.delivery,
            delivery_id: call.delivery_id.as_deref(),
            proposed_by: call.proposed_by.as_deref(),
        },
    )?;
    record
        .journal
        .push(noted("response", &record.id, format!("{number} → {answer}")));
    Ok(record)
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
fn curate<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    _defs: &Definitions,
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
    let mut moves: Vec<signals::Move> = Vec::new();
    let mut push = |signal: &str, word: &str, names: &str| {
        let word = word.trim();
        if signal.is_empty() || word.is_empty() {
            return;
        }
        let names = names.trim();
        moves.push(signals::Move {
            signal: signal.to_string(),
            target: match names.is_empty() {
                true => word.to_string(),
                false => format!("{word} {names}"),
            },
            reason: reason.clone(),
            at: at.to_rfc3339(),
        });
    };
    if let Some(Value::Array(listed)) = call.args.get("moves") {
        for item in listed {
            let text = |name: &str| item.get(name).and_then(Value::as_str).unwrap_or_default();
            let signal = text("signal");
            // `move` is the word and `target` what it names; a `target` alone
            // carries both, as the record does (`attach <intent>`).
            match text("move") {
                "" => {
                    let mut parts = text("target").splitn(2, char::is_whitespace);
                    let word = parts.next().unwrap_or_default().to_string();
                    let names = parts.next().unwrap_or_default().to_string();
                    push(signal, &word, &names);
                }
                word => push(signal, word, text("target")),
            }
        }
    }
    for (name, value) in &call.args {
        let Some(signal) = name.strip_prefix("move.") else {
            continue;
        };
        let word = value.as_str().unwrap_or_default();
        let names = call.text(&format!("target.{signal}")).unwrap_or_default();
        push(signal, word, &names);
    }
    let mut journal = Vec::new();
    for moved in &moves {
        signals::write_move(world, moved)?;
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
    journal.push(noted("exit", &session, format!("done: {} move(s)", moves.len())));
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
    let Some(named) = call.text("bolt").map(|b| b.trim().to_string()).filter(|b| !b.is_empty()) else {
        bail!("`propose-unit` takes the bolt's name, and the call names none");
    };
    let (repository, name) = match named.strip_prefix("bolt/").and_then(|rest| rest.split_once('/')) {
        Some((repository, name)) => (repository.to_string(), slug(name)),
        None => {
            let repository = match call.text("repository").filter(|r| !r.trim().is_empty()) {
                Some(repository) => repository.trim().to_string(),
                None => {
                    let tracked: Vec<String> = world.repositories()?.into_iter().map(|r| r.name).collect();
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
    let unit = format!("unit/{repository}/{name}");
    if store.get(&unit)?.is_some() {
        bail!("`{unit}` already exists; a name is given once (I1)");
    }
    let capture = call.text("capture").map(|c| c.trim().to_string()).filter(|c| !c.is_empty());
    let subject = call
        .text("text")
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .or_else(|| capture.as_deref().and_then(|c| capture_text(store, c)));
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
    let approval = record.id.clone();
    let mut held = store.get(&unit)?.expect("the unit was just written");
    let base = held.seq;
    held.record.insert("approval".into(), json!(approval));
    held.config.insert("life".into(), "approved".into());
    held.entered_at.insert("life".into(), at);
    store.put(&unit, &held, base)?;
    let bolt = flywheel_domain::effects::create_bolt(store, defs, &unit, &name, &repository, at)?
        .unwrap_or_else(|| format!("bolt/{repository}/{name}"));
    let items = flywheel_domain::effects::create_items(store, defs, &unit, at)?;
    record.journal.push(noted(
        "propose-unit",
        &unit,
        format!("approved on {bolt} by {}, {items} item(s)", call.by),
    ));
    Ok(record)
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

/// What a capture said, from the one ask signal the page's box wrote with it
/// (19): the signal's assertion or excerpt, as the quote on the board shows it.
fn capture_text<S: StateStore>(store: &S, capture: &str) -> Option<String> {
    let key = capture.strip_prefix("capture/")?;
    let signal = signals::signal_object(key, 1);
    let held = store.get(&signal).ok().flatten()?;
    held.record
        .get("assertion")
        .or_else(|| held.record.get("excerpt"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
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
