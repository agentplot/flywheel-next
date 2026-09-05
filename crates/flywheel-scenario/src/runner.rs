//! Running the machinery over the store: tick, settle, respond, dictate.

use crate::store::Store;
use crate::world;
use chrono::Duration;
use flywheel_engine::runtime::{DecisionInstance, Response, ResponseKind, Snapshot};
use flywheel_engine::{plan, tick, Definitions};
use serde_json::{json, Value};
use std::collections::BTreeMap;

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
        self.store.play_services();
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
            for (region, e) in commanded.iter().map(|(r, e)| (r.as_str(), e)).chain(f.effects.iter().map(|e| (f.region.as_str(), e))) {
                world::perform(&self.defs, &mut self.store, &f.object, region, e);
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

    /// What one capture-box submission made: the object it created and what it was read as.
    pub fn captured(&self) -> Vec<Value> {
        let mut caps: Vec<&flywheel_engine::Object> = self.store.objects.values().filter(|o| o.machine == "capture").collect();
        caps.sort_by_key(|o| std::cmp::Reverse(o.created));
        caps.iter().map(|c| {
            let signals: Vec<Value> = self.store.objects.values().filter(|o| o.parent.as_deref() == Some(&c.id) && o.machine == "signal")
                .map(|s| json!({"id": s.id, "kind": s.record.get("kind"), "state": s.top_state(), "assertion": s.record.get("assertion")})).collect();
            json!({"id": c.id, "state": c.top_state(), "raw": c.record.get("raw"), "source": c.record.get("source"), "at": c.record.get("event_at"), "signals": signals})
        }).collect()
    }

    /// The page's capture box (19): plain text is a capture with one signal of kind ask; a prefix
    /// is a dictation that creates an intent, a chore unit on a repository's shared line, or a unit
    /// on a bolt. The submission is recorded once as a response naming the object it made.
    pub fn capture(&mut self, text: &str, by: &str) -> Value {
        let text = text.trim();
        let n = self.store.next_response;
        let now = self.store.now;
        let (id, kind): (String, &str) = if let Some(rest) = text.strip_prefix("intent:") {
            let subject = rest.trim();
            let id = format!("intent/{}", slug(subject));
            if !self.store.objects.contains_key(&id) {
                let rec = [("subject", json!(subject)), ("signals", json!([])), ("sources", json!(1)), ("source", json!("page"))].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
                world::new_object(&self.defs, &mut self.store, &id, "intent", None, rec);
            }
            (id, "intent")
        } else if let Some((repo, body)) = text.strip_prefix("chore ").and_then(|r| r.split_once(':')) {
            let (repo, body) = (repo.trim(), body.trim());
            let id = format!("unit/{repo}/chore-{}", slug(body));
            if !self.store.objects.contains_key(&id) {
                let rec = [("type", json!("chore")), ("repository", json!(repo)), ("items", json!(1)), ("target", json!({})), ("scope", json!("shared-line")),
                           ("batch", json!(format!("shared/{repo}"))), ("summary", json!(body)), ("source", json!("page"))].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
                world::new_object(&self.defs, &mut self.store, &id, "unit", None, rec);
            }
            (id, "chore")
        } else if let Some((name, body)) = text.strip_prefix("bolt ").and_then(|r| r.split_once(':')) {
            let (name, body) = (name.trim(), body.trim());
            let bolt = self.store.objects.values().find(|o| o.machine == "bolt" && (o.id == name || o.id.ends_with(&format!("/{name}")) || o.record.get("name").and_then(|v| v.as_str()) == Some(name))).cloned();
            let repo = bolt.as_ref().and_then(|b| b.record.get("repository")).and_then(|v| v.as_str()).unwrap_or("repo").to_string();
            let id = format!("unit/{repo}/{}", slug(body));
            if !self.store.objects.contains_key(&id) {
                let target = match &bolt { Some(b) => json!({"bolt": b.id}), None => json!({"new_name": name}) };
                let rec = [("type", json!("default")), ("repository", json!(repo)), ("items", json!(1)), ("target", target), ("summary", json!(body)), ("source", json!("page"))].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
                world::new_object(&self.defs, &mut self.store, &id, "unit", bolt.as_ref().map(|b| b.id.as_str()), rec);
            }
            (id, "unit")
        } else {
            let id = format!("capture/page-{n}");
            let rec = [("source", json!("page")), ("event_key", json!(format!("page-{n}"))), ("event_at", json!(now)), ("captured_by", json!(by)), ("raw", json!(text))].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
            world::new_object(&self.defs, &mut self.store, &id, "capture", None, rec);
            let srec: BTreeMap<String, Value> = [("kind", json!("ask")), ("asserted_by", json!(by)), ("assertion", json!(text)), ("excerpt", json!(text)), ("subject_tags", json!([])), ("argues_with", json!([]))].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
            world::new_object(&self.defs, &mut self.store, &format!("signal/page-{n}"), "signal", Some(&id), srec);
            (id, "capture")
        };
        // The submission is the delivery: one response, already applied by the object it made.
        let rid = self.dictate(&id, text, by);
        if let Some(o) = self.store.objects.get_mut(&id) { if !o.applied_responses.contains(&rid) { o.applied_responses.push(rid.clone()); } }
        self.store.log("capture", &id, format!("{kind} from the page"));
        json!({"id": id, "kind": kind, "response": rid})
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

/// A short id segment from free text: lowercase words joined by dashes, at most 40 characters.
fn slug(text: &str) -> String {
    let s: String = text.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.len() > 40 { s[..40].trim_end_matches('-').to_string() } else if s.is_empty() { "untitled".into() } else { s }
}
