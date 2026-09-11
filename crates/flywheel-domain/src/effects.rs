//! The effects whose whole work is records, performed over the state store.
//!
//! `profiles/record-derived.yaml` states every evidence name that is a function
//! of an object's record and thread once, so it is the same in every profile
//! (D3). The effects below are the other half of that: an act whose only
//! outcome is a record is the same act in every profile too, so it is written
//! here — over `StateStore` and nothing else — and the host and the conformance
//! runner perform one implementation (125, 193, D1, D8).
//!
//! What is *not* here is any effect whose proof `host.yaml` binds to a real
//! place or a real line: a service on a worktree, a change directory on the
//! intent's line. Those are the host binding's, and phase 1 records the
//! line-and-place effects rather than performing them (93a), so they are
//! refused in the open rather than bound to a fact nothing reads
//! (`host.yaml` evidence, task 16.1).

use crate::commands;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use flywheel_atoms::{Records, Scope, StateStore};
use flywheel_engine::{Definitions, Object};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The id of a new object under a parent: an object's id is
/// `<machine>/<the parent's name>/<its own name>`, so a work item of
/// `unit/atlas/rail-tail` is `work-item/atlas/rail-tail/wi-1`.
pub fn id_under(parent: &str, machine: &str, name: &str) -> String {
    let stem = parent.split_once('/').map(|(_, rest)| rest).unwrap_or(parent);
    format!("{machine}/{stem}/{name}")
}

/// One field of an object's record, written back through `put` at the sequence
/// it was read at (135).
fn amend<S: StateStore>(
    store: &mut S,
    id: &str,
    change: impl FnOnce(&mut Object),
) -> Result<bool> {
    let Some(mut object) = store.get(id)? else {
        return Ok(false);
    };
    let base = object.seq;
    change(&mut object);
    store.put(id, &object, base).with_context(|| format!("writing {id}"))?;
    Ok(true)
}

/// A unit's `target`, as a map.
fn target_of(object: &Object) -> serde_json::Map<String, Value> {
    object
        .record
        .get("target")
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// The objects of one machine whose parent is this one.
pub fn children<S: Records>(store: &S, parent: &str, machine: &str) -> Result<Vec<Object>> {
    Ok(store
        .list_records(&Scope::All)?
        .into_iter()
        .filter(|o| o.machine == machine && o.parent.as_deref() == Some(parent))
        .collect())
}

// --------------------------------------------------------------- the unit

/// `create_items`: one work item per task of the approved document, with the
/// type version in force (13, 36, 57, `atoms.yaml` create_items).
///
/// Phase 1 has no unit proposal document — 17 waits on the runner — so the
/// count is the unit's own `items` field where the proposal recorded one and
/// one item otherwise. A unit marked `serial` gives each item a dependency on
/// the one before it, which is what makes the ordinals of 31 and 32 mean
/// something.
pub fn create_items<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    unit: &str,
    at: DateTime<Utc>,
) -> Result<usize> {
    if !children(store, unit, "work-item")?.is_empty() {
        return Ok(0);
    }
    let Some(held) = store.get(unit)? else {
        return Ok(0);
    };
    let how_many = held
        .record
        .get("items")
        .and_then(|v| v.as_u64())
        .unwrap_or(1)
        .max(1) as usize;
    let kind = held.record.get("type").cloned().unwrap_or(json!("default"));
    let version = held.record.get("type_version").cloned();
    let serial = held
        .record
        .get("serial")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut made = 0;
    for ordinal in 1..=how_many {
        let id = id_under(unit, "work-item", &format!("wi-{ordinal}"));
        let mut record: BTreeMap<String, Value> = BTreeMap::new();
        record.insert("ordinal".into(), json!(ordinal));
        record.insert("type".into(), kind.clone());
        if let Some(version) = &version {
            record.insert("type_version".into(), version.clone());
        }
        if serial && ordinal > 1 {
            let before = id_under(unit, "work-item", &format!("wi-{}", ordinal - 1));
            record.insert("depends_on".into(), json!([before]));
        }
        commands::put_new(store, defs, &id, "work-item", Some(unit), record, at)?;
        made += 1;
    }
    Ok(made)
}

