//! The sinks the operator reads, as objects (`engine/sink.yaml`, 14, 148, 236).
//!
//! A sink is an object like any other: its record carries what the manifest
//! said about it — its kind, the member it belongs to, the surface it delivers
//! on, the decision kinds routed to it — and the one piece of recorded state
//! behind the tail, its delivery mark (14). Nothing about a rendering is stored
//! (15): the mark is a point, and what the tail holds since it is derived from
//! the register and the objects like everything else.
//!
//! Exactly one presenter delivers to a sink at a time, held by lease or pinned
//! by the manifest — "pinned" being 148's own word (148). This release binds the
//! lease alone: a host that holds the sink's lease is its presenter, and a sink
//! record carrying a pin is refused rather than half-honoured, because a pin no
//! host reads is not a pin.
//!
//! Sinks are per member from the start, even with one member, so phase 4's
//! dispatch and phase 5's per-tier dispatcher contend for a sink that already
//! exists rather than for one invented for them (236, 236a).

use anyhow::{bail, Result};
use chrono::{DateTime, Duration, Utc};
use flywheel_atoms::{LeaseOp, LeaseOutcome, Records, Scope, StateStore};
use crate::commands;
use flywheel_engine::Definitions;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Every sink object's id begins here, so `list` finds them without the caller
/// naming one.
pub const PREFIX: &str = "sink/";

/// The sink object's id for a sink the manifest named.
pub fn id_for(name: &str) -> String {
    format!("{PREFIX}{name}")
}

/// What the operator set about one sink, as the manifest carries it. The
/// machinery reads this and writes the mark; nothing else about a sink is the
/// machinery's to decide (82, 236a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    /// The manifest's own name for it, which is the object's id suffix.
    pub name: String,
    /// `chat`, `page` or `bell` (`engine/sink.yaml` record.kind).
    pub kind: String,
    /// The member this sink belongs to, or none for a shared channel (236).
    pub member: Option<String>,
    /// The chat channel, the page route, or the named multiplexer surface.
    pub surface: String,
    /// The decision kinds delivered here, from the manifest (82).
    pub routes: Vec<String>,
    /// `own` or `all`: whether a member's delivery is filtered to what they own
    /// (237). One operator owns everything, so `all` is the phase-1 default.
    pub filter: String,
}

impl Spec {
    /// A chat sink for one member on one address (236a).
    pub fn chat(name: &str, member: &str, surface: &str) -> Spec {
        Spec {
            name: name.to_string(),
            kind: "chat".into(),
            member: Some(member.to_string()),
            surface: surface.to_string(),
            routes: vec!["all".into()],
            filter: "all".into(),
        }
    }

    /// The member's page sink: the one that exists for every member with no
    /// address of their own (236, 236a).
    pub fn page(name: &str, member: &str, surface: &str) -> Spec {
        Spec {
            name: name.to_string(),
            kind: "page".into(),
            member: Some(member.to_string()),
            surface: surface.to_string(),
            routes: vec!["all".into()],
            filter: "all".into(),
        }
    }

    pub fn routing(mut self, routes: &[&str]) -> Spec {
        self.routes = routes.iter().map(|k| k.to_string()).collect();
        self
    }
}

/// One sink as it stands in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sink {
    pub id: String,
    pub kind: String,
    pub member: Option<String>,
    pub surface: String,
    pub routes: Vec<String>,
    pub filter: String,
    /// The mark: everything that entered a tail state after it is this sink's
    /// tail (14).
    pub delivered_at: Option<DateTime<Utc>>,
    /// The last delivery's own id on the platform — a chat message id, the
    /// page's build id — recorded on the sink beside the mark
    /// (`surfaces.yaml` effects.deliver_rail).
    pub delivery: Option<String>,
    /// 148's other half, which this release does not bind.
    pub pinned_to: Option<String>,
}

