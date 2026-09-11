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

    /// The places of the same line as this one, each one a sibling of it (38,
    /// 51).
    fn siblings_of(&self, place: &str) -> Result<Vec<String>> {
        let line = field(self.store, &place_fact(place), "line")
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        if line.is_empty() {
            return Ok(vec![]);
        }
        Ok(self
            .store
            .list_records(&flywheel_atoms::Scope::All)?
            .into_iter()
            .filter_map(|o| o.id.strip_prefix("fact/place/").map(String::from))
            .filter(|other| other != place)
            .filter(|other| {
                field(self.store, &place_fact(other), "line")
                    .and_then(|v| v.as_str().map(String::from))
                    .as_deref()
                    == Some(line.as_str())
            })
            .collect())
    }

    /// The newest sequence any object carries: what a take is recorded as of,
    /// so a parent that moved after it is one this line does not contain (50).
    fn newest_seq(&self) -> Result<u64> {
        Ok(self
            .store
            .list_records(&flywheel_atoms::Scope::All)?
            .iter()
            .map(|o| o.seq)
            .max()
            .unwrap_or(0))
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

        // ---- the ancestry the machines gate on (50, 51, `host.yaml`)
        //
        // On a real workspace these are `git merge-base --is-ancestor`; here
        // they are the fact the effect that made them true wrote, which is what
        // 93a admits. A place that was never prepared contains no line, so a
        // place stays in `preparing` until `prepare_place` has run — which is
        // the machine's own rule and not a default.
        "place.contains_line" => json!(flag(store, &place, "contains_line", false)),
        // A line with no parent contains its parent vacuously; one with a
        // parent contains it once `take_parent` has run and until the parent
        // moved again (50, `line.yaml`).
        "line.contains_parent" => {
            let parent = field(store, &line, "parent_line")
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default();
            match parent.is_empty() {
                true => json!(true),
                false => json!(flag(store, &line, "contains_parent", false)),
            }
        }
        // "no sibling place of the same line with a lower ordinal is in state
        // merging" (38, `host.yaml` place.merge_slot). One at a time, in a
        // fixed order, is the whole of what 38 asks.
        "place.merge_slot" => json!(merge_slot(store, object)),
        // "the line has no place yet and one is about to be made; or the line
        // is landing; or the repository's take cadence has fired; or a
        // dictation `take <line>` stands unapplied" (50, `host.yaml`).
        // A manifest of this release names no take cadence, so what is read is
        // the first two and the dictation.
        "line.take_due" => {
            let landing = field(store, &line, "landing")
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| "none".into());
            let has_place = store
                .list_records(&flywheel_atoms::Scope::All)
                .unwrap_or_default()
                .iter()
                .any(|o| {
                    o.id.starts_with("fact/place/")
                        && o.record.get("line").and_then(|v| v.as_str()) == Some(object)
                });
            json!(!has_place || landing != "none" || flag(store, &line, "take_asked", false))
        }
        _ => return None,
    })
}

/// Whether this place may merge now: no sibling place of the same line with a
/// lower ordinal is merging (38).
///
/// The ordinal is the place's own record where it carries one and its id
/// otherwise, so the order is fixed and the same on every host.
fn merge_slot<S: Records>(store: &S, place: &str) -> bool {
    let fact = place_fact(place);
    let Some(line) = field(store, &fact, "line").and_then(|v| v.as_str().map(String::from)) else {
        return true;
    };
    let mine = ordinal_of(store, place);
    !store
        .list_records(&flywheel_atoms::Scope::All)
        .unwrap_or_default()
        .iter()
        .filter_map(|o| o.id.strip_prefix("fact/place/"))
        .filter(|other| *other != place)
        .filter(|other| {
            field(store, &place_fact(other), "line").and_then(|v| v.as_str().map(String::from))
                == Some(line.clone())
        })
        .any(|other| {
            ordinal_of(store, other) < mine
                && store
                    .get(other)
                    .ok()
                    .flatten()
                    .and_then(|o| o.top_state().map(String::from))
                    .as_deref()
                    == Some("merging")
        })
}

/// A place's place in the fixed order (38): its `ordinal` where it has one, and
/// its id otherwise, which orders it the same way on every host.
fn ordinal_of<S: Records>(store: &S, place: &str) -> (u64, String) {
    let ordinal = store
        .get(place)
        .ok()
        .flatten()
        .and_then(|o| o.record.get("ordinal").and_then(|v| v.as_u64()))
        .or_else(|| {
            field(store, &place_fact(place), "ordinal").and_then(|v| v.as_u64())
        })
        .unwrap_or(u64::MAX);
    (ordinal, place.to_string())
}

impl<S: Records> Workspace for RecordedWorkspace<'_, S> {
    fn create_line(&mut self, line: &str, parent: &str) -> Result<()> {
        self.set(
            &line_fact(line),
            &[
                ("exists", json!(true)),
                ("absent", json!(false)),
                // Not `parent`: that name is the envelope's own, for the
                // object's parent, and a record field of that name does not
                // survive the round trip (`envelope.rs` RESERVED).
                ("parent_line", json!(parent)),
            ],
        )
    }

    fn take_parent(&mut self, line: &str) -> Result<TakeOutcome> {
        // Nothing is merged, so nothing can conflict: what is unproved under
        // 93a is exactly the conflict, and the scenarios asserting one carry
        // `requires: [real-workspace]`. What the take does record is that the
        // parent is in: the same statement `git merge-base --is-ancestor`
        // makes on a real line (`host.yaml` line.contains_parent, 50).
        self.set(
            &line_fact(line),
            &[
                ("exists", json!(true)),
                ("contains_parent", json!(true)),
                ("taken_at_seq", json!(self.newest_seq()?)),
            ],
        )?;
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
        // A place just made from its line contains it: that is what lets the
        // place leave `preparing`, and on a real workspace it is the same
        // statement `git merge-base --is-ancestor` makes (`place.yaml`
        // preparing, `host.yaml` place.contains_line).
        self.set(
            &place_fact(place),
            &[
                ("exists", json!(true)),
                ("absent", json!(false)),
                ("line", json!(line)),
                ("contains_line", json!(true)),
                ("work_order", json!(work_order)),
            ],
        )
    }

    fn rebase_place(&mut self, place: &str) -> Result<TakeOutcome> {
        // Rebased onto the line, so it contains it again (51).
        self.set(
            &place_fact(place),
            &[("rebased", json!(true)), ("contains_line", json!(true))],
        )?;
        Ok(TakeOutcome::Done)
    }

    fn merge_place(&mut self, place: &str) -> Result<TakeOutcome> {
        self.set(&place_fact(place), &[("merged", json!(true))])?;
        // The line moved under every sibling place of it, so none of them
        // contains it any more: that is what puts them in `behind` and what
        // makes the rebase of 51 happen at all (`place.yaml` ready → behind).
        for sibling in self.siblings_of(place)? {
            self.set(&place_fact(&sibling), &[("contains_line", json!(false))])?;
        }
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

#[cfg(test)]
mod tests;
