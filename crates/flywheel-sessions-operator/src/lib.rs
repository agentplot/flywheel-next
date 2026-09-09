//! flywheel-sessions-operator: `Sessions` with the operator as the session
//! (93b, D8).
//!
//! An approved elaboration reaching `placing` must charge a session against
//! something real. In phase 1 that something is the operator (69, 110):
//! `start_session` records the session with its place and its work order and
//! starts no agent, the rail and the status view show it as the operator's to
//! run, and the operator does the work and reports through
//! `flywheel exit | offer | note | refuse` — the same command the scripted
//! stand-in plays and the phase-2 runner will call (67, 93).
//!
//! A session so charged is with-operator for every rule that turns on the type
//! (25, `profiles/sessions.yaml runners.operator`): the machinery never asks
//! about it on idle and it ends only by the operator's dictation. That is why
//! `session.activity` reads `working` while the record stands and nothing here
//! ever reports `idle`: nothing may be inferred from a person's absence from a
//! pane there is none of.
//!
//! The session record is a fact of its own, beside the session object, so a
//! start inside an effect never races the tick's own write of that object.

use anyhow::Result;
use chrono::{DateTime, Utc};
use flywheel_atoms::{Object, Records, SessionPresence, Sessions, ThreadEntry, WorkOrder};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

/// The record one session's facts are kept in.
pub fn session_fact(session: &str) -> String {
    format!("fact/session/{session}")
}

/// `Sessions` with no agent behind it. The store is shared because the trait's
/// methods observe as well as act, and both go through the record operations.
pub struct OperatorSessions<S: Records> {
    pub store: Arc<Mutex<S>>,
    /// The host that charged the session; it is the operator's to run wherever
    /// that host runs.
    pub host: String,
    pub now: DateTime<Utc>,
}

impl<S: Records> OperatorSessions<S> {
    pub fn new(store: Arc<Mutex<S>>, host: &str, now: DateTime<Utc>) -> Self {
        OperatorSessions {
            store,
            host: host.to_string(),
            now,
        }
    }

    fn locked(&self) -> Result<std::sync::MutexGuard<'_, S>> {
        self.store
            .lock()
            .map_err(|_| anyhow::anyhow!("the store's lock is poisoned"))
    }
}

