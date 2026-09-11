//! The runner's store: the six record operations and the eight operations of
//! 125, every one of them through the state repository the run bound (92, 160).
//!
//! What the maps here hold is the runner's reading of that repository — the
//! objects as the last fetch found them, the register the rail record carries,
//! the responses in hand — kept in step with every write so an assertion and
//! the world's derived evidence read one thing. No operation is answered from
//! them where the repository is the answer.

use crate::store::{Durable, EffectRecord, PresentedRecord, Store};
use anyhow::{anyhow, Result};
use flywheel_atoms::{
    EffectWrite, EvidenceRead, HostRecord, LeaseOp, LeaseOutcome, LeaseRecord, Listing, Notice,
    Object, Presentation, PutOutcome, ReadPoint, Received, Records, Scope, StateStore, StatusView,
    ThreadEntry, WriteOutcome,
};
use flywheel_engine::runtime::{EvidenceSource, Response};
use std::collections::BTreeMap;

impl Store {
    /// Who is writing: the acting host a `host` step named, or `local` (232).
    pub fn me(&self) -> String {
        self.acting_host.clone().unwrap_or_else(|| "local".to_string())
    }

    /// A host that cannot reach the store keeps ticking what it holds and
    /// commits locally; its writes are intentions until it reconnects (151).
    pub fn is_disconnected(&self) -> bool {
        self.disconnected.contains(&self.me())
    }

    /// The point a read is as of.
    pub fn as_of(&self) -> ReadPoint {
        ReadPoint {
            mark: format!("w{}", self.writes),
            seq: self.writes,
            at: self.now,
        }
    }

    /// Record that an object moved, and name the point it moved at.
    fn moved_at(&mut self, id: &str) -> u64 {
        self.writes += 1;
        self.moved.push((self.writes, id.to_string()));
        self.writes
    }

    /// The same, for a write of the work: the machinery's own objects move the
    /// line without moving this count (D5, 167).
    fn moved_at_by_machine(&mut self, id: &str, machine: &str) -> u64 {
        let seq = self.moved_at(id);
        if !flywheel_domain::leases::machinery(machine) {
            self.work_writes += 1;
        }
        seq
    }

    /// The first top-level region of an object; the region every evidence read
    /// without one is asked against.
    fn top_region(&self, id: &str) -> String {
        self.objects
            .get(id)
            .and_then(|o| o.config.keys().find(|k| !k.contains('.')).cloned())
            .unwrap_or_else(|| "life".to_string())
    }

    fn in_scope(object: &Object, scope: &Scope) -> bool {
        match scope {
            Scope::All => true,
            Scope::Machine(m) => &object.machine == m,
            Scope::Under(id) => {
                &object.id == id || object.parent.as_deref() == Some(id.as_str())
            }
        }
    }
}

/// The rail's own record id. The register — decision id to number, and the
/// counter — is the rail record, reachable through `get` like any other
/// (`profiles/record-derived.yaml`), so nothing needs a seventh operation to
/// read or write it.
pub const RAIL: &str = "rail";

impl Store {
    /// The rail record: the register and the decisions standing after the last
    /// derive, as an object.
    fn rail_record(&self) -> Object {
        let mut record: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        record.insert("next".into(), serde_json::json!(self.register.next_number));
        record.insert("register".into(), serde_json::json!(self.register.entries));
        record.insert("numbers".into(), serde_json::json!(self.register.numbers()));
        record.insert("standing".into(), serde_json::json!(self.standing));
        record.insert("status_as_of".into(), serde_json::json!(self.status_as_of));
        Object {
            id: RAIL.to_string(),
            machine: "rail".into(),
            parent: None,
            config: self.rail_config.clone(),
            entered_at: self.rail_entered.clone(),
            record,
            counters: Default::default(),
            applied_responses: vec![],
            seq: self.writes,
            created: 0,
        }
    }

    /// The decisions standing on the store as it is now, derived from the
    /// objects and the register alone. The rail's own decisions and the
    /// leases' stand beside the work's, because a lease under attention is a
    /// decision like any other (D5, 149).
    pub fn standing_now(&self) -> Vec<flywheel_engine::DecisionInstance> {
        let Some(defs) = self.defs.clone() else { return vec![] };
        let mut objects = self.objects.clone();
        let now = self.now;
        let _ = flywheel_domain::leases::attach(self, &mut objects, now);
        let _ = flywheel_domain::rail::attach(self, &defs, &mut objects, now);
        flywheel_engine::rail::derive(&defs, &objects, &self.register)
    }

