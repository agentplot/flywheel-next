//! A `StateStore` that behaves, for the tests that need a store and not a
//! repository (D17).
//!
//! This is a **fake**, not a mock: it holds objects, threads, responses and
//! leases in maps and answers the six record operations and the eight store
//! operations the way any store must — a `put` against a stale sequence is
//! rejected, an effect written twice is written once, a lease another host
//! holds is refused, a read names the point it is as of. Nothing here is told
//! what to expect and nothing records calls.
//!
//! It is not a profile. It is never named by `--profile`, never bound in a
//! `profiles/` file and never present in the acceptance set: what 92 retired
//! was a second store *profile* claiming conformance, and this claims nothing.
//! It exists so `flywheel-domain` and the projections — which take the trait,
//! not the repository — can be tested in the first tier, in milliseconds.
//!
//! Behind the `testing` feature, and a dev-dependency of the crates that use it.

use crate::traits::{
    Cost, EffectWrite, EvidenceRead, HostRecord, LeaseOp, LeaseOutcome, LeaseRecord, Listing,
    Notice, Presentation, PutOutcome, ReadPoint, Received, Records, Scope, StateStore, StatusView,
    ThreadEntry, WriteOutcome,
};
use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use flywheel_engine::runtime::Response;
use flywheel_engine::Object;
use std::collections::BTreeMap;

/// The point a fake store's clock starts at, so two runs of one test are one
/// run: the same fixed point the conformance runner uses (D15).
pub fn start_of_time() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0)
        .single()
        .expect("a fixed point")
}

/// Objects, threads, responses and leases in maps, behind the store's own
/// doors.
#[derive(Debug, Clone)]
pub struct FakeStore {
    objects: BTreeMap<String, Object>,
    threads: BTreeMap<String, Vec<ThreadEntry>>,
    responses: Vec<Response>,
    leases: BTreeMap<String, LeaseRecord>,
    hosts: BTreeMap<String, HostRecord>,
    effects: Vec<String>,
    presented: Vec<Presentation>,
    /// Which host this store answers as, so a lease another holds is refused.
    host: String,
    /// The write sequence: the point a read is as of (126).
    writes: u64,
    /// `(write sequence, object)` for every write, so a notice names what
    /// moved (130).
    moved: Vec<(u64, String)>,
    now: DateTime<Utc>,
    status: String,
    /// What the world reports about an object, by name; `*` says it of every
    /// object. The same door the git store answers evidence through (B.3).
    given: BTreeMap<String, BTreeMap<String, serde_json::Value>>,
}

impl Default for FakeStore {
    fn default() -> Self {
        FakeStore::new("local")
    }
}

impl FakeStore {
    pub fn new(host: &str) -> FakeStore {
        FakeStore {
            objects: BTreeMap::new(),
            threads: BTreeMap::new(),
            responses: vec![],
            leases: BTreeMap::new(),
            hosts: BTreeMap::new(),
            effects: vec![],
            presented: vec![],
            host: host.to_string(),
            writes: 0,
            moved: vec![],
            now: start_of_time(),
            status: String::new(),
            given: BTreeMap::new(),
        }
    }

    /// Put an object in place without going through `put`: what a test
    /// describes as already true before it begins.
    pub fn seed(&mut self, object: Object) -> &mut Self {
        self.objects.insert(object.id.clone(), object);
        self
    }

    /// The same for a lease a test describes as held.
    pub fn seed_lease(&mut self, object: &str, holder: &str) -> &mut Self {
        self.leases.insert(
            object.to_string(),
            LeaseRecord {
                object: object.to_string(),
                holder: holder.to_string(),
                taken_at: self.now,
                renewed_at: self.now,
                state: "held".into(),
            },
        );
        self
    }

    pub fn seed_host(&mut self, host: &str, bound: u32) -> &mut Self {
        self.hosts.insert(
            host.to_string(),
            HostRecord {
                host: host.to_string(),
                last_seen: self.now,
                bound,
                intermittent: false,
            },
        );
        self
    }

