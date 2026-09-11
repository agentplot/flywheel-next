//! What a session offered, and the records the machinery makes of it
//! (`record-derived.yaml` record_offers, 58, 59, 62).
//!
//! A session reports a finding or a chore through `flywheel offer`, which
//! writes one entry on its thread pointing at a document. It is never
//! interrupted for it (58, I5). The machinery then makes one record per offer,
//! holding the path and the entry it came from and never the text (62): a
//! finding on the session's own intent is a proposed elaboration there, a
//! finding on its own bolt a proposed unit of the fast type, a chore a
//! proposed chore unit that folds by its bolt, and anything else a signal
//! citing the path.

use crate::commands;
use anyhow::Result;
use chrono::{DateTime, Utc};
use flywheel_atoms::{Records, StateStore, ThreadEntry};
use flywheel_engine::Definitions;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// One offer, as the thread holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// `<session>#<index>`: the entry the record cites as its source.
    pub entry: String,
    pub kind: String,
    pub document: String,
}

/// The offers on one session's thread, in the order they were made. A refused
/// report is not an offer: the refusal stands in the record and nothing is
/// made of it (66, 80).
pub fn on_thread(session: &str, entries: &[ThreadEntry]) -> Vec<Offer> {
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.kind == "offer" && !entry.fields.contains_key("refused"))
        .filter_map(|(at, entry)| {
            Some(Offer {
                entry: format!("{session}#{at}"),
                kind: entry.fields.get("offer")?.as_str()?.to_string(),
                document: entry.fields.get("document")?.as_str()?.to_string(),
            })
        })
        .collect()
}

/// Whether a record already points at this offer: the object carrying its
/// document, wherever the machinery put it. This is what makes an offer
/// recorded and what makes `record_offers` a no-op the second time (62, 127).
pub fn recorded<S: Records>(store: &S, offer: &Offer) -> Result<bool> {
    for object in store.list_records(&flywheel_atoms::Scope::All)? {
        if object.record.get("document").and_then(|v| v.as_str()) == Some(offer.document.as_str()) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The offers on this session's thread that no record points at yet.
pub fn pending<S: Records>(store: &S, session: &str) -> Result<Vec<Offer>> {
    let mut out = Vec::new();
    for offer in on_thread(session, &store.thread(session)?) {
        if !recorded(store, &offer)? {
            out.push(offer);
        }
    }
    Ok(out)
}

/// The object a session runs under, and the intent or bolt above it: what a
/// finding is about when a session offers one on its own thread (58).
fn above<S: Records>(store: &S, object: &str, machine: &str) -> Result<Option<String>> {
    let mut at = store.get(object)?;
    while let Some(held) = at {
        if held.machine == machine {
            return Ok(Some(held.id));
        }
        let Some(parent) = held.parent else { break };
        at = store.get(&parent)?;
    }
    Ok(None)
}

/// The id of a new object under a parent: an object's id is
/// `<machine>/<the parent's name>/<its own name>`, so an elaboration of
/// `intent/atlas-provider-limits` is `elaboration/atlas-provider-limits/...`
/// and a unit of `bolt/atlas/plan-rows` is `unit/atlas/plan-rows/...`.
fn id_under(parent: &str, machine: &str, name: &str) -> String {
    let stem = parent.split_once('/').map(|(_, rest)| rest).unwrap_or(parent);
    format!("{machine}/{stem}/{name}")
}

/// The next free ordinal for a family of ids under one parent.
fn next<S: Records>(store: &S, parent: &str, machine: &str, name: &str) -> Result<u64> {
    let prefix = id_under(parent, machine, &format!("{name}-"));
    let highest = store
        .list_records(&flywheel_atoms::Scope::All)?
        .iter()
        .filter_map(|o| o.id.strip_prefix(&prefix).and_then(|n| n.parse::<u64>().ok()))
        .max()
        .unwrap_or(0);
    Ok(highest + 1)
}

/// Make one record per uncited offer on this session's thread.
///
/// The session is the one that offered; `owner` is the object it runs under.
/// Every record holds the document path and the entry it came from; none holds
/// the text (62).
pub fn record<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    session: &str,
    owner: &str,
    at: DateTime<Utc>,
) -> Result<Vec<String>> {
    let mut made = Vec::new();
    for offer in pending(store, session)? {
        let mut record: BTreeMap<String, Value> = BTreeMap::new();
        record.insert("document".into(), json!(offer.document));
        record.insert("sources".into(), json!([offer.entry]));

        // A chore is a chore unit of its bolt, folded with the bolt's others
        // by the `batch` field the decision names (11).
        if offer.kind == "chore" {
            let Some(bolt) = above(store, owner, "bolt")? else { continue };
            let id = id_under(&bolt, "unit", &format!("chore-{}", next(store, &bolt, "unit", "chore")?));
            record.insert("type".into(), json!("chore"));
            record.insert("type_version".into(), json!(2));
            record.insert("batch".into(), json!(bolt));
            if let Some(repository) = repository_of(store, &bolt)? {
                record.insert("repository".into(), json!(repository));
            }
            commands::put_new(store, defs, &id, "unit", Some(&bolt), record, at)?;
            made.push(id);
            continue;
        }

        // A finding on the session's own intent is a proposed elaboration
        // there; on its own bolt, a proposed unit of the fast type (58, 59).
        if let Some(intent) = above(store, owner, "intent")? {
            let id = id_under(&intent, "elaboration", &format!("finding-{}", next(store, &intent, "elaboration", "finding")?));
            record.insert("type".into(), json!("self-closing"));
            record.insert("type_version".into(), json!(2));
            commands::put_new(store, defs, &id, "elaboration", Some(&intent), record, at)?;
            made.push(id);
            continue;
        }
        if let Some(bolt) = above(store, owner, "bolt")? {
            let id = id_under(&bolt, "unit", &format!("finding-{}", next(store, &bolt, "unit", "finding")?));
            record.insert("type".into(), json!("fast"));
            record.insert("type_version".into(), json!(3));
            record.insert("target".into(), json!(bolt));
            if let Some(repository) = repository_of(store, &bolt)? {
                record.insert("repository".into(), json!(repository));
            }
            commands::put_new(store, defs, &id, "unit", Some(&bolt), record, at)?;
            made.push(id);
            continue;
        }
    }
    Ok(made)
}

fn repository_of<S: Records>(store: &S, object: &str) -> Result<Option<String>> {
    Ok(store
        .get(object)?
        .and_then(|o| o.record.get("repository").and_then(|v| v.as_str()).map(String::from)))
}