    /// Give every standing decision without an entry the next number, in one
    /// write of the register and the counter. This is `number_decisions`: a
    /// number is given once and never reused, and a decision state left and
    /// re-entered carries a different id and so takes a new number (15).
    pub fn number_decisions(&mut self) {
        let standing = self.standing_now();
        if !self.register.number_all(&standing) {
            return;
        }
        self.write_rail_record();
    }

    /// Mark every entry whose decision no longer stands as retracted, on the
    /// same record the numbers live on. A decision exists exactly while its
    /// state is active (9, I3).
    pub fn retract_gone(&mut self) -> Vec<String> {
        let standing: Vec<String> = self.standing_now().into_iter().map(|d| d.id).collect();
        let now = self.now;
        let retracted = self.register.retract_gone(&standing, now);
        if !retracted.is_empty() {
            self.write_rail_record();
        }
        retracted
    }

    /// The register and the counter, written back as one record (15, 148).
    fn write_rail_record(&mut self) {
        let record = self.rail_record();
        let seq = record.seq;
        let _ = Records::put(self, RAIL, &record, seq);
    }

    /// Take a written rail record back into the register it projects.
    fn set_rail_record(&mut self, record: &Object) {
        self.rail_config = record.config.clone();
        self.rail_entered = record.entered_at.clone();
        if let Some(n) = record.record.get("next").and_then(|v| v.as_u64()) {
            self.register.next_number = n as u32;
        }
        if let Some(entries) = record.record.get("register") {
            if let Ok(entries) = serde_json::from_value(entries.clone()) {
                self.register.entries = entries;
            }
        }
        if let Some(standing) = record.record.get("standing") {
            if let Ok(standing) = serde_json::from_value(standing.clone()) {
                self.standing = standing;
            }
        }
    }
}

impl Store {
    /// The acting host's checkout of the state repository.
    pub fn durable(&self) -> Option<Durable> {
        if self.durable.is_empty() {
            return None;
        }
        self.durable
            .get(&self.me())
            .or_else(|| self.durable.values().next())
            .cloned()
    }

    /// The same, refusing where no state repository stands behind the store.
    /// Every record operation goes through one: this release binds git-only and
    /// no other (92, 125, 160).
    pub fn state_repository(&self) -> Result<Durable> {
        self.durable().ok_or_else(|| {
            anyhow!(
                "no state repository is bound: every record operation goes through one, \
                 and this release binds the git-only profile alone (92, 125)"
            )
        })
    }

    /// Read a thread back from the repository after something outside this
    /// process wrote it — the reporting command a scripted session runs is a
    /// subprocess, and what it wrote is a fact on the shared line, not a
    /// message back (67, 93).
    pub fn reread_thread(&mut self, id: &str) -> Result<()> {
        let durable = self.state_repository()?;
        let entries = {
            let mut git = durable
                .lock()
                .map_err(|_| anyhow!("the state repository is poisoned"))?;
            git.fetch()?;
            Records::thread(&*git, id)?
        };
        self.threads.insert(id.to_string(), entries);
        let _ = self.moved_at(id);
        Ok(())
    }

    /// Read the objects the bound store holds into the map the world's derived
    /// evidence and the rail are read from. A restart drops this and reads it
    /// again: nothing the engine decides on is held anywhere else (75, 131).
    pub fn refresh(&mut self) {
        self.reread(true)
    }

    /// The same without bringing the shared line up to date: what a pass does,
    /// because the tick fetched once before it decided anything (165, 169).
    pub fn reread(&mut self, fetch: bool) {
        let Some(durable) = self.durable() else { return };
        let now = self.now;
        let Ok(mut git) = durable.lock() else { return };
        git.now = now;
        if fetch {
            let _ = git.fetch();
        }
        let Ok(objects) = Records::list_records(&*git, &Scope::All) else { return };
        // The leases and the heartbeats are branches, not files on the shared
        // line (D5), so they are read back beside the objects: a reader that
        // took the objects and kept a remembered lease would be deciding on
        // something it holds in memory (128, 136, 163, I14).
        let leases: Vec<(String, LeaseRecord)> = objects
            .iter()
            .filter_map(|o| {
                Records::leases(&*git, &o.id)
                    .ok()
                    .flatten()
                    .map(|l| (o.id.clone(), l))
            })
            .collect();
        let heartbeats = Records::hosts(&*git).unwrap_or_default();
        drop(git);
        self.objects = objects.into_iter().map(|o| (o.id.clone(), o)).collect();
        if !leases.is_empty() {
            self.leases = leases.into_iter().collect();
        }
        if !heartbeats.is_empty() {
            self.heartbeats = heartbeats
                .into_iter()
                .map(|h| (h.host.clone(), h))
                .collect();
        }
        // The register is the rail's record and is read back like anything
        // else: the numbers a host gave are on the shared line, and a reader
        // that took the objects and left the register behind would be deciding
        // on something it holds in memory (15, 75, 131, 148, I14).
        if let Some(rail) = self.objects.get(RAIL).cloned() {
            self.set_rail_record(&rail);
        }
    }
}