    /// What the world reports about one object, or about every object under
    /// `*`. A fake world is still a world: the engine reads it the same way.
    pub fn given(&mut self, object: &str, name: &str, value: serde_json::Value) -> &mut Self {
        self.given
            .entry(object.to_string())
            .or_default()
            .insert(name.to_string(), value);
        self
    }

    pub fn set_now(&mut self, now: DateTime<Utc>) -> &mut Self {
        self.now = now;
        self
    }

    pub fn now(&self) -> DateTime<Utc> {
        self.now
    }

    /// What this store has been asked to present, in order, for a test about
    /// delivery (129).
    pub fn presentations(&self) -> &[Presentation] {
        &self.presented
    }

    /// The effect ids written, in order (127).
    pub fn effects_written(&self) -> &[String] {
        &self.effects
    }

    pub fn set_status(&mut self, body: &str) -> &mut Self {
        self.status = body.to_string();
        self
    }

    fn point(&self) -> ReadPoint {
        ReadPoint {
            mark: format!("write {}", self.writes),
            seq: self.writes,
            at: self.now,
        }
    }

    fn in_scope(&self, object: &Object, scope: &Scope) -> bool {
        match scope {
            Scope::All => true,
            Scope::Machine(machine) => object.machine == *machine,
            Scope::Under(id) => {
                object.id == *id
                    || object.id.starts_with(&format!("{id}/"))
                    || object.parent.as_deref() == Some(id.as_str())
            }
        }
    }
}

impl Records for FakeStore {
    fn get(&self, id: &str) -> Result<Option<Object>> {
        Ok(self.objects.get(id).cloned())
    }

    fn put(&mut self, id: &str, record: &Object, base_seq: u64) -> Result<PutOutcome> {
        // A write against a sequence the store has moved past is a lost race:
        // the loser is told what the store holds and reads again (134).
        let held = self.objects.get(id).map(|o| o.seq).unwrap_or(0);
        if held != base_seq {
            return Ok(PutOutcome::Rejected { held_seq: held });
        }
        self.writes += 1;
        let mut written = record.clone();
        written.id = id.to_string();
        written.seq = held + 1;
        self.objects.insert(id.to_string(), written);
        self.moved.push((self.writes, id.to_string()));
        Ok(PutOutcome::Written { seq: held + 1 })
    }

    fn append(&mut self, id: &str, entry: &ThreadEntry) -> Result<()> {
        self.threads
            .entry(id.to_string())
            .or_default()
            .push(entry.clone());
        Ok(())
    }

    fn list_records(&self, scope: &Scope) -> Result<Vec<Object>> {
        Ok(self
            .objects
            .values()
            .filter(|o| self.in_scope(o, scope))
            .cloned()
            .collect())
    }

    fn thread(&self, id: &str) -> Result<Vec<ThreadEntry>> {
        Ok(self.threads.get(id).cloned().unwrap_or_default())
    }