impl Sink {
    /// The name the manifest gave it, without the prefix.
    pub fn name(&self) -> &str {
        self.id.strip_prefix(PREFIX).unwrap_or(&self.id)
    }

    /// Whether a decision of this kind is routed here (82). `all` routes every
    /// kind, which is what a sole sink with nothing set carries.
    pub fn routes_kind(&self, kind: &str) -> bool {
        self.routes.iter().any(|r| r == "all" || r == kind)
    }
}

/// Read one sink out of the store.
pub fn read<S: Records>(store: &S, id: &str) -> Result<Option<Sink>> {
    let Some(object) = store.get(id)? else {
        return Ok(None);
    };
    if object.machine != "sink" {
        return Ok(None);
    }
    Ok(Some(from_record(&object.id, &object.record)))
}

/// Every sink the instance holds, in the order `list` returns them.
pub fn all<S: StateStore>(store: &S) -> Result<Vec<Sink>> {
    Ok(store
        .list_records(&Scope::All)?
        .into_iter()
        .filter(|o| o.machine == "sink")
        .map(|o| from_record(&o.id, &o.record))
        .collect())
}

/// One sink from an object already in hand, for a caller reading a listing.
pub fn from_object(object: &flywheel_engine::Object) -> Sink {
    from_record(&object.id, &object.record)
}

fn from_record(id: &str, record: &BTreeMap<String, Value>) -> Sink {
    let text = |name: &str| {
        record
            .get(name)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from)
    };
    Sink {
        id: id.to_string(),
        kind: text("kind").unwrap_or_else(|| "chat".into()),
        member: text("member"),
        surface: text("surface").unwrap_or_default(),
        routes: record
            .get("routes")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_else(|| vec!["all".into()]),
        filter: text("filter").unwrap_or_else(|| "all".into()),
        delivered_at: text("delivered_at")
            .and_then(|t| DateTime::parse_from_rfc3339(&t).ok())
            .map(|t| t.with_timezone(&Utc)),
        delivery: text("delivery"),
        pinned_to: text("pinned_to"),
    }
}

/// Write the sink the manifest names, or leave the one that stands as it is.
///
/// The mark is never reset by this: a sink the operator renames or re-routes
/// keeps the tail it had, because the mark is recorded state and the manifest
/// is not (14).
pub fn ensure<S: StateStore>(store: &mut S, defs: &Definitions, spec: &Spec) -> Result<Sink> {
    let id = id_for(&spec.name);
    let at = commands::now(store)?;
    let held = store.get(&id)?;
    let mut record: BTreeMap<String, Value> = held.as_ref().map(|o| o.record.clone()).unwrap_or_default();
    record.insert("kind".into(), json!(spec.kind));
    record.insert("surface".into(), json!(spec.surface));
    record.insert("routes".into(), json!(spec.routes));
    record.insert("filter".into(), json!(spec.filter));
    match &spec.member {
        Some(member) => record.insert("member".into(), json!(member)),
        // A shared channel is a sink of its own with one mark and no member
        // (236).
        None => record.remove("member"),
    };
    match held {
        Some(mut object) => {
            let base = object.seq;
            object.record = record;
            store.put(&id, &object, base)?;
        }
        None => commands::put_new(store, defs, &id, "sink", None, record, at)?,
    }
    read(store, &id)?
        .ok_or_else(|| anyhow::anyhow!("the sink `{id}` was written and does not read back"))
}

// ------------------------------------------------------------------ the tail

/// One thing that reached a tail state: what the object is, what it reached,
/// and when (14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailItem {
    pub object: String,
    /// The word the machine's own state gives it: done, landed, closed,
    /// dropped, merged.
    pub kind: String,
    pub state: String,
    pub at: DateTime<Utc>,
}