impl Records for Store {
    fn get(&self, id: &str) -> Result<Option<Object>> {
        self.deciding.served("read");
        if id == RAIL {
            return Ok(Some(self.rail_record()));
        }
        // A lease object is made from the lease record, wherever it is kept.
        if let Some(object) = flywheel_domain::leases::object_of(id) {
            let held = self.leases.get(object);
            return Ok(Some(flywheel_domain::leases::as_object(
                object, held, self.now,
            )));
        }
        self.state_repository()?
            .lock()
            .map_err(|_| anyhow!("the state repository is poisoned"))?
            .get(id)
    }

    fn put(&mut self, id: &str, record: &Object, base_seq: u64) -> Result<PutOutcome> {
        // A lease is a branch and never a file on the shared line (D5), so the
        // state its machine reached is marked on the lease and never put.
        if let Some(object) = flywheel_domain::leases::object_of(id) {
            let state = record.config.get("hold").cloned().unwrap_or_default();
            self.lease(&LeaseOp::Mark {
                object: object.to_string(),
                state,
            })?;
            // A lease is a branch and never a file on the shared line (D5,
            // 167), so marking one does not advance the line's write sequence:
            // a run in which only leases moved wrote nothing (78).
            return Ok(PutOutcome::Written { seq: self.writes });
        }
        if id == RAIL {
            self.set_rail_record(record);
            let seq = self.moved_at(id);
            return Ok(PutOutcome::Written { seq });
        }
        // One commit on the shared line, written with expected-old; the map the
        // world reads is the same write, kept in step (D4).
        let fresh = !self.objects.contains_key(id);
        let outcome = self
            .state_repository()?
            .lock()
            .map_err(|_| anyhow!("the state repository is poisoned"))?
            .put(id, record, base_seq)?;
        if let PutOutcome::Written { seq } = outcome {
            let mut next = record.clone();
            next.seq = seq;
            self.objects.insert(id.to_string(), next);
            let _ = self.moved_at_by_machine(id, &record.machine);
            // An object this store had not seen is one the pass made. The
            // ordinal itself is the repository's, on its record; the count is
            // what says a pass created something, and a tick that created
            // something has not settled — its regions and the rail's register
            // have still to be decided upon (D7).
            if fresh {
                self.next_created += 1;
            }
        }
        Ok(outcome)
    }

    fn append(&mut self, id: &str, entry: &ThreadEntry) -> Result<()> {
        self.state_repository()?
            .lock()
            .map_err(|_| anyhow!("the state repository is poisoned"))?
            .append(id, entry)?;
        self.threads
            .entry(id.to_string())
            .or_default()
            .push(entry.clone());
        let _ = self.moved_at(id);
        Ok(())
    }

    fn list_records(&self, scope: &Scope) -> Result<Vec<Object>> {
        self.deciding.served("list");
        Ok(self
            .objects
            .values()
            .filter(|o| Store::in_scope(o, scope))
            .cloned()
            .collect())
    }

    fn thread(&self, id: &str) -> Result<Vec<ThreadEntry>> {
        Ok(self.threads.get(id).cloned().unwrap_or_default())
    }

    fn responses(&self, id: &str) -> Result<Vec<Response>> {
        self.deciding.served("read");
        // Responses arrive at the rail, so the rail's responses are the ones in
        // hand — which is what a tick reads before it decides anything.
        if id == RAIL {
            return Ok(self.responses.clone());
        }
        let numbers: Vec<u32> = self
            .register
            .entries
            .iter()
            .filter(|(decision, _)| decision.starts_with(&format!("{id}/")))
            .map(|(_, e)| e.number)
            .collect();
        Ok(self
            .responses
            .iter()
            .filter(|r| {
                r.object.as_deref() == Some(id)
                    || r.decision.is_some_and(|n| numbers.contains(&n))
            })
            .cloned()
            .collect())
    }

    fn leases(&self, id: &str) -> Result<Option<LeaseRecord>> {
        // Reading the lease record is a read; `lease` is the compare-and-swap
        // that takes, renews and releases one (125, `record-derived.yaml`).
        self.deciding.served("read");
        self.state_repository()?
            .lock()
            .map_err(|_| anyhow!("the state repository is poisoned"))?
            .leases(id)
    }

