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
        name: "mark-intent",
        args: &["capture"],
        doc: "the operator's judgment that a capture is an intent: an intent in \
              open with its first elaboration in approved (12, 19)",
    },
    Tool {
        name: "open-session",
        args: &["repository", "text"],
        doc: "the operator's own session (69)",
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
    for name in ["object", "item", "session", "unit", "elaboration", "bolt"] {
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
    let answer = call
        .text("answer")
        .or_else(|| call.text("text"))
        .unwrap_or_default();
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
