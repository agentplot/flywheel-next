//! Seeding and driving a scenario. The scenario file's own types live in
//! `flywheel-atoms` (94); this is the stand-in's way of playing one.

use crate::runner::Runtime;
use crate::store::Store;
use crate::world;
use flywheel_engine::Definitions;
use serde_json::Value;
use std::collections::BTreeMap;

pub use flywheel_atoms::scenario::{load, Given, GivenObject, Scenario};

/// Refuse a described state whose line no repository holds, naming what is
/// missing.
///
/// A line belongs to a repository — a unit's and a bolt's to the one they name,
/// an intent's to the blueprints — and a seed that says a line is there when
/// the instance tracks no repository to hold it describes work no host can act
/// on. It is refused the way an uncovered object is, saying what would have to
/// be tracked first, rather than seeding a line that stands for nothing
/// (19.6, 125, 149).
pub fn lines_held(sc: &Scenario, tracked: &[String]) -> anyhow::Result<()> {
    let mut missing: Vec<String> = Vec::new();
    for g in &sc.given.objects {
        let Some(state) = g.state.get("line") else { continue };
        if matches!(state.as_str(), "absent" | "removed") {
            continue;
        }
        // The blueprints hold every intent's line, and the instance always has
        // them: only a line naming a built repository can be missing one.
        let Some(repository) = g.record.get("repository").and_then(|v| v.as_str()) else {
            continue;
        };
        if tracked.iter().any(|held| held == repository) {
            continue;
        }
        let named = format!("`{repository}` (the line of {})", g.id);
        if !missing.contains(&named) {
            missing.push(named);
        }
    }
    if missing.is_empty() {
        return Ok(());
    }
    anyhow::bail!(
        "this instance tracks no repository to hold {}. A line is a branch of a repository, so \
         nothing is seeded until the instance tracks what the described state names \
         (`flywheel.yaml` repositories, 149, 205, 206)",
        missing.join(", ")
    )
}

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
        // A line a described state says is there is there. The state is the
        // machine's and the fact is the world's, and a guard reads the fact:
        // without it an elaboration approved on a seeded intent waits on a line
        // nothing made, and the scenario cannot run past it. A seed covers what
        // it seeds (19.6, 125, 149).
        let held = store.objects.get(&g.id).and_then(|o| o.config.get("line").cloned());
        if let Some(state) = held {
            if !matches!(state.as_str(), "absent" | "removed") {
                let line = store.world.lines.entry(g.id.clone()).or_default();
                line.exists = true;
                line.absent = false;
                line.landed = state == "landed";
            }
        }
    }
    // An item works its unit's type at the version the unit recorded (57), and
    // `create_items` writes both on to the item when it makes one. A scenario
    // that describes items directly need not repeat them.
    let types: Vec<(String, Value, Value)> = store
        .objects
        .values()
        .filter(|o| o.machine == "work-item" && !o.record.contains_key("type"))
        .filter_map(|o| {
            let unit = store.objects.get(o.parent.as_deref()?)?;
            Some((
                o.id.clone(),
                unit.record.get("type")?.clone(),
                unit.record
                    .get("type_version")
                    .cloned()
                    .unwrap_or(Value::Null),
            ))
        })
        .collect();
    for (id, kind, version) in types {
        if let Some(item) = store.objects.get_mut(&id) {
            item.record.insert("type".into(), kind);
            if !version.is_null() {
                item.record.insert("type_version".into(), version);
            }
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