/// `create_bolt`: the new bolt a unit's target names, made once however many
/// units name it (16, 29, I8).
///
/// A unit whose target already names a bolt makes none; several units of one
/// proposal naming the same new bolt make one, because the id is derived from
/// the repository and the name and not from the unit.
pub fn create_bolt<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    unit: &str,
    name: &str,
    repository: &str,
    at: DateTime<Utc>,
) -> Result<Option<String>> {
    let Some(held) = store.get(unit)? else {
        return Ok(None);
    };
    let target = target_of(&held);
    if target
        .get("bolt")
        .and_then(|v| v.as_str())
        .is_some_and(|b| !b.is_empty())
    {
        return Ok(None);
    }
    let name = match name.is_empty() {
        true => target
            .get("new_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        false => name.to_string(),
    };
    if name.is_empty() {
        return Ok(None);
    }
    let repository = match repository.is_empty() {
        true => held
            .record
            .get("repository")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        false => repository.to_string(),
    };
    let bolt = format!("bolt/{repository}/{name}");
    if store.get(&bolt)?.is_none() {
        let record: BTreeMap<String, Value> = [
            ("repository".to_string(), json!(repository)),
            ("name".to_string(), json!(name)),
        ]
        .into_iter()
        .collect();
        commands::put_new(store, defs, &bolt, "bolt", None, record, at)?;
    }
    // The unit belongs to the bolt now, and its target names it rather than a
    // name that has yet to exist.
    amend(store, unit, |unit| {
        unit.parent = Some(bolt.clone());
        let mut target = target_of(unit);
        target.insert("bolt".into(), json!(bolt));
        target.remove("new_name");
        unit.record.insert("target".into(), Value::Object(target));
    })?;
    Ok(Some(bolt))
}

/// `route_unit`: the unit's target becomes the bolt the response named (16).
///
/// A name that matches a bolt already recorded routes to that bolt; one that
/// matches none is a new bolt, held as `new_name` until the approval makes it
/// (`unit.yaml` in-proposal, `record-derived.yaml` unit.routed).
pub fn route_unit<S: StateStore>(store: &mut S, unit: &str, named: &str) -> Result<()> {
    // `bolt <name>` and `new bolt <name>` both arrive as the response's
    // argument; what matters is the name at the end of it.
    let name = named
        .trim()
        .trim_start_matches("new bolt")
        .trim_start_matches("bolt")
        .trim()
        .to_string();
    if name.is_empty() {
        return Ok(());
    }
    let existing = store
        .list_records(&Scope::Machine("bolt".into()))?
        .into_iter()
        .find(|bolt| {
            bolt.id == name
                || bolt.record.get("name").and_then(|v| v.as_str()) == Some(name.as_str())
                || bolt.id.rsplit('/').next() == Some(name.as_str())
        })
        .map(|bolt| bolt.id);
    amend(store, unit, |unit| {
        let mut target = target_of(unit);
        match &existing {
            Some(bolt) => {
                target.insert("bolt".into(), json!(bolt));
                target.remove("new_name");
            }
            None => {
                target.insert("new_name".into(), json!(name));
                target.remove("bolt");
            }
        }
        unit.record.insert("target".into(), Value::Object(target));
    })?;
    Ok(())
}

/// `rename_bolt`: a bolt's name is never its identity (16, 29).
///
/// `atoms.yaml` declares it of a bolt and `unit.yaml:82` fires it on a unit,
/// where it renames the bolt that unit proposes; both are the same act on the
/// record that carries the name.
pub fn rename_bolt<S: StateStore>(store: &mut S, object: &str, name: &str) -> Result<()> {
    if name.is_empty() {
        return Ok(());
    }
    let Some(held) = store.get(object)? else {
        return Ok(());
    };
    let name = name.trim().trim_start_matches("rename").trim().to_string();
    match held.machine.as_str() {
        "bolt" => {
            amend(store, object, |bolt| {
                bolt.record.insert("name".into(), json!(name));
            })?;
        }
        _ => {
            // The unit's proposed bolt, which has no record of its own yet.
            let named = target_of(&held)
                .get("bolt")
                .and_then(|v| v.as_str())
                .map(String::from);
            match named {
                Some(bolt) => {
                    amend(store, &bolt, |bolt| {
                        bolt.record.insert("name".into(), json!(name));
                    })?;
                }
                None => {
                    amend(store, object, |unit| {
                        let mut target = target_of(unit);
                        target.insert("new_name".into(), json!(name));
                        unit.record.insert("target".into(), Value::Object(target));
                    })?;
                }
            }
        }
    }
    Ok(())
}

/// `set_type`: the response corrects the type the machinery proposed (27, 37).
pub fn set_type<S: StateStore>(store: &mut S, object: &str, kind: &str) -> Result<()> {
    if kind.is_empty() {
        return Ok(());
    }
    let kind = kind.trim().trim_start_matches("type").trim().to_string();
    amend(store, object, |object| {
        object.record.insert("type".into(), json!(kind));
    })?;
    Ok(())
}

// ------------------------------------------------------------- the intent

/// The material an intent holds that no elaboration of it cites: the signals
/// moved `attach` to it and the finding records on it
/// (`record-derived.yaml` intent.material_pending, 21).
pub fn pending_material<S: Records>(store: &S, intent: &str) -> Result<Vec<String>> {
    let mut cited: Vec<String> = Vec::new();
    for elaboration in children(store, intent, "elaboration")? {
        if let Some(sources) = elaboration.record.get("sources").and_then(|v| v.as_array()) {
            cited.extend(sources.iter().filter_map(|v| v.as_str().map(String::from)));
        }
        if let Some(signals) = elaboration.record.get("signals").and_then(|v| v.as_array()) {
            cited.extend(signals.iter().filter_map(|v| v.as_str().map(String::from)));
        }
    }
    let held = store.get(intent)?;
    let signals: Vec<String> = held
        .as_ref()
        .and_then(|o| o.record.get("signals"))
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    Ok(signals
        .into_iter()
        .filter(|signal| !cited.iter().any(|c| c == signal))
        .collect())
}

/// `propose_elaboration`: the intent's one proposed elaboration, made or grown
/// (21).
///
/// New material joins the one proposal awaiting approval rather than opening a
/// second: a proposed elaboration already there gains the material, and only an
/// intent with none gets a new one.
pub fn propose_elaboration<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    intent: &str,
    kind: &str,
    at: DateTime<Utc>,
) -> Result<String> {
    let material = pending_material(store, intent)?;
    let proposed = children(store, intent, "elaboration")?
        .into_iter()
        .find(|e| e.top_state().is_some_and(|s| s == "proposed"))
        .map(|e| e.id);
    let id = match proposed {
        Some(id) => id,
        None => {
            let ordinal = children(store, intent, "elaboration")?.len() + 1;
            let id = id_under(intent, "elaboration", &format!("proposed-{ordinal}"));
            let mut record: BTreeMap<String, Value> = BTreeMap::new();
            if !kind.is_empty() && kind != "from-material" {
                record.insert("type".into(), json!(kind));
            }
            commands::put_new(store, defs, &id, "elaboration", Some(intent), record, at)?;
            id
        }
    };
    // It cites all of it, and cites nothing twice (21, 109).
    amend(store, &id, |elaboration| {
        let mut signals: Vec<String> = elaboration
            .record
            .get("signals")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        for one in &material {
            if !signals.iter().any(|s| s == one) {
                signals.push(one.clone());
            }
        }
        elaboration.record.insert("signals".into(), json!(signals));
    })?;
    Ok(id)
}