    fn hosts(&self) -> Result<Vec<HostRecord>> {
        self.deciding.served("read");
        Ok(self.heartbeats.values().cloned().collect())
    }
}

impl StateStore for Store {
    /// A tick's cost is the state repository's; the runner's own reading of it
    /// spends nothing (169, `git-only.yaml` cost).
    fn begin_tick(&mut self) {
        if let Some(durable) = self.durable() {
            if let Ok(mut git) = durable.lock() {
                git.begin_tick();
            }
        }
    }

    fn cost(&self) -> flywheel_atoms::Cost {
        self.durable()
            .and_then(|durable| durable.lock().ok().map(|git| git.cost()))
            .unwrap_or_default()
    }

    fn end_tick(&mut self) -> Result<()> {
        let durable = self.state_repository()?;
        let mut git = durable
            .lock()
            .map_err(|_| anyhow!("the state repository is poisoned"))?;
        git.end_tick()
    }

    fn read(&self, id: &str) -> Result<EvidenceRead> {
        self.deciding.served("read");
        let region = self.top_region(id);
        let mut evidence = BTreeMap::new();
        // The five the record itself answers (`record-derived.yaml`).
        if let Some(o) = self.objects.get(id) {
            evidence.insert("state".into(), serde_json::to_value(&o.config)?);
            evidence.insert("entered_at".into(), serde_json::to_value(&o.entered_at)?);
            evidence.insert("seq".into(), serde_json::json!(o.seq));
            evidence.insert(
                "applied_responses".into(),
                serde_json::to_value(&o.applied_responses)?,
            );
        }
        evidence.insert("now".into(), serde_json::json!(self.now.to_rfc3339()));
        for atom in flywheel_atoms::Evidence::all() {
            if let Some(v) = EvidenceSource::evidence(self, id, &region, atom.name) {
                evidence.insert(atom.name.to_string(), v);
            }
        }
        Ok(EvidenceRead {
            object: self.objects.get(id).cloned(),
            evidence,
            as_of: self.as_of(),
        })
    }

    fn list(&self, scope: &Scope) -> Result<Listing> {
        Ok(Listing {
            objects: self.list_records(scope)?,
            as_of: self.as_of(),
        })
    }

    fn status(&self) -> Result<StatusView> {
        self.deciding.served("status");
        // The status projection is written from `list` and `get` alone, and
        // states the point it is as of (132, 145).
        let as_of = self.as_of();
        let mut body = format!("status as of {} · {}\n", as_of.mark, as_of.at.to_rfc3339());
        for o in self.objects.values() {
            body.push_str(&format!(
                "{}\t{}\t{}\n",
                o.id,
                o.machine,
                o.top_states().join(",")
            ));
        }
        Ok(StatusView { as_of, body })
    }

    fn write_effect(&mut self, write: &EffectWrite) -> Result<WriteOutcome> {
        self.deciding.served("write_effect");
        // One commit per effect, carrying its identity, reason and evidence;
        // the repeat is found in the fetched history before anything is
        // committed (127).
        let outcome = self
            .state_repository()?
            .lock()
            .map_err(|_| anyhow!("the state repository is poisoned"))?
            .write_effect(write)?;
        if !matches!(outcome, WriteOutcome::AlreadyWritten { .. }) {
            let by = self.me();
            let pending = matches!(outcome, WriteOutcome::Pending { .. });
            self.effects_written.push(EffectRecord {
                effect_id: write.effect_id.clone(),
                object: write.object.clone(),
                effect: write.effect.clone(),
                reason: write.reason.clone(),
                evidence: write.evidence.clone(),
                at: self.now,
                by,
                pending,
            });
            let _ = self.moved_at(&write.object);
        }
        Ok(outcome)
    }

    fn lease(&mut self, op: &LeaseOp) -> Result<LeaseOutcome> {
        self.deciding.served("lease");
        // The push to `lease/<object>` with expected-old is the
        // compare-and-swap: two hosts cannot both land one (128, 163).
        let outcome = self
            .state_repository()?
            .lock()
            .map_err(|_| anyhow!("the state repository is poisoned"))?
            .lease(op)?;
        match (&outcome, op) {
            (LeaseOutcome::Held(l), _) => {
                self.leases.insert(l.object.clone(), l.clone());
            }
            (LeaseOutcome::Released, LeaseOp::Release { object, .. }) => {
                self.leases.remove(object);
            }
            _ => {}
        }
        Ok(outcome)
    }

