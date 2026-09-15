//! The instance's own types, read from a blueprints checkout at the shared
//! line.
//!
//! A type composed of existing atoms is added by writing a file in the
//! blueprints and committing it: no code changes and no host is rebuilt for one
//! (57, 85). The core machines are the binary's and nothing here may override
//! one; a file that tries is refused by name, the refusal goes to the run
//! record, and the core machine stands (223).

use anyhow::{Context, Result};
use flywheel_atoms::Records;
use flywheel_engine::defs::{Definitions, Machine, MachineKind};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::OnceLock;

/// Where an instance's own type files live in its blueprints repository.
pub const TYPE_DIRECTORIES: &[&str] = &["flywheel/unit-types", "flywheel/elaboration-types"];

/// The definitions in force, and what was refused on the way to them.
#[derive(Debug, Default)]
pub struct Loaded {
    pub defs: Definitions,
    /// One line per refusal, for the run record (79–82, 223).
    pub refusals: Vec<String>,
}

/// The core set with the instance's own types folded over it. Reading the
/// blueprints again is how a new type takes effect: no restart, no new binary
/// (57, 85).
pub fn load_over_core(checkout: &Path) -> Result<Loaded> {
    let mut loaded = Loaded {
        defs: crate::set::load()?,
        refusals: vec![],
    };
    // The shipped types are pinnable by version too, so an object in flight
    // holds the one it started with (57, 224).
    let pinned: Vec<Machine> = loaded.defs.machines.values().cloned().collect();
    for machine in pinned {
        offer(&mut loaded.defs, machine);
    }
    for directory in TYPE_DIRECTORIES {
        let dir = checkout.join(directory);
        if !dir.is_dir() {
            continue;
        }
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .with_context(|| format!("reading {}", dir.display()))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "yaml" || x == "yml"))
            .collect();
        files.sort();
        for path in files {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let machine: Machine = match serde_yaml::from_str(&text) {
                Ok(m) => m,
                Err(e) => {
                    loaded
                        .refusals
                        .push(format!("{} is no machine file: {e}", path.display()));
                    continue;
                }
            };
            // The core machines are carried in the binary and an instance never
            // edits one (223). The refusal names the file and the machine, and
            // what the binary carries goes on running.
            if crate::set::is_core_machine(&machine.machine) {
                loaded.refusals.push(format!(
                    "{} would override the core machine `{}`: refused, and the core machine stands (223)",
                    path.display(),
                    machine.machine
                ));
                continue;
            }
            offer(&mut loaded.defs, machine);
        }
    }
    Ok(loaded)
}

/// Put a machine in under its bare name — the highest version wins for a bare
/// reference — and under `name@version`, which is how a record pins the one it
/// started with.
fn offer(defs: &mut Definitions, machine: Machine) {
    let pinned = format!("{}@{}", machine.machine, machine.version);
    match defs.machines.get(&machine.machine) {
        Some(have) if have.version >= machine.version => {}
        _ => {
            defs.machines.insert(machine.machine.clone(), machine.clone());
        }
    }
    defs.machines.entry(pinned).or_insert(machine);
}

impl Loaded {
    /// A machine by name, whatever it came from.
    pub fn machines_get(&self, name: &str) -> Option<&Machine> {
        self.defs.machines.get(name)
    }
}

/// The version of a type in force right now, for stamping on an object as it
/// is created (57, 224).
pub fn version_of(defs: &Definitions, type_name: &str) -> Option<u32> {
    defs.machines.get(type_name).map(|m| m.version)
}

/// The machine an object holds: the version its record pins, and the one in
/// force only when it pins none. An object in flight is not moved to a new
/// type under it (57).
pub fn machine_for<'a>(
    defs: &'a Definitions,
    type_name: &str,
    type_version: Option<u32>,
) -> Option<&'a Machine> {
    match type_version {
        Some(v) => defs
            .machines
            .get(&format!("{type_name}@{v}"))
            .or_else(|| defs.machines.get(type_name)),
        None => defs.machines.get(type_name),
    }
}

/// The name a pinned reference stands for: `chore@2` is `chore`.
fn bare(name: &str) -> &str {
    name.split_once('@').map(|(n, _)| n).unwrap_or(name)
}

