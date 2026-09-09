//! The instance's own types, read from a blueprints checkout at the shared
//! line.
//!
//! A type composed of existing atoms is added by writing a file in the
//! blueprints and committing it: no code changes and no host is rebuilt for one
//! (57, 85). The core machines are the binary's and nothing here may override
//! one; a file that tries is refused by name, the refusal goes to the run
//! record, and the core machine stands (223).

use anyhow::{Context, Result};
use flywheel_engine::defs::{Definitions, Machine};
use std::path::Path;

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