/// `split_intent`: two proposed intents replace this one, its signals re-moved
/// to each (107).
///
/// The partition is the response's argument: the names the operator gave the
/// parts. Each part names this intent as `split_from`, which is what the proof
/// reads (`record-derived.yaml` intent.split_done).
pub fn split_intent<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    intent: &str,
    partition: &str,
    at: DateTime<Utc>,
) -> Result<Vec<String>> {
    let parts: Vec<String> = partition
        .trim()
        .trim_start_matches("split")
        .split(|c| c == ',' || c == ';')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect();
    // A split that names no parts is two parts of this intent's own name, so
    // the act is never a silent nothing (107).
    let stem = intent.split_once('/').map(|(_, rest)| rest).unwrap_or(intent);
    let parts = match parts.len() {
        0 | 1 => vec![format!("{stem}-1"), format!("{stem}-2")],
        _ => parts,
    };
    let signals: Vec<String> = store
        .get(intent)?
        .and_then(|o| o.record.get("signals").cloned())
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    let mut made = Vec::new();
    for (at_part, name) in parts.iter().enumerate() {
        let id = match name.contains('/') {
            true => name.clone(),
            false => format!("intent/{name}"),
        };
        let mut record: BTreeMap<String, Value> = BTreeMap::new();
        record.insert("split_from".into(), json!(intent));
        // The signals go to the parts in turn, so every one keeps a home and
        // none is counted twice (107, 109).
        let mine: Vec<&String> = signals
            .iter()
            .enumerate()
            .filter(|(n, _)| n % parts.len() == at_part)
            .map(|(_, s)| s)
            .collect();
        record.insert("signals".into(), json!(mine));
        record.insert("signals_count".into(), json!(mine.len()));
        if store.get(&id)?.is_none() {
            commands::put_new(store, defs, &id, "intent", None, record, at)?;
        }
        made.push(id);
    }
    Ok(made)
}

