//! Running the machinery over the store: tick, settle, respond, dictate.

use crate::store::Store;
use crate::world;
use chrono::Duration;
use flywheel_engine::runtime::{DecisionInstance, Response, ResponseKind, Snapshot};
use flywheel_engine::{plan, tick, Definitions};
use serde_json::json;

pub struct Runtime {
    pub defs: Definitions,
    pub store: Store,
}

impl Runtime {
    pub fn new(defs: Definitions, store: Store) -> Self {
        Runtime { defs, store }
    }

    /// The standing decisions, numbered.
    pub fn decisions(&mut self) -> Vec<DecisionInstance> {
        let d = plan::derive(&self.defs, &self.store.objects, &mut self.store.register);
        self.store.standing = d.iter().map(|x| x.id.clone()).collect();
        d
    }

    /// One pass: plan, apply, perform. Returns how many transitions fired.
    pub fn tick(&mut self) -> usize {
        self.store.tick += 1;
        // Alive hosts heartbeat: their last_seen is now.
        let now = self.store.now;
        let hosts: Vec<String> = self.store.objects.values().filter(|o| o.machine == "host" && o.record.get("alive").and_then(|v| v.as_bool()).unwrap_or(true)).map(|o| o.id.clone()).collect();
        for h in hosts { self.store.set_given(&h, "host.last_seen", json!(now.to_rfc3339())); }
        self.store.play_scripts();
        let fired = {
            let snap = Snapshot { objects: &self.store.objects, responses: &self.store.responses, register: &self.store.register, evidence: &self.store, now: self.store.now };
            tick::plan_tick(&self.defs, &snap)
        };
        let n = fired.len();
        for f in &fired {
            let commanded = self.store.objects.get(&f.object).map(|o| tick::commanded_effects(&self.defs, o, f)).unwrap_or_default();
            let mut tail = Vec::new();
            if let Some(o) = self.store.objects.get_mut(&f.object) {
                tail = tick::apply(&self.defs, o, f, self.store.now);
            }
            self.store.log("transition", &f.object, format!("{}: {} → {}{}", f.region, f.from, f.to, f.note.as_ref().map(|n| format!(" — {n}")).unwrap_or_default()));
            for e in commanded.iter().chain(f.effects.iter()) {
                world::perform(&self.defs, &mut self.store, &f.object, &f.region, e);
            }
            self.store.tail.extend(tail);
        }
        self.decisions();
        self.store.now = self.store.now + Duration::seconds(self.store.tick_seconds);
        n
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
