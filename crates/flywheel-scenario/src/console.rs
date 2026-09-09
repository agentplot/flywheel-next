//! The commands, written against the trait surface.
//!
//! `seed`, `tick`, `respond`, `dictate`, `rail` and `log` reach the store
//! through the operations of 125 and the six record operations under them, and
//! never through one store's own fields. That is what lets the same commands
//! serve the stand-in today and `flywheel-store-git` in the next group, with no
//! branch between them (125, 136, 139).
//!
//! Two things are not trait operations in phase 1 and say so where they are
//! used: performing an effect, which is the profile's own binding of each
//! effect name and reaches the world through `World`, `Workspace` and
//! `Sessions`; and the log, which becomes the run record when group 6 writes
//! one (79–82).

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use flywheel_atoms::{Records, Scope, StateStore};
use flywheel_engine::runtime::{
    DecisionInstance, EvidenceSource, Object, Register, Response, ResponseKind, Snapshot,
};
use flywheel_engine::{rail, tick, Definitions, PlannedEffect};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The rail's own record: the register and the decisions standing after the
/// last derive (`profiles/record-derived.yaml`).
pub use flywheel_domain::RAIL;

// --------------------------------------------------------------- the register

/// Read the register out of the rail record.
pub fn register(store: &impl Records) -> Result<Register> {
    let record = store
        .get(RAIL)?
        .map(|o| o.record)
        .unwrap_or_default();
    Ok(Register {
        next_number: record
            .get("next")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32,
        numbers: record
            .get("numbers")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default(),
    })
}

/// Write the register and the standing set back into the rail record.
pub fn set_register(
    store: &mut impl Records,
    register: &Register,
    standing: &[String],
) -> Result<()> {
    let mut record: BTreeMap<String, Value> = BTreeMap::new();
    record.insert("next".into(), json!(register.next_number));
    record.insert("numbers".into(), json!(register.numbers));
    record.insert("standing".into(), json!(standing));
    let seq = store.get(RAIL)?.map(|o| o.seq).unwrap_or(0);
    let rail = Object {
        id: RAIL.into(),
        machine: "rail".into(),
        parent: None,
        config: Default::default(),
        entered_at: Default::default(),
        record,
        counters: Default::default(),
        applied_responses: vec![],
        seq,
        created: 0,
    };
    store.put(RAIL, &rail, seq)?;
    Ok(())
}

// ------------------------------------------------------------------ the reads

/// Every object in a scope, keyed by id — what the engine ticks over.
///
/// The lease objects are put beside them: a lease is a branch and never a file
/// on the shared line, so it is no part of what `list` returned, and the lease
/// machine still has to tick over one per object the engine acts on (D5, 128,
/// `engine/lease.yaml`).
pub fn objects(store: &impl StateStore, scope: &Scope) -> Result<BTreeMap<String, Object>> {
    let mut objects: BTreeMap<String, Object> = store
        .list(scope)?
        .objects
        .into_iter()
        .map(|o| (o.id.clone(), o))
        .collect();
    let at = now(store)?;
    flywheel_domain::leases::attach(store, &mut objects, at)?;
    Ok(objects)
}

/// The point the store names a read as of (126).
pub fn now(store: &impl StateStore) -> Result<DateTime<Utc>> {
    Ok(store.read(RAIL)?.as_of.at)
}

// ------------------------------------------------------------------- the rail

/// The standing decisions, numbered. Read through `list`, numbered through the
/// rail record, written back through it (15, 131).
pub fn rail<S: StateStore>(store: &mut S, defs: &Definitions) -> Result<Vec<DecisionInstance>> {
    let objects = objects(store, &Scope::All)?;
    let mut register = register(store)?;
    let decisions = rail::derive(defs, &objects, &mut register);
    let standing: Vec<String> = decisions.iter().map(|d| d.id.clone()).collect();
    set_register(store, &register, &standing)?;
    Ok(decisions)
}

// -------------------------------------------------------------- the responses

/// The id the next response from this delivery takes. A response is an object,
/// so the number comes from `list` and not from a counter only one store knows
/// about; a number is never reused (15).
fn next_delivery(store: &impl StateStore, delivery: &str) -> Result<u64> {
    let prefix = format!("response/{delivery}-");
    let highest = store
        .list(&Scope::Machine("response".into()))?
        .objects
        .iter()
        .filter_map(|o| o.id.strip_prefix(&prefix).and_then(|n| n.parse::<u64>().ok()))
        .max()
        .unwrap_or(0);
    Ok(highest + 1)
}

/// One line for the run record: what happened, on what, in what words (79–82).
/// The console returns them rather than writing them, because the run record is
/// the host's and group 6 gives it a home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Noted {
    pub kind: String,
    pub object: String,
    pub text: String,
}

fn noted(kind: &str, object: &str, text: impl Into<String>) -> Noted {
    Noted {
        kind: kind.to_string(),
        object: object.to_string(),
        text: text.into(),
    }
}

/// Write the response record and hand the response to the store. The record is
/// written before the transition it causes fires (129, 153).
fn record_response<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    response: &Response,
    extra: BTreeMap<String, Value>,
) -> Result<Vec<Noted>> {
    store.receive(response)?;
    let mut record = extra;
    record.insert("answer".into(), json!(response.answer));
    record.insert("given_by".into(), json!(response.given_by));
    let id = format!("response/{}", response.id);
    put_new(
        store,
        defs,
        &id,
        "response",
        None,
        record,
        response.given_at,
    )?;
    Ok(vec![noted("create", &id, "response created")])
}

