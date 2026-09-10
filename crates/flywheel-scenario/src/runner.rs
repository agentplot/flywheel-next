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
    /// The operator's own commits this run read out of a fetch (164).
    pub operator_commits: Vec<String>,
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

/// The regions a guard reads: the ones it names, and whether it reads the
/// submachine running inside the state through `final:`.
fn regions_read(guard: &flywheel_engine::defs::Guard) -> (Vec<String>, bool) {
    use flywheel_engine::defs::Guard;
    let mut named = Vec::new();
    let mut reads_final = false;
    fn walk(guard: &Guard, named: &mut Vec<String>, reads_final: &mut bool) {
        match guard {
            Guard::Region { region } => named.push(region.name.clone()),
            Guard::Final { .. } => *reads_final = true,
            Guard::All { all } => all.iter().for_each(|g| walk(g, named, reads_final)),
            Guard::Any { any } => any.iter().for_each(|g| walk(g, named, reads_final)),
            Guard::Not { not } => walk(not, named, reads_final),
            _ => {}
        }
    }
    walk(guard, &mut named, &mut reads_final);
    (named, reads_final)
}

/// The response name a guard is driven by, wherever it sits in the algebra.
fn response_of(guard: &flywheel_engine::defs::Guard) -> Option<String> {
    use flywheel_engine::defs::Guard;
    match guard {
        Guard::Response { response } => Some(response.clone()),
        Guard::All { all } => all.iter().find_map(response_of),
        Guard::Any { any } => any.iter().find_map(response_of),
        Guard::Not { not } => response_of(not),
        _ => None,
    }
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
    /// A host's declaration covered the object at the moment it was asked
    /// (149); nothing is ever taken when this is false.
    pub coverable: bool,
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
    /// The host that made the write, where the run knows which. Two hosts of
    /// one instance write on the same line, so a scenario may say which of them
    /// a move was (147, 232, S18).
    #[serde(default)]
    pub host: Option<String>,
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
    /// Whether the reconciler called the act back because its proof was gone
    /// from the world. Such an act ran again and counts again, though its
    /// write carries the identity it had and is no second write (73, 127).
    #[serde(default)]
    pub recalled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DecisionRecord {
    pub id: String,
    pub object: String,
    pub kind: String,
    /// The rail group it is in: approve, decide, answer or attention. A line
    /// under attention is not one of the numbered decisions a person works
    /// through (15, 82).
    #[serde(default)]
    pub group: String,
    pub number: Option<u32>,
}

impl Runtime {
    pub fn new(defs: Definitions, mut store: Store) -> Self {
        // The store names its sessions against these definitions (S08).
        store.defs = Some(std::sync::Arc::new(defs.clone()));
        Runtime {
            defs,
            store,
            hooks: vec![],
            operator_commits: vec![],
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
        // The lease objects stand beside the store's own: a lease is a branch
        // and never a file on the shared line, and one of its states is a line
        // under attention (D5, 149).
        let mut objects = self.store.objects.clone();
        let now = self.store.now;
        let _ = flywheel_domain::leases::attach(&self.store, &mut objects, now);
        let defs = self.defs.clone();
        let _ = flywheel_domain::rail::attach(&self.store, &defs, &mut objects, now);
        if !self.store.register_aliases.is_empty() {
            let standing = rail::derive(&self.defs, &objects, &self.store.register);
            for d in &standing {
                if let Some(n) = self.store.register_aliases.get(&Runtime::decision_name(&d.object, &d.kind)) {
                    // A scenario that states a decision's number states it as
                    // the register already having given it (`given.register`).
                    self.store.register.entries.insert(
                        d.id.clone(),
                        flywheel_engine::runtime::RegisterEntry {
                            number: *n,
                            since: Some(d.since),
                            ..Default::default()
                        },
                    );
                    if self.store.register.next_number <= *n {
                        self.store.register.next_number = n + 1;
                    }
                }
            }
        }
        let mut d = rail::derive(&self.defs, &objects, &self.store.register);
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
                folds: vec![format!("response/{}", u.response)],
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
        // One tick reports a handed-back response once, however many passes it
        // settled over: the report is the tick's, not the pass's (6, 129, D7).
        self.report_handed_back();
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
            let objects = self.store.next_created;
            let pass = self.pass_with(&mut moved);
            // A self-transition is not progress: it re-enters the state it is
            // already in, so a tick that only re-entered has settled. Without
            // this the tick would perform its effect once per pass. An object
            // the pass made is progress though it moved no region of its own:
            // nothing had read it when the tick read, so its own regions and
            // the rail's register have still to be decided upon (D7).
            let moved = pass.transitions.iter().any(|t| t.from != t.to)
                || self.store.next_created != objects;
            record.guards.extend(pass.guards);
            record.transitions.extend(pass.transitions);
            record.effects.extend(pass.effects);
            record.decisions = pass.decisions;
            if !moved {
                break;
            }
        }
        self.report_handed_back();
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
        let mut objects = self.read_before_deciding();
        // A host owns what it acts on through a lease, so it takes what it may
        // before it decides anything: the lease machine then reads a holder in
        // the same pass and the object is worked in the one after (128, 149).
        self.renew_and_take(&objects);
        let now = self.store.now;
        let _ = flywheel_domain::leases::attach(&self.store, &mut objects, now);
        let fired = self.plan(&objects);
        self.store.sessions_running_max = self.store.sessions_running_max.max(self.store.host_running());
        let mut record = TickRecord { tick: self.store.tick, at: self.store.now, ..Default::default() };
        self.perform(&objects, &fired, &mut record, moved);
        self.reperform_unproved(&mut record);
        // A decision exists exactly while its state is active: one whose state
        // was left this pass is retracted on its own register entry, which is
        // where a late reply reads that it is gone (9, I3, 15).
        self.store.retract_gone();
        // The projection is rewritten from its source whenever what it projects
        // moved, and stored nowhere else (77, 132, 142, D12).
        let defs = self.defs.clone();
        self.store.write_status(&defs);
        record.decisions = self
            .decisions()
            .iter()
            .map(|d| DecisionRecord { id: d.id.clone(), object: d.object.clone(), kind: d.kind.clone(), group: d.group.clone(), number: d.number })
            .collect();
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
        // The projection is rewritten from its source whenever what it projects
        // moved, and stored nowhere else (77, 132, 142, D12).
        let defs = self.defs.clone();
        self.store.write_status(&defs);
        record.decisions = self
            .decisions()
            .iter()
            .map(|d| DecisionRecord { id: d.id.clone(), object: d.object.clone(), kind: d.kind.clone(), group: d.group.clone(), number: d.number })
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
        // Fetch first, so no host decides on a read older than the bound (165,
        // D7), and read the operator's own commits out of what came back.
        self.fetch();
        // What the engine decides from: `list` and `read`, and nothing the
        // store keeps privately (136, 142). The window says so of itself —
        // every operation served while it stands is recorded.
        let deciding = self.store.deciding.open();
        let mut objects = console::objects(&self.store, &Scope::All).unwrap_or_default();
        let at = self.store.now;
        let _ = flywheel_domain::rail::attach(&self.store, &self.defs, &mut objects, at);
        drop(deciding);
        objects
    }

    /// Read the store again, and read out of what came back anything a person
    /// put there. Every fetch goes through here, so no host takes the
    /// operator's own commit for a write of the machinery's (165, 164).
    pub fn fetch(&mut self) {
        let before = self.store.objects.clone();
        self.store.refresh();
        self.read_operator_commits(&before);
    }

    /// An object whose state moved with no sequence of its own was written by
    /// a person, not by the machinery: the operator edited the state where it
    /// is kept and committed it. Every host derives the same response from the
    /// same fetch, with the commit as its delivery, and the machine makes the
    /// move itself so the record carries what a transition carries (3, 159,
    /// 164, S19).
    fn read_operator_commits(&mut self, before: &BTreeMap<String, flywheel_engine::Object>) {
        let now = self.store.now;
        let mut found: Vec<(String, String, String, flywheel_engine::Object)> = Vec::new();
        for (id, after) in &self.store.objects {
            let Some(was) = before.get(id) else { continue };
            if was.seq != after.seq || was.config == after.config {
                continue;
            }
            let Some((region, from, to)) = was
                .config
                .iter()
                .find(|(k, v)| after.config.get(*k).is_some_and(|w| w != *v))
                .map(|(k, v)| (k.clone(), v.clone(), after.config.get(k).cloned().unwrap_or_default()))
            else {
                continue;
            };
            let Some(answer) = self.answer_for(was, &region, &from, &to) else { continue };
            let delivery = self
                .store
                .durable()
                .and_then(|d| d.lock().ok().and_then(|g| g.operator_commit_on(id).ok().flatten()))
                .map(|sha| format!("commit/{sha}"))
                .unwrap_or_else(|| format!("commit/{id}"));
            found.push((id.clone(), delivery, answer, was.clone()));
        }
        for (id, delivery, answer, was) in found {
            // The edit is the response, not the move: the object stands where
            // it stood until the machine applies the transition, which is what
            // writes the entry time, the counters and the delivery's own id.
            // It stands so on every fetch until then, however many pass.
            let applied = was.applied_responses.contains(&delivery);
            if !applied {
                self.store.objects.insert(id.clone(), was);
            }
            if applied || self.store.responses.iter().any(|r| r.id == delivery) {
                continue;
            }
            let response = Response {
                id: delivery.clone(),
                // The operator's own commit names the object, not a number:
                // it is a dictation, applied and never proposed (4, 159).
                kind: ResponseKind::Dictation,
                decision: None,
                object: Some(id.clone()),
                answer,
                given_by: "operator".into(),
                given_at: now,
                delivery: delivery.clone(),
            };
            if let Ok(flywheel_atoms::Received::Recorded { .. }) = self.store.receive(&response) {
                self.store.log("response", &id, format!("{delivery} — the operator's own commit"));
                self.operator_commits.push(delivery);
            }
        }
    }

    /// Whether a transition out of a state the object is in re-issues this
    /// effect. Where one does, calling the act back is the machine's to say.
    fn machine_calls_back(&self, object: &str, effect: &str) -> bool {
        let Ok(Some(o)) = self.store.get(object) else { return false };
        o.config.keys().any(|region| {
            tick::state_def(&self.defs, &o, region).is_some_and(|(_, st)| {
                st.transitions
                    .iter()
                    .any(|t| t.effects.iter().any(|e| e.name == effect))
            })
        })
    }

    /// The answer a transition out of this state into that one is guarded by,
    /// where one is. Nothing is inferred from a transition no response drives.
    fn answer_for(&self, object: &flywheel_engine::Object, region: &str, from: &str, to: &str) -> Option<String> {
        let mut probe = object.clone();
        probe.config.insert(region.to_string(), from.to_string());
        let (_reg, state) = tick::state_def(&self.defs, &probe, region)?;
        state
            .transitions
            .iter()
            .filter(|t| t.to == to)
            .find_map(|t| response_of(&t.when))
    }

    /// Decide what fires, from the objects read and nothing else.
    fn plan(&self, objects: &BTreeMap<String, flywheel_engine::Object>) -> Vec<tick::Fired> {
        let _deciding = self.store.deciding.open();
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
            // The machinery's own objects are not ones a host holds.
            if !flywheel_domain::leases::leasable(object) {
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

    /// Whether this transition is guarded on a region of the same object that
    /// moved earlier in this tick — a sibling, the parent, or a child through a
    /// `final:` guard. Such a move is the next tick's to read.
    fn reads_a_region_that_moved(&self, id: &str, fired: &tick::Fired, moved: &[String]) -> bool {
        if moved.is_empty() {
            return false;
        }
        let Ok(Some(object)) = self.store.get(id) else {
            return false;
        };
        let mut probe = object.clone();
        probe.config.insert(fired.region.clone(), fired.from.clone());
        let Some((_region, state)) = tick::state_def(&self.defs, &probe, &fired.region) else {
            return false;
        };
        // The regions of this object that moved earlier in this tick.
        let moved_here: Vec<&str> = moved
            .iter()
            .filter_map(|m| m.split_once('#'))
            .filter(|(object, _)| *object == id)
            .map(|(_, region)| region)
            .collect();
        for transition in &state.transitions {
            if transition.to != fired.to {
                continue;
            }
            // A `final:` guard is the state's own submachine concluding, not a
            // region of the object beside it; it is read in the tick it
            // happens, which is what S29 and S01 both count on.
            let (named, _reads_final) = regions_read(&transition.when);
            for name in &named {
                if moved_here
                    .iter()
                    .any(|region| *region == name || region.ends_with(&format!(".{name}")))
                {
                    return true;
                }
            }
        }
        false
    }

    /// Whether the lease machine says this host holds the object. The
    /// machinery's own objects have no lease and are always its to write.
    fn holding(&self, id: &str) -> bool {
        if self.hooks.contains(&Hook::BypassLease) {
            return true;
        }
        let Ok(Some(object)) = self.store.get(id) else {
            return true;
        };
        if !flywheel_domain::leases::leasable(&object) {
            return true;
        }
        let Ok(Some(lease)) = Records::leases(&self.store, id) else {
            return false;
        };
        lease.state == "held" && lease.holder == self.store.me()
    }

    /// Take the lease, write the transition, perform the effects.
    fn perform(&mut self, objects: &BTreeMap<String, flywheel_engine::Object>, fired: &[tick::Fired], record: &mut TickRecord, moved: &mut Vec<String>) {
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
            // A region of an object moves once per tick, however many passes
            // the tick settles over: what a write implies is the next tick's to
            // read (D4, D7). A self-transition is not a second move — it is how
            // a machine calls an unproved effect back — so it still runs.
            let mut here: Vec<&tick::Fired> = Vec::new();
            for f in fired {
                if f.object != id {
                    continue;
                }
                if f.to != f.from && moved.contains(&format!("{id}#{}", f.region)) {
                    continue;
                }
                if here.iter().any(|g| g.region == f.region) {
                    continue;
                }
                here.push(f);
            }
            // Every guard in one tick of an object reads the state taken
            // before the region loop: a region's move this tick is visible to
            // its siblings, its parent and its children on the object's next
            // tick and never within the same one (model.md, the tick). A
            // scenario expecting two dependent moves expects two ticks.
            here.retain(|f| !self.reads_a_region_that_moved(&id, f, moved));
            if here.is_empty() {
                continue;
            }
            if !self.hold(&id) {
                continue;
            }
            // Nothing acts on an object its lease machine does not say it holds
            // (128, 150). The lease objects are applied first, so a lease taken
            // this pass is held by the time the object it names is written.
            if !self.holding(&id) {
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
            // What a scenario counts as a write is a write of the work: the
            // machinery's own objects tick alongside it and are not the race
            // (D5, 134).
            let counted = objects
                .get(&id)
                .map(|o| !flywheel_domain::leases::machinery(&o.machine))
                .unwrap_or(true);
            if counted {
                self.writes_attempted += 1;
            }
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
                Ok(PutOutcome::Written { .. }) => {
                    if counted {
                        self.writes_succeeded += 1;
                    }
                }
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
                // The order items merged into the bolt's line, which is the
                // order their ordinals state (38, 57).
                if f.to == "merged" && !self.store.merge_order.contains(&id) {
                    self.store.merge_order.push(id.clone());
                }
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
                    host: Some(self.store.me()),
                });
                for (region, e) in commanded.iter().map(|(r, e)| (r.as_str(), e)).chain(f.effects.iter().map(|e| (f.region.as_str(), e))) {
                    // A region performs each act once per tick, however many
                    // passes the tick settles over and by whichever transition
                    // asks for it: a state whose entry performed the act does
                    // not perform it again when its own self-transition calls
                    // it back in the same tick, and a state that performed
                    // nothing on entering performs it there and then (D4, D7,
                    // 127, S6, S7).
                    let once = format!("{id}#{region}#{}", e.name);
                    if moved.contains(&once) {
                        continue;
                    }
                    moved.push(once);
                    // The act the machine asked for, whether the world took it
                    // or refused it: a start of a name the multiplexer already
                    // holds is refused, and the machinery reads the refusal
                    // rather than being told nothing happened (72, 73, S6).
                    // What the world refused writes nothing.
                    let taken = world::perform(&self.defs, &mut self.store, &id, region, e);
                    let written = match taken {
                        true => self.write_effect(&id, region, e, f.note.as_deref()),
                        false => false,
                    };
                    record.effects.push(EffectRecord2 {
                        effect_id: e.id.clone(),
                        name: e.name.clone(),
                        object: id.clone(),
                        args: e.args.clone(),
                        written,
                        recalled: false,
                    });
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
        // A lease is the machinery's own object; nothing takes a lease on one.
        if flywheel_domain::leases::object_of(object).is_some() {
            return true;
        }
        let holder = self.store.me();
        // A host takes only what its declaration covers, and it reads the same
        // evidence the lease machine does (149). An object it does not cover
        // waits, and the machine says so under attention.
        let coverable = self
            .store
            .evidence(&flywheel_domain::leases::id_for(object), "hold", "lease.coverable")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        if !coverable {
            return false;
        }
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
                    coverable,
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
                    coverable,
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
                let parts: Vec<&str> = w
                    .effect_id
                    .strip_prefix(&format!("{}/", w.object))
                    .map(|rest| rest.split('/').collect())
                    .unwrap_or_default();
                let region = parts.first().copied().unwrap_or("life").to_string();
                // An act belongs to the state the object entered by it. Once
                // the object has left that state the act is not called back:
                // a session the operator ended is not started again because
                // its pane is gone (4, 73, 127, X01).
                let entered = parts
                    .get(1)
                    .and_then(|step| step.split("->").nth(1))
                    .unwrap_or_default();
                let still_there = self
                    .store
                    .get(&w.object)
                    .ok()
                    .flatten()
                    .and_then(|o| o.config.get(&region).cloned())
                    .is_some_and(|held| held == entered);
                if !still_there {
                    return None;
                }
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
            // A machine that calls an unproved effect back itself has said how
            // it reconciles: the runner's reconciler is for the effects no
            // transition of the state the object is in re-issues, and never a
            // second act on top of one (73, 127).
            if record.effects.iter().any(|e| e.object == object && e.name == effect)
                || self.machine_calls_back(&object, &effect)
            {
                continue;
            }
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
                    recalled: true,
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
        // The submission is the delivery: one response, already applied by the
        // capture it made. Through `put`, because the store is the state
        // repository and what only this process held would be gone at the next
        // fetch (92, 125, 137).
        let rid = self.dictate(&id, text, by);
        if let Ok(Some(mut held)) = Records::get(&self.store, &id) {
            if !held.applied_responses.contains(&rid) {
                held.applied_responses.push(rid.clone());
                let seq = held.seq;
                let _ = Records::put(&mut self.store, &id, &held, seq);
            }
        }
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

