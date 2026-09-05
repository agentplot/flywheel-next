//! The tick: for each object, for each active region innermost first, the
//! first transition whose guard holds fires. `plan_tick` is pure; `apply`
//! mutates one object and returns the effects to perform.

use crate::defs::{Definitions, Effect, Machine, Region, State};
use crate::eval::{eval, evidence, truthy, Ctx};
use crate::runtime::{Object, Snapshot, TailEntry};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// One transition the planner decided to fire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fired {
    pub object: String,
    pub region: String,
    pub from: String,
    pub to: String,
    pub note: Option<String>,
    pub bump: Option<String>,
    pub enter: BTreeMap<String, String>,
    /// The response consumed, and the argument it bound to `$response`.
    pub response: Option<(String, String)>,
    /// Effects to perform, in order: exit of `from`, the transition's, entry of `to`. Proven ones are left out.
    pub effects: Vec<PlannedEffect>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedEffect {
    pub id: String,
    pub name: String,
    pub args: BTreeMap<String, Value>,
    pub note: Option<String>,
}

/// Resolve the definition of the state at `region` for this object, walking
/// nested regions and instantiated submachines.
pub fn state_def<'a>(defs: &'a Definitions, obj: &Object, region: &str) -> Option<(&'a Region, &'a State)> {
    let machine = defs.for_object(&obj.machine).or_else(|| defs.get(&obj.machine))?;
    let mut parts = region.split('.');
    let top = parts.next()?;
    let mut reg: &Region = machine.regions.get(top)?;
    let mut state_name = obj.config.get(top)?.as_str();
    let mut st: &State = reg.states.get(state_name)?;
    let mut path = top.to_string();
    // Alternating: state, region, state, region...
    let mut rest: Vec<&str> = parts.collect();
    while !rest.is_empty() {
        let s = rest.remove(0);
        if s != state_name { return None; }
        let r = rest.first()?.to_string();
        rest.remove(0);
        path = format!("{path}.{s}.{r}");
        let next_reg: &Region = if let Some(nr) = st.regions.get(&r) {
            nr
        } else {
            let sub = submachine(defs, st, obj)?;
            sub.regions.get(&r)?
        };
        reg = next_reg;
        state_name = obj.config.get(&path)?.as_str();
        st = reg.states.get(state_name)?;
    }
    Some((reg, st))
}

/// The machine a state instantiates, resolving `$record.field@$record.other` references.
pub fn submachine<'a>(defs: &'a Definitions, st: &State, obj: &Object) -> Option<&'a Machine> {
    let name = st.machine.as_deref()?;
    let resolved = resolve_ref(name, obj);
    defs.get(&resolved).or_else(|| defs.for_object(&resolved))
}

fn resolve_ref(name: &str, obj: &Object) -> String {
    let base = name.split('@').next().unwrap_or(name);
    if let Some(field) = base.strip_prefix('$') {
        // `$unit.type` → record field `type`
        let field = field.rsplit('.').next().unwrap_or(field);
        if let Some(Value::String(s)) = obj.record.get(field) {
            return s.clone();
        }
        return field.to_string();
    }
    base.to_string()
}

/// Active region paths of an object, innermost first.
pub fn active_regions(obj: &Object) -> Vec<String> {
    let mut v: Vec<String> = obj.config.keys().cloned().collect();
    v.sort_by(|a, b| b.matches('.').count().cmp(&a.matches('.').count()).then(a.cmp(b)));
    v
}

