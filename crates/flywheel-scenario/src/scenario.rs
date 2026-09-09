//! Seeding and driving a scenario. The scenario file's own types live in
//! `flywheel-atoms` (94); this is the stand-in's way of playing one.

use crate::runner::Runtime;
use crate::store::Store;
use crate::world;
use flywheel_engine::Definitions;
use serde_json::Value;
use std::collections::BTreeMap;

pub use flywheel_atoms::scenario::{load, Given, GivenObject, Scenario};

pub fn seed(defs: Definitions, sc: &Scenario) -> Runtime {
    let mut store = Store::default();
    store.scenario = Some(sc.scenario.clone());
    if let Some(n) = sc.given.now { store.now = n; }
    if let Some(n) = sc.given.register_start { store.register.next_number = n; }
    store.host_bound = 0;
    for h in &sc.given.hosts {
        let alive = h.get("alive").and_then(|v| v.as_bool()).unwrap_or(true);
        // What the host takes leases within (149, 217). A host that declares
        // nothing takes everything, which is what a scenario with no
        // `declares:` describes.
        if let Some(name) = h.get("name").or_else(|| h.get("id")).and_then(|v| v.as_str()) {
            if let Some(declares) = h.get("declares") {
                let list = |key: &str| -> Vec<String> {
                    declares
                        .get(key)
                        .and_then(|v| v.as_array())
                        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
                        .unwrap_or_default()
                };
                store.declarations.insert(
                    name.to_string(),
                    flywheel_domain::derived::Declaration {
                        repositories: list("repositories"),
                        types: list("unit_types"),
                        kinds: list("kinds"),
                    },
                );
            }
        }
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
