//! Running the machinery over the store: tick, settle, respond, dictate.

use crate::store::Store;
use crate::world;
use chrono::Duration;
use flywheel_engine::runtime::{DecisionInstance, Response, ResponseKind, Snapshot};
use flywheel_engine::{rail, tick, Definitions};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub struct Runtime {
    pub defs: Definitions,
    pub store: Store,
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
        Runtime { defs, store }
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
        let d = rail::derive(&self.defs, &self.store.objects, &mut self.store.register);
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
        for _ in 0..50 {
            let pass = self.pass();
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
    fn pass(&mut self) -> TickRecord {
        self.store.tick += 1;
        // Alive hosts heartbeat: their last_seen is now.
        let now = self.store.now;
        let hosts: Vec<String> = self.store.objects.values().filter(|o| o.machine == "host" && o.record.get("alive").and_then(|v| v.as_bool()).unwrap_or(true)).map(|o| o.id.clone()).collect();
        for h in hosts { self.store.set_given(&h, "host.last_seen", json!(now.to_rfc3339())); }
        self.store.play_scripts();
        self.store.play_services();
        let fired = {
            let snap = Snapshot { objects: &self.store.objects, responses: &self.store.responses, register: &self.store.register, evidence: &self.store, now: self.store.now };
            tick::plan_tick(&self.defs, &snap)
        };
        let mut record = TickRecord { tick: self.store.tick, at: self.store.now, ..Default::default() };
        for f in &fired {
            let commanded = self.store.objects.get(&f.object).map(|o| tick::commanded_effects(&self.defs, o, f)).unwrap_or_default();
            let mut tail = Vec::new();
            if let Some(o) = self.store.objects.get_mut(&f.object) {
                tail = tick::apply(&self.defs, o, f, self.store.now);
            }
            self.store.log("transition", &f.object, format!("{}: {} → {}{}", f.region, f.from, f.to, f.note.as_ref().map(|n| format!(" — {n}")).unwrap_or_default()));
            // The guard that matched, as the trace states it.
            record.guards.push(GuardRecord {
                object: f.object.clone(),
                region: f.region.clone(),
                from: f.from.clone(),
                matched: f.note.clone().unwrap_or_else(|| format!("→ {}", f.to)),
                response: f.response.as_ref().map(|(id, _)| id.clone()),
            });
            record.transitions.push(TransitionRecord {
                object: f.object.clone(),
                region: f.region.clone(),
                from: f.from.clone(),
                to: f.to.clone(),
                response: f.response.as_ref().map(|(id, _)| id.clone()),
                reason: f.note.clone(),
            });
            for (region, e) in commanded.iter().map(|(r, e)| (r.as_str(), e)).chain(f.effects.iter().map(|e| (f.region.as_str(), e))) {
                // Only an act the world actually performed is an effect the
                // scenario counts (72, 73).
                if world::perform(&self.defs, &mut self.store, &f.object, region, e) {
                    record.effects.push(EffectRecord2 {
                        effect_id: e.id.clone(),
                        name: e.name.clone(),
                        object: f.object.clone(),
                        args: e.args.clone(),
                    });
                }
            }
            self.store.tail.extend(tail);
        }
        record.decisions = self
            .decisions()
            .iter()
            .map(|d| DecisionRecord { id: d.id.clone(), object: d.object.clone(), kind: d.kind.clone(), number: d.number })
            .collect();
        record
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

