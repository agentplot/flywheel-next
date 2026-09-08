//! The object envelope: what every object record carries whatever kind it is,
//! written over the engine's generic rec reader and writer.
//!
//! The envelope's own fields are a closed set. Every other field of a record is
//! the object's own, and is carried through untouched — which is why a type
//! composed of existing atoms needs no code change here (57, 85).

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use flywheel_engine::rec::Record;
use flywheel_engine::runtime::Object;
use serde_json::Value;

/// The envelope's field names. Anything else in a record is the object's own.
pub const ENVELOPE_FIELDS: &[&str] = &[
    "id", "machine", "parent", "state", "entered", "seq", "created", "applied", "counter",
];

fn parse_pair(v: &str) -> Result<(&str, &str)> {
    v.split_once('=')
        .ok_or_else(|| anyhow!("expected `name=value`, found {v:?}"))
}

/// Write an object as one rec record of kind `object`.
pub fn to_record(object: &Object) -> Record {
    let mut r = Record {
        kind: Some("object".to_string()),
        fields: vec![],
    };
    r.fields.push(("id".into(), object.id.clone()));
    r.fields.push(("machine".into(), object.machine.clone()));
    if let Some(p) = &object.parent {
        r.fields.push(("parent".into(), p.clone()));
    }
    for (region, state) in &object.config {
        r.fields.push(("state".into(), format!("{region}={state}")));
    }
    for (region, at) in &object.entered_at {
        r.fields
            .push(("entered".into(), format!("{region}={}", at.to_rfc3339())));
    }
    r.fields.push(("seq".into(), object.seq.to_string()));
    r.fields.push(("created".into(), object.created.to_string()));
    for id in &object.applied_responses {
        r.fields.push(("applied".into(), id.clone()));
    }
    for (name, n) in &object.counters {
        r.fields.push(("counter".into(), format!("{name}={n}")));
    }
    // The object's own fields, JSON-encoded so a string that looks like a
    // number comes back a string.
    for (name, value) in &object.record {
        r.fields.push((name.clone(), value.to_string()));
    }
    r
}

/// Read an object back out of a rec record.
pub fn from_record(r: &Record) -> Result<Object> {
    let id = r
        .get("id")
        .ok_or_else(|| anyhow!("an object record has no id"))?
        .to_string();
    let machine = r
        .get("machine")
        .ok_or_else(|| anyhow!("object {id} has no machine"))?
        .to_string();

    let mut object = Object {
        id,
        machine,
        parent: r.get("parent").map(String::from),
        config: Default::default(),
        entered_at: Default::default(),
        record: Default::default(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    };

    for v in r.all("state") {
        let (region, state) = parse_pair(v)?;
        object.config.insert(region.into(), state.into());
    }
    for v in r.all("entered") {
        let (region, at) = parse_pair(v)?;
        let at: DateTime<Utc> = DateTime::parse_from_rfc3339(at)
            .map_err(|e| anyhow!("object {}: entered {v:?}: {e}", object.id))?
            .with_timezone(&Utc);
        object.entered_at.insert(region.into(), at);
    }
    if let Some(v) = r.get("seq") {
        object.seq = v.parse()?;
    }
    if let Some(v) = r.get("created") {
        object.created = v.parse()?;
    }
    object.applied_responses = r.all("applied").into_iter().map(String::from).collect();
    for v in r.all("counter") {
        let (name, n) = parse_pair(v)?;
        object.counters.insert(name.into(), n.parse()?);
    }
    for (name, value) in &r.fields {
        if ENVELOPE_FIELDS.contains(&name.as_str()) {
            continue;
        }
        let parsed: Value = serde_json::from_str(value)
            .map_err(|e| anyhow!("object {}: field {name}: {e}", object.id))?;
        object.record.insert(name.clone(), parsed);
    }
    Ok(object)
}

/// A file of object records.
pub fn write_all(objects: &[Object]) -> String {
    let records: Vec<Record> = objects.iter().map(to_record).collect();
    flywheel_engine::rec::write(&records)
}

pub fn read_all(text: &str) -> Result<Vec<Object>> {
    flywheel_engine::rec::parse(text)
        .iter()
        .map(from_record)
        .collect()
}
