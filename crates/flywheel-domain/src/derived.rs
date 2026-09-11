//! The evidence every profile derives from the record operations alone
//! (`profiles/record-derived.yaml`).
//!
//! `get`, `put`, `append`, `list`, `responses`, `leases` and `hosts` are the
//! six operations; every name below is a function of them, so it reads the same
//! on the stand-in, on the git-only profile and on the tracker. Nothing here
//! reads a clock of its own: `now` is handed in, so no wall-clock time reaches
//! a guard.

use chrono::{DateTime, Duration, Utc};
use flywheel_atoms::{Records, Scope};
use flywheel_engine::runtime::Register;
use serde_json::{json, Value};

/// What a host is, as the manifest declared it: what it takes leases within
/// (149, 217).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Declaration {
    /// Repositories this host takes work in; empty declares none.
    pub repositories: Vec<String>,
    /// Unit and elaboration types it takes; empty declares none.
    pub types: Vec<String>,
    /// Kinds of work it takes; `all` covers every kind.
    pub kinds: Vec<String>,
}

impl Declaration {
    /// Whether this declaration covers an object. A host takes a lease only
    /// within it (149).
    pub fn covers(&self, object: &flywheel_engine::Object) -> bool {
        let named = |list: &[String], value: Option<&str>| -> bool {
            if list.iter().any(|k| k == "all") {
                return true;
            }
            match value {
                None => true,
                Some(v) => list.iter().any(|k| k == v),
            }
        };
        // The rail, the sinks and the hosts themselves are the machinery's own
        // objects and belong to whichever host runs; a declaration narrows the
        // work, not the machinery.
        if matches!(object.machine.as_str(), "rail" | "sink" | "host" | "fact") {
            return true;
        }
        let repository = object.record.get("repository").and_then(|v| v.as_str());
        if let Some(repository) = repository {
            if !self.repositories.iter().any(|r| r == repository || r == "all") {
                return false;
            }
        }
        let object_type = object.record.get("type").and_then(|v| v.as_str());
        if object_type.is_some() && !self.types.is_empty() && !named(&self.types, object_type) {
            return false;
        }
        if !self.kinds.is_empty() && !named(&self.kinds, Some(&object.machine)) {
            return false;
        }
        true
    }
}

/// What the derived evidence is read against: who this host is, when it is, and
/// the engine windows in force.
#[derive(Debug, Clone)]
pub struct Reading {
    pub me: String,
    pub now: DateTime<Utc>,
    pub stale: Duration,
    /// The rail's register and the decisions standing after the last derive,
    /// for `response.decision_present`.
    pub register: Register,
    pub standing: Vec<String>,
    /// What the hosts declared they take. With none given every host covers
    /// everything, which is what a single host with no declaration is (149).
    pub declarations: Vec<Declaration>,
}

impl Reading {
    pub fn new(me: &str, now: DateTime<Utc>) -> Reading {
        Reading {
            me: me.to_string(),
            now,
            stale: Duration::minutes(5),
            register: Register::default(),
            standing: vec![],
            declarations: vec![],
        }
    }
}