/// Whether the operator has reported on this session: the newest exit entry on
/// its thread, which `flywheel exit` wrote (67, `sessions.yaml session.exit`).
pub fn exit_of<S: Records>(store: &S, session: &str) -> Option<String> {
    store
        .thread(session)
        .ok()?
        .iter()
        .filter(|e| e.kind == "exit")
        .next_back()
        .and_then(|e| e.fields.get("exit"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// The `session.*` evidence an operator-bound session answers. A host binds
/// this half of its evidence to this function.
///
/// `session.activity` never reads `idle`: under the operator runner there is no
/// pane and no keystroke to read, and a with-operator session is never asked
/// about on idle anyway (25, 93b). Nothing is inferred from what is on disk
/// (67): every one of these is a record or a thread entry.
pub fn evidence<S: Records>(store: &S, session: &str, name: &str) -> Option<Value> {
    let fact = store.get(&session_fact(session)).ok().flatten();
    let field = |n: &str| fact.as_ref().and_then(|o| o.record.get(n).cloned());
    let started = field("started_at").is_some_and(|v| !v.is_null());
    let ended = field("ended_at").is_some_and(|v| !v.is_null());
    let exit = exit_of(store, session);
    let entries = |kind: &str| -> Vec<ThreadEntry> {
        store
            .thread(session)
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.kind == kind)
            .collect()
    };
    Some(match name {
        // Present while the record stands and the operator has not reported.
        "session.pane" => json!(match started && !ended && exit.is_none() {
            true => "present",
            false => "absent",
        }),
        "session.pane_absent" => json!(!(started && !ended && exit.is_none())),
        "session.activity" => json!(match started && !ended && exit.is_none() {
            true => "working",
            false => "none",
        }),
        // No pane, so no idle clock; a with-operator session has none to read.
        "session.idle_since" => Value::Null,
        "session.operator_present" => json!(false),
        "session.exit" => json!(exit.unwrap_or_else(|| "none".into())),
        "session.exit_recorded" => json!(exit.is_some()),
        "session.question" => json!(entries("exit")
            .last()
            .and_then(|e| e.fields.get("question").cloned())),
        "session.delivered" => entries("exit")
            .last()
            .and_then(|e| e.fields.get("deliverables").cloned())
            .unwrap_or_else(|| json!([])),
        "session.expected" => field("deliverables").unwrap_or_else(|| json!([])),
        "session.offers_pending" => json!(!entries("offer").is_empty()),
        "session.refusals_pending" => json!(!entries("refusal").is_empty()),
        "session.answer_delivered" => json!(entries("answer")
            .last()
            .map(|_| true)
            .unwrap_or(true)),
        "session.message_delivered" => json!(true),
        "session.host_alive" => json!(true),
        // The one the rail and the status view read: whose session is it to run.
        "session.runner" => json!("operator"),
        _ => return None,
    })
}

// --------------------------------------------------------- the acts, in the open
//
// The trait's methods take `&self`, so the binding holds the store behind a
// lock; a host performing an effect already holds `&mut` on its store and calls
// these directly. One body serves both, so the host and the binding cannot
// drift.

/// Write one session fact, leaving every other field alone.
pub fn set<S: Records>(store: &mut S, session: &str, fields: &[(&str, Value)]) -> Result<()> {
    let id = session_fact(session);
    let held = store.get(&id)?;
    let seq = held.as_ref().map(|o| o.seq).unwrap_or(0);
    let mut record: BTreeMap<String, Value> = held.map(|o| o.record).unwrap_or_default();
    for (name, value) in fields {
        record.insert((*name).to_string(), value.clone());
    }
    let object = Object {
        id: id.clone(),
        machine: "fact".into(),
        parent: None,
        config: Default::default(),
        entered_at: Default::default(),
        record,
        counters: Default::default(),
        applied_responses: vec![],
        seq,
        created: 0,
    };
    store.put(&id, &object, seq)?;
    Ok(())
}

fn field<S: Records>(store: &S, session: &str, name: &str) -> Option<Value> {
    store
        .get(&session_fact(session))
        .ok()
        .flatten()?
        .record
        .get(name)
        .cloned()
}

fn present<S: Records>(store: &S, session: &str) -> bool {
    field(store, session, "started_at").is_some_and(|v| !v.is_null())
        && !field(store, session, "ended_at").is_some_and(|v| !v.is_null())
        && exit_of(store, session).is_none()
}

/// Say something to the operator on the session's thread. There is no pane to
/// send to, so the thread is the whole of the delivery (197).
pub fn say<S: Records>(
    store: &mut S,
    session: &str,
    by: &str,
    at: DateTime<Utc>,
    kind: &str,
    fields: BTreeMap<String, Value>,
) -> Result<()> {
    store.append(
        session,
        &ThreadEntry {
            at,
            kind: kind.to_string(),
            by: Some(by.to_string()),
            fields,
        },
    )
}

/// Record the session with its place and its work order — and start no agent.
/// A second start of the same name is not a second session (72).
pub fn start<S: Records>(
    store: &mut S,
    host: &str,
    now: DateTime<Utc>,
    order: &WorkOrder,
) -> Result<()> {
    if present(store, &order.session) {
        return Ok(());
    }
    set(
        store,
        &order.session,
        &[
            ("runner", json!("operator")),
            ("place", json!(order.place)),
            ("kind", json!(order.kind)),
            ("work_order", json!(order.body)),
            ("host", json!(host)),
            ("started_at", json!(now.to_rfc3339())),
            ("ended_at", Value::Null),
        ],
    )?;
    // The record is what the rail and the status view read; the thread is where
    // the operator is told what the work is (89).
    say(
        store,
        &order.session,
        host,
        now,
        "work-order",
        [
            ("place".to_string(), json!(order.place)),
            ("kind".to_string(), json!(order.kind)),
            ("body".to_string(), json!(order.body)),
            ("runner".to_string(), json!("operator")),
        ]
        .into_iter()
        .collect(),
    )
}

/// Only ever from a state the operator's response or the type reached (26, 69,
/// 74, 196). There is no pane to close; the record is closed.
pub fn end<S: Records>(store: &mut S, session: &str, now: DateTime<Utc>) -> Result<()> {
    set(store, session, &[("ended_at", json!(now.to_rfc3339()))])
}

pub fn answer<S: Records>(
    store: &mut S,
    session: &str,
    host: &str,
    now: DateTime<Utc>,
    text: &str,
) -> Result<()> {
    say(
        store,
        session,
        host,
        now,
        "answer",
        [
            ("text".to_string(), json!(text)),
            ("delivered".to_string(), json!(true)),
        ]
        .into_iter()
        .collect(),
    )
}

pub fn moved<S: Records>(
    store: &mut S,
    session: &str,
    host: &str,
    now: DateTime<Utc>,
) -> Result<()> {
    say(
        store,
        session,
        host,
        now,
        "message",
        [
            ("text".to_string(), json!("the line under your place moved")),
            ("delivered".to_string(), json!(true)),
        ]
        .into_iter()
        .collect(),
    )
}

/// Whether the session is one the operator is running: started, not ended and
/// not reported on.
pub fn running<S: Records>(store: &S, session: &str) -> bool {
    present(store, session)
}

impl<S: Records> Sessions for OperatorSessions<S> {
    fn start_session(&self, order: &WorkOrder) -> Result<()> {
        start(&mut *self.locked()?, &self.host, self.now, order)
    }

    fn presence(&self, session: &str) -> Result<SessionPresence> {
        // Never `Idle`: a with-operator session is not asked about on idle (25).
        Ok(match running(&*self.locked()?, session) {
            true => SessionPresence::Alive,
            false => SessionPresence::Absent,
        })
    }

    fn end_session(&self, session: &str) -> Result<()> {
        end(&mut *self.locked()?, session, self.now)
    }

    fn deliver_answer(&self, session: &str, text: &str) -> Result<()> {
        answer(&mut *self.locked()?, session, &self.host, self.now, text)
    }

    fn tell_moved(&self, session: &str) -> Result<()> {
        moved(&mut *self.locked()?, session, &self.host, self.now)
    }
}