    fn notify(&self, since: &ReadPoint) -> Result<Notice> {
        self.deciding.served("notify");
        // A notice names what moved, so a host re-reads only those objects and
        // never the whole store (130).
        let mut objects: Vec<String> = self
            .moved
            .iter()
            .filter(|(seq, _)| *seq > since.seq)
            .map(|(_, id)| id.clone())
            .collect();
        objects.sort();
        objects.dedup();
        Ok(Notice {
            as_of: self.as_of(),
            objects,
        })
    }

    fn present(&mut self, presentation: &Presentation) -> Result<()> {
        self.deciding.served("present");
        // A disconnected host delivers to no sink (151).
        if self.is_disconnected() {
            return Ok(());
        }
        self.presented.push(PresentedRecord {
            sink: presentation.sink.clone(),
            at: self.now,
            numbers: presentation
                .decisions
                .iter()
                .filter_map(|d| d.number)
                .collect(),
            decisions: presentation
                .decisions
                .iter()
                .map(|d| d.id.clone())
                .collect(),
        });
        self.marks.insert(presentation.sink.clone(), self.now);
        Ok(())
    }

    fn receive(&mut self, response: &Response) -> Result<Received> {
        self.deciding.served("receive");
        // The same delivery twice takes effect once, whatever restarts happen
        // between the giving and the application (137, I2).
        if self.responses.iter().any(|r| r.id == response.id)
            || self
                .objects
                .values()
                .any(|o| o.applied_responses.contains(&response.id))
        {
            return Ok(Received::AlreadyApplied {
                id: response.id.clone(),
            });
        }
        // A response naming a decision that has been retracted is handed back
        // as unapplicable and never dropped (6, 129).
        if let Some(number) = response.decision {
            match self.register.decision_of(number) {
                Some(decision) if self.standing.iter().any(|d| d == decision) => {}
                Some(decision) => {
                    let decision = decision.to_string();
                    self.log("unapplicable", &decision, "the decision no longer stands");
                    let reason = format!("decision {decision} no longer stands");
                    self.hand_back(response, &decision, &reason);
                    return Ok(Received::Unapplicable {
                        id: response.id.clone(),
                        reason,
                    });
                }
                None => {
                    let reason = format!("no decision carries number {number}");
                    self.hand_back(response, "rail", &reason);
                    return Ok(Received::Unapplicable {
                        id: response.id.clone(),
                        reason,
                    });
                }
            }
        }
        // The response is written before the transition it causes fires (129,
        // 153): the record first, the engine's guard second.
        self.responses.push(response.clone());
        let _ = self.moved_at(response.object.as_deref().unwrap_or("rail"));
        Ok(Received::Recorded {
            id: response.id.clone(),
        })
    }
}

impl Store {
    /// Hand a response back to the engine rather than dropping it: it stands
    /// under attention until one tick has reported it (6, 129).
    fn hand_back(&mut self, response: &Response, decision: &str, reason: &str) {
        if self.unapplicable.iter().any(|u| u.response == response.id) {
            return;
        }
        self.unapplicable.push(crate::store::Unapplicable {
            response: response.id.clone(),
            object: decision.split('/').next().unwrap_or("rail").to_string(),
            reason: reason.to_string(),
            reported_after: None,
        });
    }

    /// The status view, without the trait in scope.
    pub fn status_view(&self) -> flywheel_atoms::StatusView {
        StateStore::status(self).expect("the status view is derived, never fallible")
    }

    /// A lease whose holder stopped renewing it past the expiry is free to
    /// take (128, 163). The window is the engine's, read from the manifest at
    /// load; the release's default is 24 hours.
    pub fn lease_expired(&self, lease: &LeaseRecord) -> bool {
        // A lease its machine put at `expired` is expired whatever the clock
        // says: the gone host's decision answered takeover reaches that state
        // at once, and a lease nobody holds is nobody's (128, 150,
        // `lease.yaml` stale).
        lease.state == "expired"
            || lease.holder.is_empty()
            || self.now - lease.renewed_at > chrono::Duration::hours(24)
    }

    /// The bound the profile states for notification: a host learns what moved
    /// within it, by a webhook where the manifest names one and by the poll of
    /// the shared line's head otherwise (130, 166, `git-only.yaml` notify).
    pub fn notify_bound(&self) -> &'static str {
        "30s"
    }

    /// Past this, the holder shows as away with since-when; the lease is still
    /// its own until the expiry (128, 163, `git-only.yaml` leases).
    pub fn lease_stale(&self) -> chrono::Duration {
        chrono::Duration::minutes(5)
    }
}
