//! The definition set the release ships, compiled into the binary.
//!
//! The machines, profiles, schemas and atoms the flywheel ships are core
//! definitions an instance never edits (223): they are in the binary, not
//! beside it, so a host can prove which set it ran and 208's stamped version
//! is provable from the bytes themselves (224, 83, D2). The instance's own
//! unit and elaboration types are a second source, read from the blueprints
//! (57, 85) — see `blueprints`.

use anyhow::{Context, Result};
use flywheel_engine::defs::{Atoms, Definitions, Machine};
use include_dir::{include_dir, Dir};

/// `definitions/`, byte for byte, as the release shipped it.
static SET: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../definitions");

/// The release's set version: one version for the whole set, which
/// initialization and repository creation stamp (208, 224).
pub const SET_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Every file of the set, by path, in path order. The order is the digest's,
/// so two builds of one set agree.
pub fn files() -> Vec<(String, &'static [u8])> {
    let mut out = Vec::new();
    collect(&SET, &mut out);
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn collect(dir: &'static Dir<'static>, out: &mut Vec<(String, &'static [u8])>) {
    for file in dir.files() {
        out.push((file.path().to_string_lossy().to_string(), file.contents()));
    }
    for sub in dir.dirs() {
        collect(sub, out);
    }
}

/// What the set hashes to: the path and the bytes of every file, in path
/// order. This is what a host reports to prove which set it ran (83, 224).
pub fn digest() -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (path, bytes) in files() {
        for b in path.as_bytes().iter().chain(bytes.iter()) {
            hash ^= *b as u64;
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
    }
    hash
}

/// The digest of a directory of definitions, by the same rule, so the embedded
/// set and `definitions/` on disk can be compared (D2).
pub fn digest_of_dir(dir: &std::path::Path) -> Result<u64> {
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d)
            .with_context(|| format!("reading {}", d.display()))?
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(dir)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            files.push((relative, std::fs::read(&path)?));
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for (path, bytes) in &files {
        for b in path.as_bytes().iter().chain(bytes.iter()) {
            hash ^= *b as u64;
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
    }
    Ok(hash)
}

/// The core definitions, parsed. The rules are the directory loader's: one
/// atoms file, every file naming a machine is one, and the highest version of
/// a name wins for a bare reference.
pub fn load() -> Result<Definitions> {
    let mut defs = Definitions::default();
    for (path, bytes) in files() {
        let Some(name) = path.rsplit('/').next() else { continue };
        if !(path.ends_with(".yaml") || path.ends_with(".yml")) {
            continue;
        }
        let text = std::str::from_utf8(bytes)
            .with_context(|| format!("the embedded {path} is not text"))?;
        if name == "atoms.yaml" {
            defs.atoms =
                serde_yaml::from_str::<Atoms>(text).with_context(|| format!("parsing {path}"))?;
            continue;
        }
        if !text.contains("\nmachine:") && !text.starts_with("machine:") {
            continue;
        }
        let machine: Machine =
            serde_yaml::from_str(text).with_context(|| format!("parsing {path}"))?;
        match defs.machines.get(&machine.machine) {
            Some(have) if have.version >= machine.version => {}
            _ => {
                defs.machines.insert(machine.machine.clone(), machine);
            }
        }
    }
    Ok(defs)
}

/// Every core machine and the version it carries, in name order (224).
pub fn versions() -> Result<Vec<(String, u32)>> {
    let defs = load()?;
    let mut out: Vec<(String, u32)> = defs
        .machines
        .values()
        .map(|m| (m.machine.clone(), m.version))
        .collect();
    out.sort();
    Ok(out)
}

/// Whether a path names a core machine, which nothing outside the binary may
/// override (223).
pub fn is_core_machine(name: &str) -> bool {
    load()
        .map(|d| d.machines.contains_key(name))
        .unwrap_or(false)
}

/// The engine's own machines, with the atoms they name and no others.
///
/// The domain is `atoms.yaml` plus every machine file outside `engine/`
/// (model.md §2.5, 87). A scenario that names its own `machines:` replaces the
/// domain — the contract set runs over a toy machine that shares no atom with
/// the flywheel — and the five engine machines still run, because the lease,
/// the rail, the sink, the host and the response are the engine's and every
/// object is worked through them (86, 128, 148).
pub fn engine() -> Result<Definitions> {
    let mut shipped = Atoms::default();
    let mut defs = Definitions::default();
    for (path, bytes) in files() {
        let text = std::str::from_utf8(bytes)
            .with_context(|| format!("the embedded {path} is not text"))?;
        if path == "atoms.yaml" {
            shipped =
                serde_yaml::from_str::<Atoms>(text).with_context(|| format!("parsing {path}"))?;
            continue;
        }
        if !path.starts_with("engine/") || !path.ends_with(".yaml") {
            continue;
        }
        let machine: Machine =
            serde_yaml::from_str(text).with_context(|| format!("parsing {path}"))?;
        defs.machines.insert(machine.machine.clone(), machine);
    }
    for machine in defs.machines.values() {
        let (evidence, effects) = machine.atom_names();
        for name in evidence {
            if let Some(value) = shipped.evidence.get(&name) {
                defs.atoms.evidence.insert(name, value.clone());
            }
        }
        for name in effects {
            let Some(value) = shipped.effects.get(&name) else { continue };
            // The effect's proof is an atom the engine reads to know the act
            // already happened, so it comes with it (127).
            if let Some(proof) = shipped.proof_of(&name) {
                if let Some(value) = shipped.evidence.get(&proof) {
                    defs.atoms.evidence.insert(proof, value.clone());
                }
            }
            defs.atoms.effects.insert(name, value.clone());
        }
    }
    Ok(defs)
}

/// A set the engine's machines and atoms are folded into. What the directory
/// carries wins: a set that defines a machine or an atom of its own keeps it.
pub fn with_engine(mut defs: Definitions) -> Result<Definitions> {
    let engine = engine()?;
    for (name, machine) in engine.machines {
        defs.machines.entry(name).or_insert(machine);
    }
    for (name, value) in engine.atoms.evidence {
        defs.atoms.evidence.entry(name).or_insert(value);
    }
    for (name, value) in engine.atoms.effects {
        defs.atoms.effects.entry(name).or_insert(value);
    }
    Ok(defs)
}
