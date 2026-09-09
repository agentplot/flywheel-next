//! Running the machinery over the store: tick, settle, respond, dictate.

use crate::console;
use crate::store::Store;
use crate::world;
use chrono::Duration;
use flywheel_atoms::conformance::Hook;
use flywheel_atoms::{
    EffectWrite, LeaseOp, LeaseOutcome, PutOutcome, Records, Scope, StateStore,
};
use flywheel_engine::runtime::{DecisionInstance, EvidenceSource, Response, ResponseKind, Snapshot};
use flywheel_engine::{rail, tick, Definitions, PlannedEffect};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub struct Runtime {
    pub defs: Definitions,
    pub store: Store,
    /// The contract's declared hooks, honoured in process alone (D15).
    pub hooks: Vec<Hook>,
    /// The lease traffic of this run, which is what a scenario's `leases:`
    /// clause is asserted against (128, 163).
    pub lease_log: Vec<LeaseEvent>,
    /// How many writes this run attempted and how many the store took: two
    /// writers of one object cannot both succeed (134).
    pub writes_attempted: usize,
    pub writes_succeeded: usize,
    /// Whether the loser of a race learned it lost, and read again before
    /// deciding anything (134).
    pub loser_told: bool,
    pub loser_reread: bool,
    /// `interrupt_write` cuts one write off, once (135, D15).
    pub interrupted: bool,
}

/// One take of a lease, and what the store held at the moment it was asked
/// (128, 163).
#[derive(Debug, Clone)]
pub struct LeaseEvent {
    pub object: String,
    pub holder: String,
    pub held: bool,
    /// The lease that stood was past its stale window when this was asked.
    pub stale: bool,
    /// It was past its expiry, so it was free to take.
    pub expired: bool,
    /// Another host's live lease was overwritten — never, on a store that
    /// keeps its promise.
    pub doubled: bool,
}

/// What one tick did. The trace is rendered from these and the assertions
/// evaluate them, so the document a person reads and the evidence a failure
/// cites cannot diverge (95).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TickRecord {
    pub tick: u64,
    #[serde(default = "crate::runner::epoch")]
    pub at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub guards: Vec<GuardRecord>,
    #[serde(default)]
    pub transitions: Vec<TransitionRecord>,
    #[serde(default)]
    pub effects: Vec<EffectRecord2>,
    #[serde(default)]
    pub decisions: Vec<DecisionRecord>,
}

