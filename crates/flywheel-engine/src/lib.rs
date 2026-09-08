//! flywheel-engine: the generic part. Loads machine definitions, evaluates
//! guards over evidence, plans and applies ticks, derives the rail's decisions. It
//! contains no domain name and no store: evidence and effects are strings a
//! profile binds.

pub mod defs;
pub mod eval;
pub mod load;
pub mod rail;
pub mod rec;
pub mod runtime;
pub mod tick;

pub use defs::{Definitions, Machine};
pub use runtime::{DecisionInstance, Object, Register, Response, ResponseKind, Snapshot, TailEntry};
pub use tick::{apply, plan_tick, Fired, PlannedEffect};

/// Put a fresh object into its machine's initial configuration.
pub fn initialise(defs: &Definitions, obj: &mut Object, now: chrono::DateTime<chrono::Utc>) {
    let Some(m) = defs.for_object(&obj.machine).or_else(|| defs.get(&obj.machine)) else { return };
    let m = m.clone();
    for (rname, reg) in &m.regions {
        if obj.config.contains_key(rname) { continue; }
        obj.config.insert(rname.clone(), reg.initial.clone());
        obj.entered_at.insert(rname.clone(), now);
    }
    // Nested regions and submachines of every top-level initial state.
    let tops: Vec<(String, String)> = obj.config.iter().filter(|(k, _)| !k.contains('.')).map(|(k, v)| (k.clone(), v.clone())).collect();
    for (rname, sname) in tops {
        if let Some(st) = m.regions.get(&rname).and_then(|r| r.states.get(&sname)) {
            let st = st.clone();
            let path = format!("{rname}.{sname}");
            init_nested_pub(defs, obj, &path, &st, now);
        }
    }
}

fn init_nested_pub(defs: &Definitions, obj: &mut Object, path: &str, st: &defs::State, now: chrono::DateTime<chrono::Utc>) {
    let mut regions: Vec<(String, defs::Region)> = st.regions.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    if let Some(sub) = tick::submachine(defs, st, obj) {
        for (k, v) in &sub.regions { regions.push((k.clone(), v.clone())); }
    }
    for (rname, reg) in regions {
        let rpath = format!("{path}.{rname}");
        if obj.config.contains_key(&rpath) { continue; }
        obj.config.insert(rpath.clone(), reg.initial.clone());
        obj.entered_at.insert(rpath.clone(), now);
        if let Some(init) = reg.states.get(&reg.initial) {
            init_nested_pub(defs, obj, &format!("{rpath}.{}", reg.initial), init, now);
        }
    }
}
