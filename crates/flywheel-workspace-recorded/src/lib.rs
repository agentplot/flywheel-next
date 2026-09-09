//! flywheel-workspace-recorded: `Workspace` as records (93a, D8).
//!
//! Every effect of 42 — create a line, prepare or remove a place, merge into
//! the bolt, land — is performed by writing the fact the effect's proof reads:
//! `place.exists`, `place.merged`, `place.absent`, `line.landed`. The machines
//! tick unchanged over those facts, so a bolt, a unit, a work item and a stage
//! move through their states while nothing they do reaches a repository.
//!
//! The facts live in the state store, because that is the one durable place a
//! host has (125): one record per place and per line, under the `fact/` prefix,
//! with an empty configuration, so nothing ticks them. `evidence` reads them
//! back under the atom names the proofs use, and is what a host binds as the
//! `place.*` and `line.*` half of its evidence.
//!
//! Phase 2 replaces this crate with `flywheel-workspace-host` over `wt` and
//! git; the trait, the atom names and the machines do not change.

use anyhow::Result;
use flywheel_atoms::{Endpoint, LandingPolicy, Object, Records, TakeOutcome, Workspace};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The record a place's facts are kept in.
pub fn place_fact(place: &str) -> String {
    format!("fact/place/{place}")
}

/// The record a line's facts are kept in.
pub fn line_fact(line: &str) -> String {
    format!("fact/line/{line}")
}

/// `Workspace` over any state store. The facts are the store's, so a host that
/// stops and starts again reads back what it recorded (75, I14).
pub struct RecordedWorkspace<'a, S: Records> {
    pub store: &'a mut S,
}

impl<'a, S: Records> RecordedWorkspace<'a, S> {
    pub fn new(store: &'a mut S) -> Self {
        RecordedWorkspace { store }
    }

    /// Write one fact, leaving every other field of the record alone. The
    /// record carries no state, so it is never ticked and never conflicts with
    /// an object of the same name.
    fn set(&mut self, id: &str, fields: &[(&str, Value)]) -> Result<()> {
        let held = self.store.get(id)?;
        let seq = held.as_ref().map(|o| o.seq).unwrap_or(0);
        let mut record: BTreeMap<String, Value> =
            held.map(|o| o.record).unwrap_or_default();
        for (name, value) in fields {
            record.insert((*name).to_string(), value.clone());
        }
        let object = Object {
            id: id.to_string(),
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
        self.store.put(id, &object, seq)?;
        Ok(())
    }
}

/// A fact as a record field, or none where nothing was recorded.
fn field<S: Records>(store: &S, id: &str, name: &str) -> Option<Value> {
    store.get(id).ok().flatten()?.record.get(name).cloned()
}

fn flag<S: Records>(store: &S, id: &str, name: &str, default: bool) -> bool {
    field(store, id, name)
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

/// The `place.*` and `line.*` evidence, read back from the facts the effects
/// wrote. A host binds this half of its evidence to this function; every other
/// name is the store's or the sessions binding's (`record-derived.yaml`).
pub fn evidence<S: Records>(store: &S, object: &str, name: &str) -> Option<Value> {
    let place = place_fact(object);
    let line = line_fact(object);
    Some(match name {
        "place.exists" | "place.ready" => {
            json!(flag(store, &place, "exists", false) && !flag(store, &place, "absent", false))
        }
        "place.absent" => {
            json!(flag(store, &place, "absent", false) || !flag(store, &place, "exists", false))
        }
        "place.merged" => json!(flag(store, &place, "merged", false)),
        "place.conflicted" => json!(flag(store, &place, "conflicted", false)),
        "place.endpoints_recorded" => json!(flag(store, &place, "endpoints_recorded", true)),
        "place.endpoints_served" => {
            field(store, &place, "endpoints").unwrap_or_else(|| json!([]))
        }
        "line.exists" => {
            json!(flag(store, &line, "exists", false) && !flag(store, &line, "absent", false))
        }
        "line.absent" => {
            json!(flag(store, &line, "absent", false) || !flag(store, &line, "exists", false))
        }
        "line.landed" => json!(flag(store, &line, "landed", false)),
        "line.landing" => field(store, &line, "landing").unwrap_or_else(|| json!("none")),
        "line.acceptance_written" => json!(flag(store, &line, "acceptance", false)),
        _ => return None,
    })
}

impl<S: Records> Workspace for RecordedWorkspace<'_, S> {
    fn create_line(&mut self, line: &str, parent: &str) -> Result<()> {
        self.set(
            &line_fact(line),
            &[
                ("exists", json!(true)),
                ("absent", json!(false)),
                ("parent", json!(parent)),
            ],
        )
    }

    fn take_parent(&mut self, line: &str) -> Result<TakeOutcome> {
        // Nothing is merged, so nothing can conflict: what is unproved under
        // 93a is exactly the conflict, and the scenarios asserting one carry
        // `requires: [real-workspace]`.
        self.set(&line_fact(line), &[("exists", json!(true))])?;
        Ok(TakeOutcome::Done)
    }

    fn remove_line(&mut self, line: &str) -> Result<()> {
        self.set(
            &line_fact(line),
            &[("absent", json!(true)), ("exists", json!(false))],
        )
    }

    fn land_line(&mut self, line: &str, policy: LandingPolicy) -> Result<()> {
        match policy {
            LandingPolicy::Direct => self.set(
                &line_fact(line),
                &[
                    ("exists", json!(true)),
                    ("landed", json!(true)),
                    ("landing", json!("passed")),
                ],
            ),
            LandingPolicy::PullRequest => self.set(
                &line_fact(line),
                &[("exists", json!(true)), ("landing", json!("open"))],
            ),
        }
    }

    fn write_acceptance(&mut self, line: &str, body: &str) -> Result<()> {
        self.set(
            &line_fact(line),
            &[("acceptance", json!(true)), ("acceptance_body", json!(body))],
        )
    }

    fn prepare_place(&mut self, place: &str, line: &str, work_order: &str) -> Result<()> {
        self.set(
            &place_fact(place),
            &[
                ("exists", json!(true)),
                ("absent", json!(false)),
                ("line", json!(line)),
                ("work_order", json!(work_order)),
            ],
        )
    }

    fn rebase_place(&mut self, place: &str) -> Result<TakeOutcome> {
        self.set(&place_fact(place), &[("rebased", json!(true))])?;
        Ok(TakeOutcome::Done)
    }

    fn merge_place(&mut self, place: &str) -> Result<TakeOutcome> {
        self.set(&place_fact(place), &[("merged", json!(true))])?;
        Ok(TakeOutcome::Done)
    }

    fn remove_place(&mut self, place: &str) -> Result<()> {
        self.set(
            &place_fact(place),
            &[("absent", json!(true)), ("exists", json!(false))],
        )
    }

    fn endpoints(&self, place: &str) -> Result<Vec<Endpoint>> {
        let served = field(self.store, &place_fact(place), "endpoints")
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        Ok(served
            .iter()
            .filter_map(|v| v.as_str())
            .map(|url| Endpoint {
                name: place.to_string(),
                url: url.to_string(),
            })
            .collect())
    }
}