// ----------------------------------------------------------- the curation

/// One gathering a curation session delivered: elaborations of one type
/// proposed on several intents, gathered into one (188).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Gathering {
    pub kind: String,
    pub intents: Vec<String>,
}

/// The gatherings a session delivered, read from its exit's deliverables.
///
/// A deliverable naming a gathering is `gather <type>: <intent>, <intent>`;
/// anything else is a deliverable of another kind and is not one of these. In
/// this phase the operator is the curation session (93b) and the page's curator
/// surface delivers moves, so a run that gathered nothing delivers none — and
/// the effect then makes none, which is what its proof reads.
pub fn gatherings_of<S: Records>(store: &S, session: &str) -> Result<Vec<Gathering>> {
    let mut out = Vec::new();
    for entry in store.thread(session)? {
        if entry.kind != "exit" {
            continue;
        }
        let delivered: Vec<String> = entry
            .fields
            .get("deliverables")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        for one in delivered {
            let Some(rest) = one.trim().strip_prefix("gather ") else {
                continue;
            };
            let (kind, intents) = rest.split_once(':').unwrap_or((rest, ""));
            let intents: Vec<String> = intents
                .split(',')
                .map(|i| i.trim().to_string())
                .filter(|i| !i.is_empty())
                .collect();
            if intents.is_empty() {
                continue;
            }
            out.push(Gathering {
                kind: kind.trim().to_string(),
                intents,
            });
        }
    }
    Ok(out)
}

/// `gather_elaborations`: one proposed elaboration on the first intent named,
/// covering all of them, in place of one per intent (188).
pub fn gather_elaborations<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    gatherings: &[Gathering],
    at: DateTime<Utc>,
) -> Result<usize> {
    let mut made = 0;
    for gathering in gatherings {
        let Some(first) = gathering.intents.first() else {
            continue;
        };
        // One per gathering, found by what it covers, so a second pass makes
        // nothing (127).
        let held = children(store, first, "elaboration")?
            .into_iter()
            .find(|e| covers(e) == gathering.intents);
        if held.is_some() {
            continue;
        }
        let ordinal = children(store, first, "elaboration")?.len() + 1;
        let id = id_under(first, "elaboration", &format!("gathered-{ordinal}"));
        let mut record: BTreeMap<String, Value> = BTreeMap::new();
        record.insert("type".into(), json!(gathering.kind));
        record.insert("covers".into(), json!(gathering.intents));
        commands::put_new(store, defs, &id, "elaboration", Some(first), record, at)?;
        made += 1;
    }
    Ok(made)
}

/// The intents one elaboration covers (188).
pub fn covers(elaboration: &Object) -> Vec<String> {
    elaboration
        .record
        .get("covers")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default()
}
