//! The lease machine's objects (`engine/lease.yaml`).
//!
//! A lease is a branch and never a file on the shared line (D5, 167), so it is
//! not among the objects `list` returns. The machine still has to tick over it:
//! free, uncovered, acknowledged, held, stale, expired are states a host reads
//! and a decision hangs off one of them (149). So the lease objects are made
//! from the lease records at the top of every tick and the state the machine
//! reaches is written back on to the record. One instance per object the engine
//! acts on, and nothing in the domain names a lease.

use chrono::{DateTime, Utc};
use flywheel_atoms::{LeaseRecord, Object, Records};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const PREFIX: &str = "lease/";

/// The lease object's id for an object.
pub fn id_for(object: &str) -> String {
    format!("{PREFIX}{object}")
}

/// The object a lease object's id names.
pub fn object_of(id: &str) -> Option<&str> {
    id.strip_prefix(PREFIX)
}

/// Whether an object is one the engine takes a lease on at all. The rail, the
/// sinks, the hosts and the bindings' own facts are the machinery's; the lease
/// machine is about the work.
pub fn leasable(object: &Object) -> bool {
    !matches!(
        object.machine.as_str(),
        "rail" | "sink" | "host" | "fact" | "lease" | "response"
    )
}

/// The lease object for one object, from its lease record.
pub fn as_object(object: &str, record: Option<&LeaseRecord>, now: DateTime<Utc>) -> Object {
    let mut fields: BTreeMap<String, Value> = BTreeMap::new();
    if let Some(record) = record {
        fields.insert("holder".into(), json!(record.holder));
        fields.insert("taken_at".into(), json!(record.taken_at.to_rfc3339()));
        fields.insert("renewed_at".into(), json!(record.renewed_at.to_rfc3339()));
    }
    let state = record.map(|r| r.state.clone()).unwrap_or_else(flywheel_atoms::traits::free);
    Object {
        id: id_for(object),
        machine: "lease".into(),
        parent: None,
        config: [("hold".to_string(), state)].into_iter().collect(),
        entered_at: [(
            "hold".to_string(),
            record.map(|r| r.taken_at).unwrap_or(now),
        )]
        .into_iter()
        .collect(),
        record: fields,
        counters: Default::default(),
        applied_responses: record
            .map(|_| vec![])
            .unwrap_or_default(),
        seq: 0,
        created: 0,
    }
}

/// Put a lease object beside every leasable object in a read. They are the
/// engine's to tick and no part of what `list` returned.
pub fn attach<S: Records>(
    store: &S,
    objects: &mut BTreeMap<String, Object>,
    now: DateTime<Utc>,
) -> anyhow::Result<()> {
    let ids: Vec<String> = objects
        .values()
        .filter(|o| leasable(o))
        .map(|o| o.id.clone())
        .collect();
    for id in ids {
        let record = store.leases(&id)?;
        let mut lease = as_object(&id, record.as_ref(), now);
        // A response answering the lease's own decision is applied to the lease
        // object, so the acknowledgement holds across ticks.
        if let Ok(responses) = store.responses(&lease.id) {
            lease.applied_responses = responses
                .iter()
                .filter(|r| r.object.as_deref() == Some(lease.id.as_str()))
                .map(|r| r.id.clone())
                .collect();
        }
        objects.insert(lease.id.clone(), lease);
    }
    Ok(())
}