pub fn epoch() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(0, 0).expect("the epoch")
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GuardRecord {
    pub object: String,
    pub region: String,
    pub from: String,
    pub matched: String,
    pub response: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransitionRecord {
    pub object: String,
    pub region: String,
    pub from: String,
    pub to: String,
    pub response: Option<String>,
    pub reason: Option<String>,
}

/// One effect performed, with the identity the write carries (79, 127).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EffectRecord2 {
    pub effect_id: String,
    pub name: String,
    pub object: String,
    pub args: BTreeMap<String, Value>,
    /// Whether the store reported this as a write of its own. A repeat of an
    /// effect already written is not a second write, though the act ran (127).
    #[serde(default)]
    pub written: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecisionRecord {
    pub id: String,
    pub object: String,
    pub kind: String,
    pub number: Option<u32>,
}

impl Runtime {
    pub fn new(defs: Definitions, store: Store) -> Self {
        Runtime {
            defs,
            store,
            hooks: vec![],
            lease_log: vec![],
            writes_attempted: 0,
            writes_succeeded: 0,
            loser_told: false,
            loser_reread: false,
            interrupted: false,
        }
    }

    /// The standing decisions, numbered. A number a scenario gave by the
    /// decision's readable name is honoured here, before anything reads it.
    pub fn decisions(&mut self) -> Vec<DecisionInstance> {
        if !self.store.register_aliases.is_empty() {
            let standing = rail::derive(&self.defs, &self.store.objects, &mut self.store.register.clone());
            for d in &standing {
                if let Some(n) = self.store.register_aliases.get(&Runtime::decision_name(&d.object, &d.kind)) {
                    self.store.register.numbers.insert(d.id.clone(), *n);
                    if self.store.register.next_number <= *n {
                        self.store.register.next_number = n + 1;
                    }
                }
            }
        }
        let mut d = rail::derive(&self.defs, &self.store.objects, &mut self.store.register);
        d.extend(self.handed_back());
        self.store.standing = d.iter().map(|x| x.id.clone()).collect();
        for x in &d {
            if let Some(n) = x.number {
                self.store
                    .decision_numbers
                    .insert(Runtime::decision_name(&x.object, &x.kind), n);
            }
        }
        d
    }

    /// A response the store could not apply stands under attention until one
    /// tick has reported it. It is handed back to the engine, never dropped
    /// (6, 129).
    fn handed_back(&mut self) -> Vec<DecisionInstance> {
        let tick = self.store.tick;
        let now = self.store.now;
        let mut out = Vec::new();
        for u in &self.store.unapplicable {
            match u.reported_after {
                Some(reported) if tick > reported => continue,
                _ => {}
            }
            out.push(DecisionInstance {
                id: format!("response/{}/response-unapplicable", u.response),
                object: format!("response/{}", u.response),
                region: "life".into(),
                state: "unapplicable".into(),
                kind: "response-unapplicable".into(),
                group: "attention".into(),
                number: None,
                answers: vec!["ok".into()],
                shows: vec![u.reason.clone()],
                document: None,
                since: now,
            });
        }
        out
    }

    /// The readable name of a decision: `<object id>/<decision kind>`, which is
    /// how a scenario names one.
    pub fn decision_name(object: &str, kind: &str) -> String {
        format!("{object}/{kind}")
    }

    /// The standing decision a scenario's readable name resolves to, right now.
    pub fn standing_decision(&mut self, name: &str) -> Option<DecisionInstance> {
        self.decisions()
            .into_iter()
            .find(|d| Runtime::decision_name(&d.object, &d.kind) == name)
    }

    /// One pass: plan, apply, perform. Returns how many transitions fired.
    pub fn tick(&mut self) -> usize {
        self.tick_recorded().transitions.len()
    }

    /// One pass, with what happened, and the clock moved once.
    pub fn tick_recorded(&mut self) -> TickRecord {
        let record = self.pass();
        self.store.now = self.store.now + Duration::seconds(self.store.tick_seconds);
        record
    }

    /// One tick as a scenario's `tick` step means it: plan, apply and perform
    /// until nothing more fires, because a tick is one bounded invocation that
    /// decides everything the state it read implies (D7). The clock moves once
    /// for the whole tick, which is the second of the two reasons it ever
    /// moves (D15).
    pub fn tick_settled(&mut self) -> TickRecord {
        let mut record = TickRecord { tick: self.store.tick + 1, at: self.store.now, ..Default::default() };
        let mut moved: Vec<String> = Vec::new();
        for _ in 0..50 {
            let pass = self.pass_with(&mut moved);
            // A self-transition is not progress: it re-enters the state it is
            // already in, so a tick that only re-entered has settled. Without
            // this the tick would perform its effect once per pass.
            let moved = pass.transitions.iter().any(|t| t.from != t.to);
            record.guards.extend(pass.guards);
            record.transitions.extend(pass.transitions);
            record.effects.extend(pass.effects);
            record.decisions = pass.decisions;
            if !moved {
                break;
            }
        }
        self.store.now = self.store.now + Duration::seconds(self.store.tick_seconds);
        record
    }

    /// One plan-apply-perform pass. The clock does not move here.
    ///
    /// Everything read comes from `list`, `get` and the rail record, and
    /// everything written goes through `put` and `write_effect`: the pass never
    /// reaches into one store's own fields, which is what lets the same tick
    /// run over the stand-in and over `flywheel-store-git` with no branch
    /// between them (125, 136, 139, D15).
    fn pass(&mut self) -> TickRecord {
        self.pass_with(&mut Vec::new())
    }

    /// One pass, told what this tick has already written. A tick writes each
    /// object once, however many passes it settles over (D7, D4).
    fn pass_with(&mut self, moved: &mut Vec<String>) -> TickRecord {
        let objects = self.read_before_deciding();
        let fired = self.plan(&objects);
        let mut record = TickRecord { tick: self.store.tick, at: self.store.now, ..Default::default() };
        self.perform(&objects, &fired, &mut record, moved);
        self.reperform_unproved(&mut record);
        record.decisions = self
            .decisions()
            .iter()
            .map(|d| DecisionRecord { id: d.id.clone(), object: d.object.clone(), kind: d.kind.clone(), number: d.number })
            .collect();
        self.report_handed_back();
        record
    }

    /// A tick of several hosts at once: one read, each host's plan against it,
    /// and the clock moved once. There is no settle loop — the point of the
    /// step is the race, and a second pass would be a second read (D15).
    pub fn tick_concurrent(&mut self, hosts: &[String]) -> TickRecord {
        let record = self.pass_concurrent(hosts);
        self.store.now = self.store.now + Duration::seconds(self.store.tick_seconds);
        record
    }

    /// One pass for each of several hosts, all planning from the one read they
    /// share. That is what puts two writers on one object: the second carries
    /// the sequence it read, the store rejects it, and the loser learns that it
    /// lost (134, 162, I15).
    pub fn pass_concurrent(&mut self, hosts: &[String]) -> TickRecord {
        let objects = self.read_before_deciding();
        let mut record = TickRecord { tick: self.store.tick, at: self.store.now, ..Default::default() };
        for host in hosts {
            self.store.acting_host = Some(host.clone());
            let fired = self.plan(&objects);
            // Each host writes for itself: what one wrote does not stop the
            // other from attempting its own write, which is the race (134).
            self.perform(&objects, &fired, &mut record, &mut Vec::new());
        }
        self.reperform_unproved(&mut record);
        record.decisions = self
            .decisions()
            .iter()
            .map(|d| DecisionRecord { id: d.id.clone(), object: d.object.clone(), kind: d.kind.clone(), number: d.number })
            .collect();
        self.report_handed_back();
        record
    }

    /// A tick that showed a handed-back response has reported it; it stands no
    /// longer, and it is not shown twice (6, 129).
    fn report_handed_back(&mut self) {
        let tick = self.store.tick;
        for u in &mut self.store.unapplicable {
            if u.reported_after.is_none() {
                u.reported_after = Some(tick);
            }
        }
    }

    /// Fetch, heartbeat, play what the world does on its own, and read every
    /// object through `list` — what a tick does before it decides anything
    /// (131, D7).
    fn read_before_deciding(&mut self) -> BTreeMap<String, flywheel_engine::Object> {
        self.store.tick += 1;
        // Alive hosts heartbeat: their last_seen is now.
        let now = self.store.now;
        let hosts: Vec<String> = self
            .store
            .list_records(&Scope::Machine("host".into()))
            .unwrap_or_default()
            .into_iter()
            .filter(|o| o.record.get("alive").and_then(|v| v.as_bool()).unwrap_or(true))
            .map(|o| o.id)
            .collect();
        for h in hosts { self.store.set_given(&h, "host.last_seen", json!(now.to_rfc3339())); }
        self.store.play_scripts();
        self.store.play_services();
        console::objects(&self.store, &Scope::All).unwrap_or_default()
    }

    /// Decide what fires, from the objects read and nothing else.
    fn plan(&self, objects: &BTreeMap<String, flywheel_engine::Object>) -> Vec<tick::Fired> {
        let responses = Records::responses(&self.store, console::RAIL).unwrap_or_default();
        let register = console::register(&self.store).unwrap_or_default();
        let snap = Snapshot { objects, responses: &responses, register: &register, evidence: &self.store, now: self.store.now };
        tick::plan_tick(&self.defs, &snap)
    }

    /// Renew what this host holds and take what it may: a host owns the
    /// objects it acts on through a lease, takes only within its declaration
    /// and never more than its bound (149, 150, 163). A lease past its expiry
    /// is free to take.
    fn renew_and_take(&mut self, objects: &BTreeMap<String, flywheel_engine::Object>) {
        if self.hooks.contains(&Hook::BypassLease) {
            return;
        }
        let me = self.store.me();
        for (id, object) in objects {
            // A host record is not an object a host holds.
            if object.machine == "host" {
                continue;
            }
            let standing = Records::leases(&self.store, id).ok().flatten();
            let mine = standing.as_ref().is_some_and(|l| l.holder == me);
            let free = standing
                .as_ref()
                .map(|l| self.store.lease_expired(l))
                .unwrap_or(true);
            if !mine && !free {
                continue;
            }
            self.hold(id);
        }
    }

    /// Take the lease, write the transition, perform the effects.
    fn perform(&mut self, objects: &BTreeMap<String, flywheel_engine::Object>, fired: &[tick::Fired], record: &mut TickRecord, moved: &mut Vec<String>) {
        self.renew_and_take(objects);
        // One object, one write: every region of it that fired on this read is
        // applied to the copy this host read and put back in one go (D4). A
        // region moves once per tick, however many passes the tick settles
        // over, because what a write implies is the next tick's to read (D7).
        let mut order: Vec<String> = Vec::new();
        for f in fired {
            if !order.contains(&f.object) {
                order.push(f.object.clone());
            }
        }
        for id in order {
            // One region of one object moves once per write, and once per
            // tick however many passes the tick settles over (D4, D7).
            let mut here: Vec<&tick::Fired> = Vec::new();
            for f in fired {
                if f.object != id || moved.contains(&format!("{id}#{}", f.region)) {
                    continue;
                }
                if here.iter().any(|g| g.region == f.region) {
                    continue;
                }
                here.push(f);
            }
            if here.is_empty() {
                continue;
            }
            if !self.hold(&id) {
                continue;
            }
            let Some(read) = objects.get(&id).cloned() else { continue };
            let base = read.seq;
            let mut next = read.clone();
            let mut applied: Vec<(&tick::Fired, Vec<(String, flywheel_engine::PlannedEffect)>, Vec<flywheel_engine::runtime::TailEntry>)> = Vec::new();
            let regions: Vec<String> = here.iter().map(|f| f.region.clone()).collect();
            for f in here {
                // Commanded from the object as it was read, not from the copy
                // earlier transitions of this same read have already moved. A
                // command into a region that is moving under its own transition
                // in this same write is that transition's to issue, not the
                // command's: otherwise one act would be performed twice.
                let commanded: Vec<(String, flywheel_engine::PlannedEffect)> =
                    tick::commanded_effects(&self.defs, &read, f)
                        .into_iter()
                        .filter(|(path, _)| {
                            !regions.iter().any(|r: &String| {
                                r == path || r.starts_with(&format!("{path}."))
                            })
                        })
                        .collect();
                let tail = tick::apply(&self.defs, &mut next, f, self.store.now);
                applied.push((f, commanded, tail));
            }
            // `interrupt_write` cuts the acting host's first write off midway.
            // One write is one commit: cut off, it is not there at all, and no
            // reader sees half of it (135, D15). The host went with it, so the
            // lease it wrote under is free for the next host (128).
            self.writes_attempted += 1;
            if self.hooks.contains(&Hook::InterruptWrite) && !self.interrupted {
                self.interrupted = true;
                let holder = self.store.me();
                let _ = self.store.lease(&LeaseOp::Release { object: id.clone(), holder });
                self.store.log("write", &id, "cut off midway; nothing was written (135)");
                continue;
            }
            // A rejection means another writer moved it first: the loser reads
            // again before deciding anything, and its transitions never
            // happened (134, 162).
            match self.store.put(&id, &next, base) {
                Ok(PutOutcome::Written { .. }) => self.writes_succeeded += 1,
                Ok(PutOutcome::Rejected { held_seq }) => {
                    self.loser_told = true;
                    let _ = StateStore::read(&self.store, &id);
                    self.loser_reread = true;
                    self.store.log(
                        "write",
                        &id,
                        format!("rejected: the store holds sequence {held_seq}; reading again before deciding (134)"),
                    );
                    continue;
                }
                Err(_) => continue,
            }
            for (f, commanded, tail) in applied {
                moved.push(format!("{id}#{}", f.region));
                self.store.log("transition", &id, format!("{}: {} → {}{}", f.region, f.from, f.to, f.note.as_ref().map(|n| format!(" — {n}")).unwrap_or_default()));
                // The guard that matched, as the trace states it.
                record.guards.push(GuardRecord {
                    object: id.clone(),
                    region: f.region.clone(),
                    from: f.from.clone(),
                    matched: f.note.clone().unwrap_or_else(|| format!("→ {}", f.to)),
                    response: f.response.as_ref().map(|(r, _)| r.clone()),
                });
                record.transitions.push(TransitionRecord {
                    object: id.clone(),
                    region: f.region.clone(),
                    from: f.from.clone(),
                    to: f.to.clone(),
                    response: f.response.as_ref().map(|(r, _)| r.clone()),
                    reason: f.note.clone(),
                });
                for (region, e) in commanded.iter().map(|(r, e)| (r.as_str(), e)).chain(f.effects.iter().map(|e| (f.region.as_str(), e))) {
                    // Only an act the world actually performed is an effect the
                    // scenario counts (72, 73).
                    if world::perform(&self.defs, &mut self.store, &id, region, e) {
                        let written = self.write_effect(&id, region, e, f.note.as_deref());
                        record.effects.push(EffectRecord2 {
                            effect_id: e.id.clone(),
                            name: e.name.clone(),
                            object: id.clone(),
                            args: e.args.clone(),
                            written,
                        });
                    }
                }
                self.store.tail.extend(tail);
            }
        }
    }

    /// Take or renew the lease on an object before writing it: every object is
    /// owned by at most one host, and a host that cannot take the lease reads
    /// again and moves on (128, 150, I11). `bypass_lease` is the contract's
    /// declared hook, honoured in process alone (D15).
    fn hold(&mut self, object: &str) -> bool {
        if self.hooks.contains(&Hook::BypassLease) {
            return true;
        }
        let holder = self.store.me();
        let before = Records::leases(&self.store, object).ok().flatten();
        let stale = before.as_ref().is_some_and(|l| self.store.now - l.renewed_at > self.store.lease_stale());
        let expired = before.as_ref().is_some_and(|l| self.store.lease_expired(l));
        match self.store.lease(&LeaseOp::Take { object: object.to_string(), holder: holder.clone() }) {
            Ok(LeaseOutcome::Held(l)) => {
                let doubled = before
                    .as_ref()
                    .is_some_and(|b| b.holder != holder && !expired);
                self.lease_log.push(LeaseEvent {
                    object: object.to_string(),
                    holder: l.holder,
                    held: true,
                    stale,
                    expired,
                    doubled,
                });
                true
            }
            Ok(LeaseOutcome::HeldByAnother(l)) => {
                // The loser reads the holder and moves on; it decides nothing
                // on an object it does not hold (128, 150).
                let _ = StateStore::read(&self.store, object);
                self.loser_reread = true;
                self.lease_log.push(LeaseEvent {
                    object: object.to_string(),
                    holder: l.holder,
                    held: false,
                    stale,
                    expired,
                    doubled: false,
                });
                false
            }
            _ => true,
        }
    }

    /// Write one performed effect with an identity of its own. A repeat of an
    /// effect already written changes nothing, is not an error and is not
    /// reported as a second write; the act ran either way (127, 167).
    fn write_effect(&mut self, object: &str, region: &str, e: &PlannedEffect, note: Option<&str>) -> bool {
        let mut evidence: BTreeMap<String, Value> = BTreeMap::new();
        if let Some(proof) = self.defs.atoms.proof_of(&e.name) {
            let held = EvidenceSource::evidence(&self.store, object, region, &proof);
            evidence.insert(proof, held.unwrap_or(Value::Null));
        }
        let write = EffectWrite {
            effect_id: e.id.clone(),
            object: object.to_string(),
            effect: e.name.clone(),
            reason: note.unwrap_or(&e.id).to_string(),
            evidence,
        };
        matches!(
            StateStore::write_effect(&mut self.store, &write),
            Ok(flywheel_atoms::WriteOutcome::Written { .. })
        )
    }

    /// An effect whose proof the world does not report is not done: the act
    /// runs again on the next tick, and the write stays the one it already is
    /// (73, 127). This is the reconciler over the effects this host wrote.
    fn reperform_unproved(&mut self, record: &mut TickRecord) {
        let outstanding: Vec<(String, String, String, String)> = self
            .store
            .effects_written
            .iter()
            .filter_map(|w| {
                // An act performed on this tick has not had its chance to be
                // proved yet: the world is read again on the next one.
                if w.at == self.store.now {
                    return None;
                }
                let proof = self.defs.atoms.proof_of(&w.effect)?;
                // The region the act was performed in is in the effect's own
                // identity: `<object>/<region>/<from>-><to>/<phase>/<name><i>`
                // (79, 127, 167). The act runs again where it ran before.
                let region = w
                    .effect_id
                    .strip_prefix(&format!("{}/", w.object))
                    .and_then(|rest| rest.split('/').next())
                    .unwrap_or("life")
                    .to_string();
                // The world's own report, not evidence derived from the
                // record: a proof the world says is absent calls the act back,
                // and silence is not a denial (72, 73, 127).
                let reported = self
                    .store
                    .given
                    .get(&w.object)
                    .and_then(|m| m.get(&proof))
                    .or_else(|| self.store.given.get("*").and_then(|m| m.get(&proof)));
                let _ = &region;
                match reported {
                    Some(v) if flywheel_engine::eval::truthy(v) => None,
                    None => None,
                    _ => Some((w.effect_id.clone(), w.object.clone(), w.effect.clone(), region)),
                }
            })
            .collect();
        for (effect_id, object, effect, region) in outstanding {
            let planned = PlannedEffect {
                id: effect_id.clone(),
                name: effect.clone(),
                args: Default::default(),
                note: Some("the proof is absent; the act runs again (73, 127)".into()),
            };
            if world::perform(&self.defs, &mut self.store, &object, &region, &planned) {
                let written = self.write_effect(&object, &region, &planned, planned.note.as_deref());
                record.effects.push(EffectRecord2 {
                    effect_id,
                    name: effect,
                    object,
                    args: Default::default(),
                    written,
                });
            }
        }
    }

    /// Tick until nothing fires (or a bound), so one response cascades as far as it can.
    pub fn settle(&mut self, max: usize) -> usize {
        let mut total = 0;
        for _ in 0..max {
            let n = self.tick();
            total += n;
            if n == 0 { break; }
        }
        total
    }

    pub fn respond(&mut self, number: u32, answer: &str, by: &str) -> String {
        let id = format!("page-{}", self.store.next_response);
        self.store.next_response += 1;
        let r = Response { id: id.clone(), kind: ResponseKind::Answer, decision: Some(number), object: None, answer: answer.to_string(), given_by: by.to_string(), given_at: self.store.now, delivery: "page".into() };
        self.store.responses.push(r);
        let rec = [("decision".to_string(), json!(number)), ("answer".to_string(), json!(answer)), ("given_by".to_string(), json!(by))].into_iter().collect();
        world::new_object(&self.defs, &mut self.store, &format!("response/{id}"), "response", None, rec);
        self.store.log("response", &format!("response/{id}"), format!("{number} → {answer}"));
        id
    }

    /// What one capture-box submission made: the object it created and what it was read as.
    pub fn captured(&self) -> Vec<Value> {
        let mut caps: Vec<&flywheel_engine::Object> = self.store.objects.values().filter(|o| o.machine == "capture").collect();
        caps.sort_by_key(|o| std::cmp::Reverse(o.created));
        caps.iter().map(|c| {
            let signals: Vec<Value> = self.store.objects.values().filter(|o| o.parent.as_deref() == Some(&c.id) && o.machine == "signal")
                .map(|s| json!({"id": s.id, "kind": s.record.get("kind"), "state": s.top_state(), "assertion": s.record.get("assertion")})).collect();
            json!({"id": c.id, "state": c.top_state(), "raw": c.record.get("raw"), "source": c.record.get("source"), "intent": c.record.get("intent").and_then(|v| v.as_bool()).unwrap_or(false), "at": c.record.get("event_at"), "signals": signals})
        }).collect()
    }

    /// The page's capture box (19, 194): typed text is a capture with one signal of kind ask, so
    /// curation sees it. No prefix is parsed out of the text; `intent: true` is the separate
    /// control's judgment, carried on the capture record for curation. The submission is
    /// recorded once as a response naming the capture it made.
    pub fn capture(&mut self, text: &str, intent: bool, by: &str) -> Value {
        let text = text.trim();
        let n = self.store.next_response;
        let now = self.store.now;
        let id = format!("capture/page-{n}");
        let rec = [("source", json!("page")), ("event_key", json!(format!("page-{n}"))), ("event_at", json!(now)), ("captured_by", json!(by)), ("raw", json!(text)), ("intent", json!(intent))].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        world::new_object(&self.defs, &mut self.store, &id, "capture", None, rec);
        let srec: BTreeMap<String, Value> = [("kind", json!("ask")), ("asserted_by", json!(by)), ("assertion", json!(text)), ("excerpt", json!(text)), ("subject_tags", json!([])), ("argues_with", json!([]))].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
        world::new_object(&self.defs, &mut self.store, &format!("signal/page-{n}"), "signal", Some(&id), srec);
        // The submission is the delivery: one response, already applied by the capture it made.
        let rid = self.dictate(&id, text, by);
        if let Some(o) = self.store.objects.get_mut(&id) { if !o.applied_responses.contains(&rid) { o.applied_responses.push(rid.clone()); } }
        let kind = if intent { "capture · intent" } else { "capture" };
        self.store.log("capture", &id, format!("{kind} from the page"));
        json!({"id": id, "kind": kind, "intent": intent, "response": rid})
    }

    pub fn dictate(&mut self, object: &str, answer: &str, by: &str) -> String {
        let id = format!("page-{}", self.store.next_response);
        self.store.next_response += 1;
        let r = Response { id: id.clone(), kind: ResponseKind::Dictation, decision: None, object: Some(object.to_string()), answer: answer.to_string(), given_by: by.to_string(), given_at: self.store.now, delivery: "page".into() };
        self.store.responses.push(r);
        let rec = [("object".to_string(), json!(object)), ("answer".to_string(), json!(answer)), ("given_by".to_string(), json!(by))].into_iter().collect();
        world::new_object(&self.defs, &mut self.store, &format!("response/{id}"), "response", None, rec);
        self.store.log("dictation", object, answer.to_string());
        id
    }
}