/// Decide what fires this tick. Pure: reads the snapshot, writes nothing.
pub fn plan_tick(defs: &Definitions, snap: &Snapshot) -> Vec<Fired> {
    let mut fired = Vec::new();
    let mut objs: Vec<&Object> = snap.objects.values().collect();
    objs.sort_by_key(|o| o.created);
    for obj in objs {
        let mut regions_fired: Vec<String> = Vec::new();
        for region in active_regions(obj) {
            // A region under a state that already left this tick does not fire.
            if regions_fired.iter().any(|r| region.starts_with(&format!("{r}."))) { continue; }
            let Some((_reg, st)) = state_def(defs, obj, &region) else { continue };
            let from = obj.config.get(&region).cloned().unwrap_or_default();
            for t in &st.transitions {
                let cx = Ctx { defs, snap, object: obj, region: &region, state_name: &from, state: st };
                let h = eval(&t.when, &cx);
                if !h.holds { continue; }
                let arg = h.response.as_ref().map(|(_, a)| a.clone()).unwrap_or_default();
                // Self-transition with nothing to do writes nothing.
                let target_def = _reg.states.get(&t.to);
                let mut effects = Vec::new();
                let mut push = |list: &[Effect], phase: &str| {
                    for (i, e) in list.iter().enumerate() {
                        if let Some(proof) = defs.atoms.proof_of(&e.name) {
                            if let Some(v) = evidence(&cx, &proof) {
                                if truthy(&v) { continue; }
                            }
                        }
                        let mut args = BTreeMap::new();
                        if let Some(a) = &e.args {
                            for (k, v) in a {
                                args.insert(k.clone(), substitute(v, &arg, obj));
                            }
                        }
                        effects.push(PlannedEffect {
                            id: format!("{}/{}/{}->{}/{}/{}{}", obj.id, region, from, t.to, phase, e.name, i),
                            name: e.name.clone(),
                            args,
                            note: e.note.clone(),
                        });
                    }
                };
                if t.to != from { push(&st.exit, "exit"); }
                push(&t.effects, "do");
                if t.to != from {
                    if let Some(td) = target_def { push(&td.entry, "entry"); }
                }
                if t.to == from && effects.is_empty() && h.response.is_none() && t.bump.is_none() && t.enter.is_empty() {
                    break; // quiescent: reading twice writes nothing
                }
                fired.push(Fired {
                    object: obj.id.clone(),
                    region: region.clone(),
                    from: from.clone(),
                    to: t.to.clone(),
                    note: t.note.clone(),
                    bump: t.bump.clone(),
                    enter: t.enter.clone(),
                    response: h.response.clone(),
                    effects,
                });
                regions_fired.push(region.clone());
                break;
            }
        }
    }
    fired
}

fn substitute(v: &Value, arg: &str, obj: &Object) -> Value {
    match v {
        Value::String(s) if s == "$response" => Value::String(arg.to_string()),
        Value::String(s) if s.starts_with('$') => {
            let field = s.trim_start_matches('$').rsplit('.').next().unwrap_or("");
            obj.record.get(field).cloned().unwrap_or(Value::String(s.clone()))
        }
        Value::String(s) => {
            // `target.new_name`, `parent.repository`: record paths
            let mut it = s.split('.');
            match (it.next(), it.next()) {
                (Some(a), Some(b)) if obj.record.get(a).and_then(|x| x.get(b)).is_some() => obj.record[a][b].clone(),
                _ => v.clone(),
            }
        }
        _ => v.clone(),
    }
}

/// Apply one fired transition to its object. Returns tail entries created.
pub fn apply(defs: &Definitions, obj: &mut Object, f: &Fired, now: DateTime<Utc>) -> Vec<TailEntry> {
    let mut tail = Vec::new();
    // Commanded states (`enter:`): set them now; their entry effects were planned by `commanded_effects`.
    for c in commands(defs, obj, f) {
        let sub = format!("{}.", c.path);
        let cur = obj.config.get(&c.path).cloned().unwrap_or_default();
        if cur == c.target { continue; }
        let gone: Vec<String> = obj.config.keys().filter(|k| k.starts_with(&sub)).cloned().collect();
        for k in gone { obj.config.remove(&k); obj.entered_at.remove(&k); }
        obj.config.insert(c.path.clone(), c.target.clone());
        obj.entered_at.insert(c.path.clone(), now);
        if let Some((reg, _)) = state_def(defs, obj, &c.path) {
            if let Some(ts) = reg.states.get(&c.target) {
                let ts = ts.clone();
                init_nested(defs, obj, &format!("{}.{}", c.path, c.target), &ts, now);
            }
        }
    }
    let prefix = format!("{}.{}.", f.region, f.from);
    if f.to != f.from {
        let gone: Vec<String> = obj.config.keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
        for k in gone { obj.config.remove(&k); obj.entered_at.remove(&k); }
    }
    obj.config.insert(f.region.clone(), f.to.clone());
    if f.to != f.from { obj.entered_at.insert(f.region.clone(), now); }
    if let Some(b) = &f.bump { *obj.counters.entry(b.clone()).or_insert(0) += 1; }
    if let Some((id, _)) = &f.response { if !obj.applied_responses.contains(id) { obj.applied_responses.push(id.clone()); } }
    obj.seq += 1;
    // Initialise nested regions and submachines of the target.
    if f.to != f.from {
        if let Some((_reg, st)) = state_def(defs, obj, &f.region) {
            let st = st.clone();
            init_nested(defs, obj, &format!("{}.{}", f.region, f.to), &st, now);
            if let Some(t) = &st.tail {
                tail.push(TailEntry { at: now, object: obj.id.clone(), kind: t.clone(), state: f.to.clone(), by: f.response.as_ref().map(|(id, _)| id.clone()) });
            }
        }
    }
    tail
}