    fn responses(&self, id: &str) -> Result<Vec<Response>> {
        // The responses in hand, or those this object's own record and register
        // entries name (`record-derived.yaml`): the rail holds every one.
        let mut out: Vec<Response> = self
            .responses
            .iter()
            .filter(|r| id == "rail" || r.object.as_deref() == Some(id))
            .cloned()
            .collect();
        out.sort_by(|a, b| a.given_at.cmp(&b.given_at).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    fn leases(&self, id: &str) -> Result<Option<LeaseRecord>> {
        Ok(self.leases.get(id).cloned())
    }

    fn hosts(&self) -> Result<Vec<HostRecord>> {
        Ok(self.hosts.values().cloned().collect())
    }
}

impl StateStore for FakeStore {
    fn cost(&self) -> Cost {
        // A store with no repository behind it spends nothing external, and
        // says so rather than pretending to a number (169).
        Cost::default()
    }

    fn read(&self, id: &str) -> Result<EvidenceRead> {
        let object = self.objects.get(id).cloned();
        let mut evidence: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        if let Some(held) = &object {
            evidence.insert("state".into(), serde_json::to_value(&held.config)?);
            evidence.insert("entered_at".into(), serde_json::to_value(&held.entered_at)?);
            evidence.insert("seq".into(), serde_json::json!(held.seq));
            evidence.insert(
                "applied_responses".into(),
                serde_json::to_value(&held.applied_responses)?,
            );
        }
        Ok(EvidenceRead {
            object,
            evidence,
            as_of: self.point(),
        })
    }

    fn list(&self, scope: &Scope) -> Result<Listing> {
        Ok(Listing {
            objects: self.list_records(scope)?,
            as_of: self.point(),
        })
    }

    fn status(&self) -> Result<StatusView> {
        Ok(StatusView {
            as_of: self.point(),
            body: self.status.clone(),
        })
    }

    fn write_effect(&mut self, write: &EffectWrite) -> Result<WriteOutcome> {
        // An effect written twice is written once: the identity is the whole
        // of it (127).
        if self.effects.contains(&write.effect_id) {
            return Ok(WriteOutcome::AlreadyWritten {
                effect_id: write.effect_id.clone(),
            });
        }
        self.effects.push(write.effect_id.clone());
        self.writes += 1;
        self.moved.push((self.writes, write.object.clone()));
        Ok(WriteOutcome::Written {
            effect_id: write.effect_id.clone(),
        })
    }

    fn lease(&mut self, op: &LeaseOp) -> Result<LeaseOutcome> {
        match op {
            LeaseOp::Take { object, holder } => {
                if let Some(held) = self.leases.get(object) {
                    if !held.holder.is_empty() && held.holder != *holder {
                        return Ok(LeaseOutcome::HeldByAnother(held.clone()));
                    }
                }
                let taken_at = self
                    .leases
                    .get(object)
                    .filter(|h| h.holder == *holder)
                    .map(|h| h.taken_at)
                    .unwrap_or(self.now);
                let state = self
                    .leases
                    .get(object)
                    .map(|h| h.state.clone())
                    .unwrap_or_else(crate::traits::free);
                let record = LeaseRecord {
                    object: object.clone(),
                    holder: holder.clone(),
                    taken_at,
                    renewed_at: self.now,
                    state,
                };
                self.leases.insert(object.clone(), record.clone());
                Ok(LeaseOutcome::Held(record))
            }
            LeaseOp::Renew { object, holder } => match self.leases.get(object).cloned() {
                Some(held) if held.holder == *holder => {
                    let record = LeaseRecord {
                        renewed_at: self.now,
                        ..held
                    };
                    self.leases.insert(object.clone(), record.clone());
                    Ok(LeaseOutcome::Held(record))
                }
                Some(held) => Ok(LeaseOutcome::HeldByAnother(held)),
                None => anyhow::bail!("no lease on {object} to renew"),
            },
            LeaseOp::Release { object, holder } => {
                match self.leases.get(object).map(|h| h.holder == *holder) {
                    Some(false) => Ok(LeaseOutcome::HeldByAnother(
                        self.leases.get(object).cloned().expect("held"),
                    )),
                    _ => {
                        self.leases.remove(object);
                        Ok(LeaseOutcome::Released)
                    }
                }
            }
            LeaseOp::Mark { object, state } => match self.leases.get(object).cloned() {
                Some(held) => {
                    let record = LeaseRecord {
                        state: state.clone(),
                        ..held
                    };
                    self.leases.insert(object.clone(), record.clone());
                    Ok(LeaseOutcome::Held(record))
                }
                None => Ok(LeaseOutcome::Released),
            },
        }
    }

    fn notify(&self, since: &ReadPoint) -> Result<Notice> {
        let mut objects: Vec<String> = self
            .moved
            .iter()
            .filter(|(seq, _)| *seq > since.seq)
            .map(|(_, id)| id.clone())
            .collect();
        objects.sort();
        objects.dedup();
        Ok(Notice {
            as_of: self.point(),
            objects,
        })
    }

    fn present(&mut self, presentation: &Presentation) -> Result<()> {
        self.presented.push(presentation.clone());
        Ok(())
    }

    fn receive(&mut self, response: &Response) -> Result<Received> {
        // A response is kept under its own id, so the same delivery arriving
        // twice writes the same record and takes effect once (137).
        if self.responses.iter().any(|r| r.id == response.id) {
            return Ok(Received::AlreadyApplied {
                id: response.id.clone(),
            });
        }
        self.responses.push(response.clone());
        Ok(Received::Recorded {
            id: response.id.clone(),
        })
    }
}

/// The host this store answers as, for a test about two of them.
impl FakeStore {
    pub fn me(&self) -> &str {
        &self.host
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(id: &str, machine: &str) -> Object {
        Object {
            id: id.into(),
            machine: machine.into(),
            parent: None,
            config: Default::default(),
            entered_at: Default::default(),
            record: Default::default(),
            counters: Default::default(),
            applied_responses: vec![],
            seq: 0,
            created: 0,
        }
    }

    #[test]
    fn a_put_against_a_stale_sequence_is_rejected() {
        let mut store = FakeStore::default();
        let held = object("bolt/atlas/plan-rows", "bolt");
        assert_eq!(
            store.put(&held.id, &held, 0).expect("the first write"),
            PutOutcome::Written { seq: 1 }
        );
        // The loser carries the sequence it read and is told what the store
        // holds (134).
        assert_eq!(
            store.put(&held.id, &held, 0).expect("the second"),
            PutOutcome::Rejected { held_seq: 1 }
        );
    }

    #[test]
    fn an_effect_written_twice_is_written_once() {
        let mut store = FakeStore::default();
        let write = EffectWrite {
            effect_id: "bolt/atlas/plan-rows/create_line/line.exists/abc".into(),
            object: "bolt/atlas/plan-rows".into(),
            effect: "create_line".into(),
            reason: "the line is made once".into(),
            evidence: Default::default(),
        };
        assert!(matches!(
            store.write_effect(&write).expect("the write"),
            WriteOutcome::Written { .. }
        ));
        assert!(matches!(
            store.write_effect(&write).expect("the repeat"),
            WriteOutcome::AlreadyWritten { .. }
        ));
        assert_eq!(store.effects_written().len(), 1);
    }

    #[test]
    fn a_lease_another_host_holds_is_refused() {
        let mut store = FakeStore::default();
        store.seed_lease("unit/atlas/status-writer", "studio");
        let outcome = store
            .lease(&LeaseOp::Take {
                object: "unit/atlas/status-writer".into(),
                holder: "mac-mini".into(),
            })
            .expect("the take");
        match outcome {
            LeaseOutcome::HeldByAnother(held) => assert_eq!(held.holder, "studio"),
            other => panic!("the loser was told it held it: {other:?}"),
        }
    }

    #[test]
    fn a_notice_names_what_moved_since_a_point() {
        let mut store = FakeStore::default();
        let before = store.list(&Scope::All).expect("a point").as_of;
        let held = object("bolt/atlas/plan-rows", "bolt");
        store.put(&held.id, &held, 0).expect("a write");
        let notice = store.notify(&before).expect("the notice");
        assert_eq!(notice.objects, vec!["bolt/atlas/plan-rows".to_string()]);
        // And nothing moved since the notice itself.
        assert!(store
            .notify(&notice.as_of)
            .expect("the second notice")
            .objects
            .is_empty());
    }

    #[test]
    fn a_scope_is_the_objects_it_names() {
        let mut store = FakeStore::default();
        store.seed(object("bolt/atlas/plan-rows", "bolt"));
        store.seed(object("unit/atlas/status-writer", "unit"));
        store.seed(object("bolt/atlas/plan-rows/wi-1", "work-item"));
        assert_eq!(store.list_records(&Scope::All).expect("all").len(), 3);
        assert_eq!(
            store
                .list_records(&Scope::Machine("unit".into()))
                .expect("one machine")
                .len(),
            1
        );
        assert_eq!(
            store
                .list_records(&Scope::Under("bolt/atlas/plan-rows".into()))
                .expect("one object and what it owns")
                .len(),
            2
        );
    }
}

impl flywheel_engine::runtime::EvidenceSource for FakeStore {
    fn evidence(&self, object: &str, _region: &str, name: &str) -> Option<serde_json::Value> {
        self.given
            .get(object)
            .and_then(|held| held.get(name))
            .or_else(|| self.given.get("*").and_then(|held| held.get(name)))
            .cloned()
    }
}
