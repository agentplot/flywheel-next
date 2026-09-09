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
        // A sink's delivery newer than the response carried it under attention;
        // the sinks are group 8, and until one runs nothing has been reported.
        "response.reported" => json!(false),

        // ---- the run record (79)
        "report.recorded" => json!(true),
        _ => return None,
    })
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
