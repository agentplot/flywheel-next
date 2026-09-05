//! Scenario files: `given` seeds the store, `when` drives it.

use crate::store::{ScriptEntry, ServiceDecl, SessionFact, Store};
use crate::world;
use crate::runner::Runtime;
use anyhow::{Context, Result};
use flywheel_engine::Definitions;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct Scenario {
    pub scenario: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub given: Given,
    #[serde(default)]
    pub when: Vec<BTreeMap<String, Value>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Given {
    #[serde(default)]
    pub objects: Vec<GivenObject>,
    #[serde(default)]
    pub evidence: BTreeMap<String, BTreeMap<String, Value>>,
    #[serde(default)]
    pub script: BTreeMap<String, Vec<ScriptEntry>>,
    #[serde(default)]
    pub sessions: BTreeMap<String, SessionFact>,
    /// Service declarations per repository: what `.flywheel/services.yaml` would say.
    #[serde(default)]
    pub services: BTreeMap<String, Vec<ServiceDecl>>,
    #[serde(default)]
    pub hosts: Vec<BTreeMap<String, Value>>,
    #[serde(default)]
    pub tail: Vec<flywheel_engine::TailEntry>,
    #[serde(default)]
    pub now: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub register_start: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct GivenObject {
    pub id: String,
    pub machine: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub state: BTreeMap<String, String>,
    #[serde(default)]
    pub record: BTreeMap<String, Value>,
    #[serde(default)]
    pub entered: Option<String>,
}

pub fn load(path: &std::path::Path) -> Result<Scenario> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(serde_yaml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?)
}

pub fn seed(defs: Definitions, sc: &Scenario) -> Runtime {
    let mut store = Store::default();
    store.scenario = Some(sc.scenario.clone());
    if let Some(n) = sc.given.now { store.now = n; }
    if let Some(n) = sc.given.register_start { store.register.next_number = n; }
    store.host_bound = 0;
    for h in &sc.given.hosts {
        let alive = h.get("alive").and_then(|v| v.as_bool()).unwrap_or(true);
        if let Some(b) = h.get("bound").and_then(|v| v.as_u64()) { if alive { store.host_bound += b as usize; } }
        if let Some(id) = h.get("id").and_then(|v| v.as_str()) {
            let mut rec: BTreeMap<String, Value> = h.clone();
            rec.remove("id");
            world::new_object(&defs, &mut store, &format!("host/{id}"), "host", None, rec);
        }
    }
    // evidence given as name -> {object -> value}
    for (name, per) in &sc.given.evidence {
        for (obj, v) in per {
            store.set_given(obj, name, v.clone());
        }
    }
    store.world.script = sc.given.script.clone();
    store.world.sessions = sc.given.sessions.clone();
    store.world.declarations = sc.given.services.clone();
    store.tail = sc.given.tail.clone();
    for g in &sc.given.objects {
        world::new_object(&defs, &mut store, &g.id, &g.machine, g.parent.as_deref(), g.record.clone());
        let entered = g.entered.as_deref().and_then(flywheel_engine::eval::parse_duration).map(|d| store.now - d).unwrap_or(store.now);
        if let Some(o) = store.objects.get_mut(&g.id) {
            // Top-level first so nested initialisation follows the given state.
            let mut paths: Vec<(&String, &String)> = g.state.iter().collect();
            paths.sort_by_key(|(k, _)| k.matches('.').count());
            for (path, st) in paths {
                // drop nested paths under the state being replaced
                let prefix = format!("{path}.");
                let gone: Vec<String> = o.config.keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
                for k in gone { o.config.remove(&k); }
                o.config.insert(path.clone(), st.clone());
                o.entered_at.insert(path.clone(), entered);
            }
            let snapshot = o.clone();
            let mut o2 = snapshot;
            flywheel_engine::initialise(&defs, &mut o2, entered);
            *o = o2;
        }
    }
    let mut rt = Runtime::new(defs, store);
    rt.decisions();
    rt
}

/// Run the `when` steps of a scenario against a seeded runtime.
pub fn drive(rt: &mut Runtime, sc: &Scenario) {
    for step in &sc.when {
        for (k, v) in step {
            match k.as_str() {
                "tick" => {
                    let n = v.get("n").and_then(|x| x.as_u64()).unwrap_or(1);
                    for _ in 0..n { rt.tick(); }
                }
                "settle" => { rt.settle(50); }
                "response" => {
                    let answer = v.get("answer").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    let by = v.get("by").and_then(|x| x.as_str()).unwrap_or("operator").to_string();
                    if let Some(n) = v.get("number").and_then(|x| x.as_u64()) {
                        rt.respond(n as u32, &answer, &by);
                    } else if let Some(obj) = v.get("object").and_then(|x| x.as_str()) {
                        rt.dictate(obj, &answer, &by);
                    }
                    rt.settle(50);
                }
                "evidence" => {
                    if let Some(m) = v.as_object() {
                        for (name, per) in m {
                            if let Some(per) = per.as_object() {
                                for (obj, val) in per { rt.store.set_given(obj, name, val.clone()); }
                            }
                        }
                    }
                }
                "clock" => {
                    if let Some(a) = v.get("advance").and_then(|x| x.as_str()).and_then(flywheel_engine::eval::parse_duration) {
                        rt.store.now = rt.store.now + a;
                    }
                }
                _ => {}
            }
        }
    }
}