/// One evidence name, derived from the record operations. `None` means this is
/// not a name the record layer answers; the world, workspace and sessions
/// bindings answer the rest.
pub fn evidence<S: Records>(
    store: &S,
    reading: &Reading,
    object: &str,
    name: &str,
) -> Option<Value> {
    let held = store.get(object).ok().flatten();
    let field = |n: &str| held.as_ref().and_then(|o| o.record.get(n).cloned());
    Some(match name {
        "now" => json!(reading.now.to_rfc3339()),
        "state" => serde_json::to_value(held.as_ref().map(|o| o.config.clone())?).ok()?,
        "entered_at" => serde_json::to_value(held.as_ref().map(|o| o.entered_at.clone())?).ok()?,
        "seq" => json!(held.as_ref()?.seq),
        "applied_responses" => json!(held.as_ref()?.applied_responses.clone()),

        // ---- the host's own record (147, 163)
        "host.bound" => {
            let host = host_name(object);
            json!(store
                .hosts()
                .ok()?
                .into_iter()
                .find(|h| h.host == host)
                .map(|h| h.bound)
                .or_else(|| field("bound").and_then(|v| v.as_u64()).map(|n| n as u32))?)
        }
        "host.intermittent" => json!(store
            .hosts()
            .ok()?
            .into_iter()
            .find(|h| h.host == host_name(object))
            .map(|h| h.intermittent)
            .or_else(|| field("intermittent").and_then(|v| v.as_bool()))
            .unwrap_or(true)),
        "host.last_seen" => {
            let host = host_name(object);
            match store.hosts().ok()?.into_iter().find(|h| h.host == host) {
                Some(h) => json!(h.last_seen.to_rfc3339()),
                None => field("last_seen")?,
            }
        }
        // The sessions this host runs: session records naming it, still alive.
        "host.running" => {
            let host = host_name(object);
            json!(store
                .list_records(&Scope::All)
                .ok()?
                .iter()
                .filter(|o| o.id.starts_with("fact/session/"))
                .filter(|o| o.record.get("host").and_then(|v| v.as_str()) == Some(host.as_str()))
                .filter(|o| {
                    o.record.get("started_at").is_some_and(|v| !v.is_null())
                        && !o.record.get("ended_at").is_some_and(|v| !v.is_null())
                })
                .count())
        }
        // No lease record names the host (150).
        "host.leases_expired" => {
            let host = host_name(object);
            json!(!leases_of(store, &host).into_iter().any(|_| true))
        }
        // A decision or approved work waits on what this host holds (150a).
        "host.work_waiting" => {
            let host = host_name(object);
            let objects = store.list_records(&Scope::All).ok()?;
            json!(leases_of(store, &host).iter().any(|held| {
                objects.iter().any(|o| {
                    &o.id == held
                        && o.config
                            .values()
                            .any(|state| state == "approved" || state == "waiting")
                })
            }))
        }

        // ---- leases (128)
        // A holder that is nobody is no holder: `exists:` reads it as absent.
        "lease.holder" => store
            .leases(lease_object(object))
            .ok()?
            .map(|l| l.holder)
            .filter(|h| !h.is_empty())
            .map(Value::String)?,
        // No host's declaration covering it is what makes it uncovered (149).
        "lease.coverable" => {
            let held = store.get(lease_object(object)).ok().flatten();
            match held {
                None => json!(true),
                Some(held) => json!(reading.declarations.is_empty()
                    || reading.declarations.iter().any(|d| d.covers(&held))),
            }
        }
        "lease.holder_is_me" => json!(store
            .leases(lease_object(object))
            .ok()?
            .map(|l| l.holder == reading.me)
            .unwrap_or(false)),
        "lease.renewed_at" => json!(store
            .leases(lease_object(object))
            .ok()?
            .map(|l| l.renewed_at.to_rfc3339())?),

        // ---- responses (129, 137)
        "response.applied" => {
            let id = object.strip_prefix("response/").unwrap_or(object);
            json!(store
                .list_records(&Scope::All)
                .ok()?
                .iter()
                .any(|o| o.applied_responses.iter().any(|a| a == id)))
        }
        "response.decision_present" => {
            let id = object.strip_prefix("response/").unwrap_or(object);
            let responses = store.responses(crate::RAIL).ok()?;
            let Some(response) = responses.iter().find(|r| r.id == id) else {
                return Some(json!(false));
            };
            match response.decision {
                // A dictation names an operation of the catalogue. One that
                // asserts work was done names none — no such tool exists — so
                // it is unapplicable and comes back once under attention
                // (4, 6, 193). A dictation recorded before the catalogue
                // named its tool carries none and stands as it always did.
                None => json!(store
                    .get(&format!("response/{id}"))
                    .ok()
                    .flatten()
                    .and_then(|o| o.record.get("tool").and_then(|v| v.as_str().map(String::from)))
                    .map(|tool| crate::commands::is_operation(&tool))
                    .unwrap_or(true)),
                Some(number) => json!(reading
                    .register
                    .decision_of(number)
                    .map(|d| reading.standing.iter().any(|s| s == d))
                    .unwrap_or(false)),
            }
        }
        "response.answer" => {
            let id = object.strip_prefix("response/").unwrap_or(object);
            json!(store
                .responses(crate::RAIL)
                .ok()?
                .iter()
                .find(|r| r.id == id)
                .map(|r| r.answer.clone()))
        }
        // A sink's delivery newer than the response carried it under attention
        // (6, 14, 82). With no sink delivering, nothing has been reported and
        // the decision stands, which is what "never dropped" means.
        "response.reported" => {
            let id = object.strip_prefix("response/").unwrap_or(object);
            let Some(given_at) = store
                .responses(crate::RAIL)
                .ok()?
                .iter()
                .find(|r| r.id == id)
                .map(|r| r.given_at)
            else {
                return Some(json!(false));
            };
            json!(store
                .list_records(&Scope::All)
                .ok()?
                .iter()
                .filter(|o| o.machine == "sink")
                .map(|o| crate::sinks::from_object(o))
                .filter(|sink| sink.routes_kind("response-unapplicable"))
                .any(|sink| sink.delivered_at.is_some_and(|mark| mark > given_at)))
        }

        // ---- the sinks (14, 18, 82, 148, `surfaces.yaml` evidence)
        //
        // Both are read from the register the rail record holds and the mark
        // the sink record holds, so they are the same in every profile and
        // nothing about a rendering is stored to answer them (15).
        "sink.due" => {
            let Some(sink) = crate::sinks::read(store, object).ok().flatten() else {
                return Some(json!(false));
            };
            json!(!standing_delivered(store, &sink))
        }
        "sink.delivered" => {
            let Some(sink) = crate::sinks::read(store, object).ok().flatten() else {
                return Some(json!(false));
            };
            // The mark is newer than every register entry routed here, and the
            // last delivery's own id is recorded on the sink
            // (`surfaces.yaml` evidence.sink.delivered).
            json!(sink.delivery.is_some() && standing_delivered(store, &sink))
        }

        // ---- curation (110, 118, `blueprints.yaml` evidence)
        "curation.threshold" => json!(field("threshold")
            .and_then(|v| v.as_u64())
            .unwrap_or(12)),
        // The cadence has fired since this curation last ran. What "last ran"
        // means is when its run region last settled, which is the object's own
        // entered_at and nothing a process remembers (75, 231).
        "curation.cadence_due" => {
            let cadence = field("cadence")
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| crate::cadence::DEFAULT.to_string());
            let since = held
                .as_ref()
                .and_then(|o| o.entered_at.get("run").copied())
                .unwrap_or(reading.now);
            json!(crate::cadence::due(&cadence, since, reading.now))
        }

        // ---- what a dependency is waiting for (31)
        //
        // "every id in get(id).depends_on has state merged" — for a unit, the
        // units of its bolt it named; for an item, the items of its unit. One
        // that names none waits for nothing (`record-derived.yaml`).
        "unit.deps_merged" | "item.deps_merged" => {
            let depends: Vec<String> = held
                .as_ref()
                .and_then(|o| o.record.get("depends_on").cloned())
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();
            json!(depends.iter().all(|id| {
                store
                    .get(id)
                    .ok()
                    .flatten()
                    .and_then(|o| o.top_state().map(String::from))
                    .is_some_and(|state| state == "merged" || state == "landed")
            }))
        }

        // ---- the work item's slot (31, 32)
        //
        // "host.running < host.bound and no ready item of an earlier ordinal on
        // this host is unplaced" (`record-derived.yaml` item.slot_free). Both
        // conjuncts: the bound is what 31 asks and the ordinal is what 32 does,
        // and an item that jumped its order would start work the operator put
        // second.
        "item.slot_free" => {
            let running = evidence(store, reading, &format!("host/{}", reading.me), "host.running")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let bound = evidence(store, reading, &format!("host/{}", reading.me), "host.bound")
                .and_then(|v| v.as_u64())
                .unwrap_or(u64::MAX);
            if running >= bound {
                return Some(json!(false));
            }
            let Some(item) = held.as_ref() else {
                return Some(json!(true));
            };
            let mine = item.record.get("ordinal").and_then(|v| v.as_u64()).unwrap_or(0);
            let Some(unit) = item.parent.clone() else {
                return Some(json!(true));
            };
            // An earlier item of the same unit that is ready and not yet placed
            // holds the slot: the order is the one the unit stated (31, 32).
            let earlier_waiting = store
                .list_records(&Scope::All)
                .unwrap_or_default()
                .into_iter()
                .filter(|o| o.machine == "work-item" && o.parent.as_deref() == Some(unit.as_str()))
                .filter(|o| o.id != item.id)
                .filter(|o| o.record.get("ordinal").and_then(|v| v.as_u64()).unwrap_or(0) < mine)
                .any(|o| o.top_state().is_some_and(|s| s == "ready"));
            json!(!earlier_waiting)
        }

        // ---- the run record (79)
        "report.recorded" => json!(true),

        // ---- the proofs of the effects whose whole work is records
        //
        // `record-derived.yaml` states each of these once, so it reads the same
        // in every profile; `crate::effects` is the act each one proves (D3,
        // task 16.1). Without them the acts fire on every tick, because a proof
        // nothing answers is a proof never found.
        //
        // "work-item records with parent = id exist, one per task of the
        // document" — phase 1 has no unit proposal document (17), so what is
        // read is that the items exist at all.
        "unit.items_exist" => json!(!crate::effects::children(store, object, "work-item")
            .unwrap_or_default()
            .is_empty()),
        // "a bolt record exists with id derived from (repository, new_name) or
        // the unit's target.bolt names an existing bolt".
        "unit.bolt_exists" => {
            let Some(unit) = held.as_ref() else {
                return Some(json!(true));
            };
            let target = unit.record.get("target").and_then(|v| v.as_object());
            let named = target
                .and_then(|t| t.get("bolt"))
                .and_then(|v| v.as_str())
                .filter(|b| !b.is_empty());
            let new_name = target
                .and_then(|t| t.get("new_name"))
                .and_then(|v| v.as_str())
                .filter(|n| !n.is_empty());
            match (named, new_name) {
                // It names a bolt: the bolt is there, or it is not.
                (Some(bolt), _) => json!(store.get(bolt).ok().flatten().is_some()),
                // It names a new bolt: the one derived from the repository and
                // that name.
                (None, Some(name)) => {
                    let repository = unit
                        .record
                        .get("repository")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default();
                    json!(store
                        .get(&format!("bolt/{repository}/{name}"))
                        .ok()
                        .flatten()
                        .is_some())
                }
                // It names neither, so there is no bolt to make.
                (None, None) => json!(true),
            }
        }
        // "get(id).target.new_name is set and target.bolt is not".
        "unit.target_is_new_bolt" => {
            let target = held
                .as_ref()
                .and_then(|u| u.record.get("target"))
                .and_then(|v| v.as_object());
            let named = target
                .and_then(|t| t.get("bolt"))
                .and_then(|v| v.as_str())
                .is_some_and(|b| !b.is_empty());
            let new_name = target
                .and_then(|t| t.get("new_name"))
                .and_then(|v| v.as_str())
                .is_some_and(|n| !n.is_empty());
            json!(new_name && !named)
        }
        // "get(id).target.bolt equals the response's argument" — the argument
        // is the name the response gave, which routing resolved to a bolt id or
        // held as the new bolt's name.
        "unit.routed" => {
            let target = held
                .as_ref()
                .and_then(|u| u.record.get("target"))
                .and_then(|v| v.as_object());
            json!(target.is_some_and(|t| {
                t.get("bolt").and_then(|v| v.as_str()).is_some_and(|b| !b.is_empty())
                    || t.get("new_name").and_then(|v| v.as_str()).is_some_and(|n| !n.is_empty())
            }))
        }
        // "get(bolt).name equals the response's argument".
        "bolt.named" => json!(held
            .as_ref()
            .and_then(|o| o.record.get("name"))
            .and_then(|v| v.as_str())
            .is_some_and(|n| !n.is_empty())),
        // "get(id).type equals the response's argument".
        "elaboration.type_set" => json!(held
            .as_ref()
            .and_then(|o| o.record.get("type"))
            .and_then(|v| v.as_str())
            .is_some_and(|t| !t.is_empty())),
        // "signals moved attach to the intent, or finding records on it, that
        // no elaboration record in state proposed or later cites".
        "intent.material_pending" => json!(!crate::effects::pending_material(store, object)
            .unwrap_or_default()
            .is_empty()),
        // "the intent has exactly one elaboration in proposed and it cites all
        // pending material".
        "intent.material_held" => {
            let proposed: Vec<_> = crate::effects::children(store, object, "elaboration")
                .unwrap_or_default()
                .into_iter()
                .filter(|e| e.top_state().is_some_and(|s| s == "proposed"))
                .collect();
            let pending = crate::effects::pending_material(store, object).unwrap_or_default();
            json!(proposed.len() == 1 && pending.is_empty())
        }
        // "two intent records name this one as split_from".
        "intent.split_done" => json!(store
            .list_records(&Scope::Machine("intent".into()))
            .unwrap_or_default()
            .iter()
            .filter(|i| i.record.get("split_from").and_then(|v| v.as_str()) == Some(object))
            .count()
            >= 2),
        // `session.offers_recorded` and `curation.gatherings_proposed` read
        // what a session delivered, so they are the sessions binding's and are
        // answered where the session id is known
        // (`flywheel-sessions-operator::evidence`, 93b).
        _ => return None,
    })
}