/// The tail since one sink's own mark (14, 236).
///
/// Derived like the decisions and stored nowhere: an object whose current state
/// carries `tail:` and which entered it after this sink's mark. Two sinks with
/// different marks therefore have different tails, and a sink that has never
/// delivered has the whole of it.
pub fn tail<S: StateStore>(store: &S, defs: &Definitions, sink: &Sink) -> Result<Vec<TailItem>> {
    let mut out: Vec<TailItem> = Vec::new();
    for object in store.list_records(&Scope::All)? {
        for (region, state) in &object.config {
            let Some((_region, definition)) = flywheel_engine::tick::state_def(defs, &object, region)
            else {
                continue;
            };
            let Some(kind) = definition.tail.clone() else {
                continue;
            };
            let Some(at) = object.entered_at.get(region).copied() else {
                continue;
            };
            if sink.delivered_at.is_some_and(|mark| at <= mark) {
                continue;
            }
            out.push(TailItem {
                object: object.id.clone(),
                kind,
                state: state.clone(),
                at,
            });
        }
    }
    out.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.object.cmp(&b.object)));
    Ok(out)
}

/// Advance the sink's mark to the point it has now delivered through, in one
/// write with the delivery's own id beside it (14,
/// `surfaces.yaml` effects.deliver_rail).
pub fn mark<S: StateStore>(
    store: &mut S,
    sink: &str,
    at: DateTime<Utc>,
    delivery: &str,
) -> Result<Sink> {
    let Some(mut object) = store.get(sink)? else {
        bail!("`{sink}` is no sink of this instance");
    };
    let base = object.seq;
    object
        .record
        .insert("delivered_at".into(), json!(at.to_rfc3339()));
    object.record.insert("delivery".into(), json!(delivery));
    store.put(sink, &object, base)?;
    read(store, sink)?.ok_or_else(|| anyhow::anyhow!("`{sink}` does not read back"))
}

// ------------------------------------------------------------- the routing

/// The sinks a notification of one kind reaches: the ones the operator routed
/// it to (82).
pub fn routed<S: StateStore>(store: &S, kind: &str) -> Result<Vec<Sink>> {
    Ok(all(store)?
        .into_iter()
        .filter(|sink| sink.routes_kind(kind))
        .collect())
}

/// One thing the machinery noticed, on the object it is about (82, 79).
///
/// A notice is state, not a line a host printed: whichever host noticed it
/// writes it on the object, and whichever host presents a sink the kind is
/// routed to carries it. That is what makes nothing the machinery notices
/// visible only on the host that noticed it (82).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub object: String,
    /// The kind the operator routes by (82).
    pub kind: String,
    pub text: String,
    /// The host that noticed it.
    pub by: String,
    pub at: DateTime<Utc>,
}

/// The thread entry kind a notice is written as.
pub const NOTICE: &str = "notice";

/// Notice something, on the object it is about. Any host may; no host shows it
/// anywhere of its own (82).
pub fn notice<S: StateStore>(
    store: &mut S,
    object: &str,
    kind: &str,
    text: &str,
    by: &str,
    at: DateTime<Utc>,
) -> Result<()> {
    store.append(
        object,
        &flywheel_atoms::ThreadEntry {
            at,
            kind: NOTICE.to_string(),
            by: Some(by.to_string()),
            // Not `kind`: the thread entry's own kind is `notice`, and the
            // record format keeps that name for itself.
            fields: [
                ("notice_kind".to_string(), json!(kind)),
                ("text".to_string(), json!(text)),
            ]
            .into_iter()
            .collect(),
        },
    )
}

/// The notices one sink carries: those of a kind routed to it, since its own
/// mark (82, 14).
pub fn notices<S: StateStore>(store: &S, sink: &Sink) -> Result<Vec<Notice>> {
    let mut out = Vec::new();
    for object in store.list_records(&Scope::All)? {
        for entry in store.thread(&object.id)? {
            if entry.kind != NOTICE {
                continue;
            }
            if sink.delivered_at.is_some_and(|mark| entry.at <= mark) {
                continue;
            }
            let kind = entry
                .fields
                .get("notice_kind")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if !sink.routes_kind(&kind) {
                continue;
            }
            out.push(Notice {
                object: object.id.clone(),
                kind,
                text: entry
                    .fields
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                by: entry.by.clone().unwrap_or_default(),
                at: entry.at,
            });
        }
    }
    out.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.object.cmp(&b.object)));
    Ok(out)
}