fn init_nested(defs: &Definitions, obj: &mut Object, path: &str, st: &State, now: DateTime<Utc>) {
    let mut regions: Vec<(String, Region)> = st.regions.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    if let Some(sub) = submachine(defs, st, obj) {
        for (k, v) in &sub.regions { regions.push((k.clone(), v.clone())); }
    }
    for (rname, reg) in regions {
        let rpath = format!("{path}.{rname}");
        obj.config.insert(rpath.clone(), reg.initial.clone());
        obj.entered_at.insert(rpath.clone(), now);
        if let Some(init) = reg.states.get(&reg.initial) {
            init_nested(defs, obj, &format!("{rpath}.{}", reg.initial), init, now);
        }
    }
}

/// A state commanded by a transition's `enter:` map: the region path to set and the state to put it in.
#[derive(Debug, Clone)]
pub struct Command { pub path: String, pub target: String }

/// Resolve `enter:` keys against the object's configuration. A key names a region (its last
/// segment), the state that holds a submachine (`place`, `line`, `session`, `sessions`), or the
/// submachine's machine name.
pub fn commands(defs: &Definitions, obj: &Object, f: &Fired) -> Vec<Command> {
    let mut out = Vec::new();
    for (key, target) in &f.enter {
        for (path, _cur) in obj.config.iter() {
            let segs: Vec<&str> = path.split('.').collect();
            let last = *segs.last().unwrap_or(&"");
            let parent_state = if segs.len() >= 2 { segs[segs.len() - 2] } else { "" };
            let by_machine = if segs.len() >= 3 {
                let parent_region = segs[..segs.len() - 2].join(".");
                state_def(defs, obj, &parent_region).and_then(|(_, st)| st.machine.clone()).map(|m| m == *key).unwrap_or(false)
            } else { false };
            let matches = last == key || parent_state == key || parent_state == format!("{key}s") || by_machine;
            if !matches { continue; }
            let Some((reg, _st)) = state_def(defs, obj, path) else { continue };
            if !reg.states.contains_key(target) { continue; }
            // Only the outermost matching region per key: a deeper match under it is left to init.
            if out.iter().any(|c: &Command| path.starts_with(&format!("{}.", c.path))) { continue; }
            out.push(Command { path: path.clone(), target: target.clone() });
        }
    }
    out
}

/// The entry effects of commanded states, planned before `apply` moves them.
pub fn commanded_effects(defs: &Definitions, obj: &Object, f: &Fired) -> Vec<PlannedEffect> {
    let mut out = Vec::new();
    for c in commands(defs, obj, f) {
        let Some((reg, _)) = state_def(defs, obj, &c.path) else { continue };
        if let Some(ts) = reg.states.get(&c.target) {
            for (i, e) in ts.entry.iter().enumerate() {
                out.push(PlannedEffect {
                    id: format!("{}/{}/enter/{}/{}{}", obj.id, c.path, c.target, e.name, i),
                    name: e.name.clone(),
                    args: e.args.clone().unwrap_or_default(),
                    note: e.note.clone(),
                });
            }
        }
    }
    out
}