/// The types the binary's registry holds, by name: every `name@version` under
/// `types` in the shipped `registry.yaml` (224, model.md 10.7).
fn registered() -> &'static BTreeSet<String> {
    static REGISTERED: OnceLock<BTreeSet<String>> = OnceLock::new();
    REGISTERED.get_or_init(|| {
        let Some((_, bytes)) = crate::set::files().into_iter().find(|(path, _)| path == "registry.yaml") else {
            return BTreeSet::new();
        };
        let registry: serde_yaml::Value = serde_yaml::from_slice(bytes).unwrap_or_default();
        registry
            .get("types")
            .and_then(|types| types.as_mapping())
            .map(|types| types.keys().filter_map(|k| k.as_str()).map(|k| bare(k).to_string()).collect())
            .unwrap_or_default()
    })
}

/// The newest version of a type the binary's registry holds, or none where it
/// holds no type of that name (224, model.md 10.7).
pub fn registered_version(name: &str) -> Option<u64> {
    let (_, bytes) = crate::set::files().into_iter().find(|(path, _)| path == "registry.yaml")?;
    let registry: serde_yaml::Value = serde_yaml::from_slice(bytes).ok()?;
    registry
        .get("types")?
        .as_mapping()?
        .keys()
        .filter_map(|key| key.as_str()?.split_once('@'))
        .filter(|(named, _)| *named == bare(name))
        .filter_map(|(_, version)| version.parse::<u64>().ok())
        .max()
}

/// The elaboration types the binary's registry holds, by name: what a curation
/// session's proposal may name (188, model.md 10.7).
pub fn elaboration_types() -> Vec<String> {
    let Some((_, bytes)) = crate::set::files().into_iter().find(|(path, _)| path == "registry.yaml") else {
        return vec![];
    };
    let registry: serde_yaml::Value = serde_yaml::from_slice(bytes).unwrap_or_default();
    let mut out: Vec<String> = registry
        .get("types")
        .and_then(|types| types.as_mapping())
        .map(|types| {
            types
                .iter()
                .filter(|(_, entry)| {
                    entry.get("file").and_then(|f| f.as_str()).is_some_and(|f| f.starts_with("elaboration-types/"))
                })
                .filter_map(|(name, _)| name.as_str().map(|n| bare(n).to_string()))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out.dedup();
    out
}

/// The machines the binary carries, by name.
fn core_names() -> &'static BTreeSet<String> {
    static CORE: OnceLock<BTreeSet<String>> = OnceLock::new();
    CORE.get_or_init(|| {
        crate::set::load()
            .map(|defs| defs.machines.into_keys().collect())
            .unwrap_or_default()
    })
}

/// Whether the registry in force holds a type at some version: one the binary
/// registers, or one of the instance's own that the definitions in force were
/// read over from its blueprints (57, 85). A type that is null, empty or names
/// no entry is not defined, and an object of it is not approved until the
/// operator sets one (85a).
pub fn type_defined(defs: &Definitions, type_name: Option<&str>) -> bool {
    let Some(name) = type_name.map(str::trim).filter(|n| !n.is_empty()) else {
        return false;
    };
    let name = bare(name);
    if registered().contains(name) {
        return true;
    }
    // An instance's own type is a template the core set does not carry: a core
    // machine is never overridden from the blueprints (223), so a template of
    // another name in force came from there.
    !core_names().contains(name)
        && defs
            .machines
            .values()
            .any(|m| m.kind == MachineKind::Template && m.machine == name)
}

/// `unit.type_defined` and `elaboration.type_defined`: the object's own type
/// against the definitions in force (85a, `record-derived.yaml`).
pub fn evidence<S: Records>(store: &S, defs: &Definitions, object: &str, name: &str) -> Option<Value> {
    if !matches!(name, "unit.type_defined" | "elaboration.type_defined") {
        return None;
    }
    let held = store.get(object).ok().flatten();
    let kind = held.as_ref().and_then(|o| o.record.get("type")).and_then(|v| v.as_str());
    Some(json!(type_defined(defs, kind)))
}