// ---------------------------------------------------------- the away holder

/// A host past its stale window, with the since-when a link has to say (150a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Away {
    pub host: String,
    pub since: DateTime<Utc>,
}

impl Away {
    /// What a link to it says instead of failing silently (308, 150a).
    pub fn said(&self) -> String {
        format!("{} is away since {}", self.host, self.since.to_rfc3339())
    }
}

/// Every object held by a host past its stale window, with the host and the
/// since-when (150a).
///
/// A laptop is intermittent by default, so this is the ordinary case and not an
/// error: its leases stand, it raises no attention line, and a link to what it
/// holds says it is away rather than opening nothing (150a, 308).
pub fn away_by_object<S: StateStore>(
    store: &S,
    now: DateTime<Utc>,
    stale: Duration,
) -> Result<BTreeMap<String, Away>> {
    let hosts = store.hosts()?;
    let mut out = BTreeMap::new();
    for object in store.list_records(&Scope::All)? {
        let Some(lease) = store.leases(&object.id)? else {
            continue;
        };
        if lease.holder.is_empty() {
            continue;
        }
        let Some(host) = hosts.iter().find(|h| h.host == lease.holder) else {
            continue;
        };
        if now - host.last_seen < stale {
            continue;
        }
        out.insert(
            object.id.clone(),
            Away {
                host: host.host.clone(),
                since: host.last_seen,
            },
        );
    }
    Ok(out)
}

// ------------------------------------------------------------- the presenter

/// Take this sink's presenter lease. The push is the compare-and-swap, so two
/// hosts cannot both hold it (134, 148, 162, D5).
pub fn take_presenter<S: StateStore>(store: &mut S, sink: &str, host: &str) -> Result<LeaseOutcome> {
    refuse_a_pin(store, sink)?;
    store.lease(&LeaseOp::Take {
        object: sink.to_string(),
        holder: host.to_string(),
    })
}

/// Give it up, so another host may present (148).
pub fn release_presenter<S: StateStore>(store: &mut S, sink: &str, host: &str) -> Result<()> {
    store.lease(&LeaseOp::Release {
        object: sink.to_string(),
        holder: host.to_string(),
    })?;
    Ok(())
}

/// Who presents this sink: the holder of its lease (148).
pub fn presenter<S: Records>(store: &S, sink: &str) -> Result<Option<String>> {
    refuse_a_pin(store, sink)?;
    Ok(store
        .leases(sink)?
        .map(|l| l.holder)
        .filter(|h| !h.is_empty()))
}

/// Whether this host is the one that delivers to this sink. Every other host
/// that could present it does not, so the operator sees each delivery once
/// (148).
///
/// A sink nobody holds is nobody's to deliver to: a presenter is taken, never
/// assumed, because assuming it is how two hosts both deliver.
pub fn presents<S: Records>(store: &S, sink: &str, host: &str) -> Result<bool> {
    Ok(presenter(store, sink)?.as_deref() == Some(host))
}

/// 148 gives two ways to hold a presenter and this release binds one. A sink
/// pinned by the manifest is refused and said so, rather than delivered to by
/// whoever happens to hold the lease.
fn refuse_a_pin<S: Records>(store: &S, sink: &str) -> Result<()> {
    let Some(object) = store.get(sink)? else {
        return Ok(());
    };
    if let Some(pinned) = object.record.get("pinned_to").and_then(|v| v.as_str()) {
        if !pinned.is_empty() {
            bail!(
                "`{sink}` is pinned to `{pinned}`; this release binds 148's lease alone and no \
                 manifest pin, and a pin no host reads is not a pin"
            );
        }
    }
    Ok(())
}