/// Whether this sink's mark is newer than every register entry of a decision
/// routed to it — which is what makes it delivered, and what makes it due when
/// it is not (`surfaces.yaml` evidence.sink, 14, 82).
///
/// The register's entries are its decision ids, and a decision id carries the
/// point its state was entered (`rail::decision_id`), so the "newer than the
/// mark" of 14 is read from the register and the mark alone. A sink that has
/// never delivered is behind everything standing.
fn standing_delivered<S: Records>(store: &S, sink: &crate::sinks::Sink) -> bool {
    let standing: Vec<String> = store
        .get(crate::RAIL)
        .ok()
        .flatten()
        .and_then(|o| o.record.get("standing").cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    let routed: Vec<&String> = standing
        .iter()
        .filter(|id| decision_kind(id).is_some_and(|kind| sink.routes_kind(kind)))
        .collect();
    if routed.is_empty() {
        return true;
    }
    let Some(mark) = sink.delivered_at else {
        return false;
    };
    routed
        .iter()
        .all(|id| decision_since(id).is_some_and(|since| since <= mark))
}

/// A decision id is `<object>/<kind>/<entered_at>` (`rail::decision_id`).
fn decision_parts(id: &str) -> Option<(&str, &str)> {
    let (rest, since) = id.rsplit_once('/')?;
    let (_object, kind) = rest.rsplit_once('/')?;
    Some((kind, since))
}

fn decision_kind(id: &str) -> Option<&str> {
    decision_parts(id).map(|(kind, _)| kind)
}

fn decision_since(id: &str) -> Option<DateTime<Utc>> {
    let (_kind, since) = decision_parts(id)?;
    DateTime::parse_from_rfc3339(since)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

/// The host a host object's id names.
fn host_name(object: &str) -> String {
    object.strip_prefix("host/").unwrap_or(object).to_string()
}

/// The object a lease object's id names: `lease/<object>` is the lease machine's
/// own object, and the lease itself is on what it names.
fn lease_object(object: &str) -> &str {
    object.strip_prefix("lease/").unwrap_or(object)
}

/// Every object a host holds a lease on.
fn leases_of<S: Records>(store: &S, host: &str) -> Vec<String> {
    store
        .list_records(&Scope::All)
        .unwrap_or_default()
        .iter()
        .filter(|o| {
            store
                .leases(&o.id)
                .ok()
                .flatten()
                .is_some_and(|l| l.holder == host)
        })
        .map(|o| o.id.clone())
        .collect()
}