/// Answer a numbered decision (129, 153).
pub fn respond<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    number: u32,
    answer: &str,
    by: &str,
) -> Result<(String, Vec<Noted>)> {
    let at = now(store)?;
    let id = format!("page-{}", next_delivery(store, "page")?);
    let response = Response {
        id: id.clone(),
        kind: ResponseKind::Answer,
        decision: Some(number),
        object: None,
        answer: answer.to_string(),
        given_by: by.to_string(),
        given_at: at,
        delivery: "page".into(),
    };
    let mut journal = record_response(
        store,
        defs,
        &response,
        [("decision".to_string(), json!(number))].into_iter().collect(),
    )?;
    journal.push(noted(
        "response",
        &format!("response/{id}"),
        format!("{number} → {answer}"),
    ));
    Ok((id, journal))
}

/// Dictate on an object: applied, never proposed (12).
pub fn dictate<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    object: &str,
    answer: &str,
    by: &str,
) -> Result<(String, Vec<Noted>)> {
    let at = now(store)?;
    let id = format!("page-{}", next_delivery(store, "page")?);
    let response = Response {
        id: id.clone(),
        kind: ResponseKind::Dictation,
        decision: None,
        object: Some(object.to_string()),
        answer: answer.to_string(),
        given_by: by.to_string(),
        given_at: at,
        delivery: "page".into(),
    };
    let mut journal = record_response(
        store,
        defs,
        &response,
        [("object".to_string(), json!(object))].into_iter().collect(),
    )?;
    journal.push(noted("dictation", object, answer));
    Ok((id, journal))
}

// ------------------------------------------------------------------ the seeds

/// Put a fresh object into the store, in its machine's initial configuration.
/// The creation ordinal is the store's to give.
pub fn put_new<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    id: &str,
    machine: &str,
    parent: Option<&str>,
    record: BTreeMap<String, Value>,
    at: DateTime<Utc>,
) -> Result<()> {
    let mut object = Object {
        id: id.to_string(),
        machine: machine.to_string(),
        parent: parent.map(String::from),
        config: Default::default(),
        entered_at: Default::default(),
        record,
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    };
    flywheel_engine::initialise(defs, &mut object, at);
    let held = store.get(id)?.map(|o| o.seq).unwrap_or(0);
    store
        .put(id, &object, held)
        .with_context(|| format!("writing {id}"))?;
    Ok(())
}

/// Write a described state of the store: every object through `put`, and the
/// register through the rail record.
pub fn seed<S: StateStore>(
    store: &mut S,
    _defs: &Definitions,
    objects: &[Object],
    register_start: Option<u32>,
) -> Result<usize> {
    for object in objects {
        let held = store.get(&object.id)?.map(|o| o.seq).unwrap_or(0);
        store.put(&object.id, object, held)?;
    }
    if let Some(next) = register_start {
        let mut register = register(store)?;
        register.next_number = next;
        set_register(store, &register, &[])?;
    }
    Ok(objects.len())
}

// ------------------------------------------------------------------- the tick

/// What one tick did, as the console reports it.
pub struct Ticked {
    pub transitions: usize,
    pub decisions: Vec<DecisionInstance>,
}

/// One tick over a scope: read, decide, write, perform.
///
/// Everything read comes from `list`, `read` and the rail record; everything
/// written goes through `put`. Performing an effect is the profile's own
/// binding of each effect name — `World`, `Workspace` and `Sessions` under it —
/// so it is handed in rather than written here (D8).
pub fn tick<S, P, N>(
    store: &mut S,
    defs: &Definitions,
    scope: &Scope,
    mut perform: P,
    mut note: N,
) -> Result<Ticked>
where
    S: StateStore + EvidenceSource,
    P: FnMut(&mut S, &str, &str, &PlannedEffect) -> bool,
    N: FnMut(&mut S, &tick::Fired, Vec<flywheel_engine::runtime::TailEntry>),
{
    let at = now(store)?;
    let objects = objects(store, scope)?;
    let responses: Vec<Response> = store.responses(RAIL).unwrap_or_default();
    let register = register(store)?;

    let fired = {
        let snapshot = Snapshot {
            objects: &objects,
            responses: &responses,
            register: &register,
            evidence: &*store,
            now: at,
        };
        tick::plan_tick(defs, &snapshot)
    };

    let transitions = fired.len();
    for f in &fired {
        let commanded = objects
            .get(&f.object)
            .map(|o| tick::commanded_effects(defs, o, f))
            .unwrap_or_default();
        let mut tail = Vec::new();
        if let Some(mut object) = store.get(&f.object)? {
            let base = object.seq;
            tail = tick::apply(defs, &mut object, f, at);
            store.put(&f.object, &object, base)?;
        }
        // What the host records about the transition, and what the state's
        // `tail:` put on the SINCE list; the run record in group 6 (79–82, 14).
        note(store, f, tail);
        for (region, e) in commanded
            .iter()
            .map(|(r, e)| (r.as_str(), e))
            .chain(f.effects.iter().map(|e| (f.region.as_str(), e)))
        {
            perform(store, &f.object, region, e);
        }
    }
    let decisions = rail(store, defs)?;
    Ok(Ticked {
        transitions,
        decisions,
    })
}
