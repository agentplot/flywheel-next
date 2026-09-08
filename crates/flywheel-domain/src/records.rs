//! The domain's record schemas, over the engine's generic rec reader and
//! writer. Every durable fact the machinery writes has one of these shapes.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use flywheel_atoms::{HostRecord, LeaseRecord, ThreadEntry};
use flywheel_engine::rec::Record;
use flywheel_engine::runtime::{Response, ResponseKind};
use serde_json::Value;

fn time(r: &Record, name: &str) -> Result<DateTime<Utc>> {
    let v = r
        .get(name)
        .ok_or_else(|| anyhow!("a record has no {name}"))?;
    Ok(DateTime::parse_from_rfc3339(v)
        .map_err(|e| anyhow!("{name} {v:?}: {e}"))?
        .with_timezone(&Utc))
}

fn text(r: &Record, name: &str) -> Result<String> {
    Ok(r.get(name)
        .ok_or_else(|| anyhow!("a record has no {name}"))?
        .to_string())
}

// ------------------------------------------------------------- the op-response
//
// One file per delivery id, so a second delivery writes the same bytes (137).
// `given_by` and `given_at` are fields from the first commit (153, D10).

pub fn response_to_record(response: &Response) -> Record {
    let mut r = Record {
        kind: Some("response".to_string()),
        fields: vec![],
    };
    r.set("id", &response.id);
    r.set(
        "kind",
        match response.kind {
            ResponseKind::Answer => "answer",
            ResponseKind::Dictation => "dictation",
        },
    );
    if let Some(n) = response.decision {
        r.set("decision", &n.to_string());
    }
    if let Some(o) = &response.object {
        r.set("object", o);
    }
    r.set("answer", &response.answer);
    r.set("given_by", &response.given_by);
    r.set("given_at", &response.given_at.to_rfc3339());
    r.set("delivery", &response.delivery);
    r
}

pub fn response_from_record(r: &Record) -> Result<Response> {
    let kind = match r.get("kind") {
        Some("answer") => ResponseKind::Answer,
        Some("dictation") => ResponseKind::Dictation,
        other => return Err(anyhow!("a response record has kind {other:?}")),
    };
    Ok(Response {
        id: text(r, "id")?,
        kind,
        decision: r.get("decision").map(str::parse).transpose()?,
        object: r.get("object").map(String::from),
        answer: text(r, "answer")?,
        given_by: r.get("given_by").unwrap_or_default().to_string(),
        given_at: time(r, "given_at")?,
        delivery: r.get("delivery").unwrap_or_default().to_string(),
    })
}

// ------------------------------------------------------------- the thread entry
//
// What a session, a command or the machinery wrote on an object (67).

pub fn thread_to_record(entry: &ThreadEntry) -> Record {
    let mut r = Record {
        kind: Some("thread".to_string()),
        fields: vec![],
    };
    r.set("at", &entry.at.to_rfc3339());
    r.set("kind", &entry.kind);
    if let Some(by) = &entry.by {
        r.set("by", by);
    }
    for (name, value) in &entry.fields {
        r.fields.push((name.clone(), value.to_string()));
    }
    r
}

const THREAD_FIELDS: &[&str] = &["at", "kind", "by"];

pub fn thread_from_record(r: &Record) -> Result<ThreadEntry> {
    let mut entry = ThreadEntry {
        at: time(r, "at")?,
        kind: text(r, "kind")?,
        by: r.get("by").map(String::from),
        fields: Default::default(),
    };
    for (name, value) in &r.fields {
        if THREAD_FIELDS.contains(&name.as_str()) {
            continue;
        }
        let parsed: Value =
            serde_json::from_str(value).map_err(|e| anyhow!("thread field {name}: {e}"))?;
        entry.fields.insert(name.clone(), parsed);
    }
    Ok(entry)
}

// ------------------------------------------------------------------- the lease

pub fn lease_to_record(lease: &LeaseRecord) -> Record {
    let mut r = Record {
        kind: Some("lease".to_string()),
        fields: vec![],
    };
    r.set("object", &lease.object);
    r.set("holder", &lease.holder);
    r.set("taken_at", &lease.taken_at.to_rfc3339());
    r.set("renewed_at", &lease.renewed_at.to_rfc3339());
    r
}

pub fn lease_from_record(r: &Record) -> Result<LeaseRecord> {
    Ok(LeaseRecord {
        object: text(r, "object")?,
        holder: text(r, "holder")?,
        taken_at: time(r, "taken_at")?,
        renewed_at: time(r, "renewed_at")?,
    })
}

// ------------------------------------------------------------ the host record

pub fn host_to_record(host: &HostRecord) -> Record {
    let mut r = Record {
        kind: Some("host".to_string()),
        fields: vec![],
    };
    r.set("host", &host.host);
    r.set("last_seen", &host.last_seen.to_rfc3339());
    r.set("bound", &host.bound.to_string());
    r.set("intermittent", &host.intermittent.to_string());
    r
}

pub fn host_from_record(r: &Record) -> Result<HostRecord> {
    Ok(HostRecord {
        host: text(r, "host")?,
        last_seen: time(r, "last_seen")?,
        bound: r.get("bound").unwrap_or("0").parse()?,
        intermittent: r.get("intermittent").unwrap_or("false").parse()?,
    })
}
