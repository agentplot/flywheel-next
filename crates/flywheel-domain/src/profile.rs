//! A profile's binding, and the gate that admits it.
//!
//! A profile binds every evidence name to a read and every effect name to an
//! act, and names the mechanism behind each guarantee. A binding that leaves a
//! name unbound is not a profile: it is refused at load, by name, rather than
//! failing at the first tick that reads the name nobody bound (138–140, 169,
//! 170).

use anyhow::{bail, Context, Result};
use flywheel_engine::defs::Atoms;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

/// The eight operations of 125, which are the whole of what the engine asks a
/// state store for.
pub const OPERATIONS: &[&str] = &[
    "read",
    "write_effect",
    "lease",
    "present",
    "receive",
    "notify",
    "list",
    "status",
];

/// The five guarantees a state store profile must name a mechanism for (139).
pub const GUARANTEES: &[&str] = &[
    "durable",
    "single_writer",
    "atomic",
    "derivable",
    "response_once",
];

/// One profile file as it is written.
#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    pub name: String,
    #[serde(default)]
    pub complete: bool,
    #[serde(default)]
    pub inherits: Vec<String>,
    #[serde(default)]
    pub evidence: BTreeMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub effects: BTreeMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub guarantees: BTreeMap<String, String>,
    #[serde(default)]
    pub contract: BTreeMap<String, String>,
}

/// A profile with everything it inherits folded in: what the gate is applied
/// to, because a partial binding is only complete together with the ones it
/// names (`inherits:`).
#[derive(Debug, Clone, Default)]
pub struct Binding {
    pub name: String,
    pub complete: bool,
    pub evidence: BTreeSet<String>,
    pub effects: BTreeSet<String>,
    pub guarantees: BTreeMap<String, String>,
    pub contract: BTreeSet<String>,
    /// The profiles folded in, in the order they were read.
    pub sources: Vec<String>,
}

/// Where profile files are read from. The set the binary carries is the one a
/// host uses; a directory is for the parity test and for a binding under test.
pub trait Profiles {
    fn read(&self, name: &str) -> Result<String>;
}

/// The profiles of the embedded set (D2).
pub struct Embedded;

impl Profiles for Embedded {
    fn read(&self, name: &str) -> Result<String> {
        let want = format!("profiles/{name}.yaml");
        for (path, bytes) in crate::set::files() {
            if path == want {
                return Ok(String::from_utf8_lossy(bytes).to_string());
            }
        }
        bail!("the set carries no profile named `{name}`")
    }
}

/// The profiles of a directory of definitions.
pub struct Dir(pub std::path::PathBuf);

impl Profiles for Dir {
    fn read(&self, name: &str) -> Result<String> {
        let path = self.0.join("profiles").join(format!("{name}.yaml"));
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))
    }
}

/// Read a profile and everything it inherits, folded into one binding.
pub fn bind(profiles: &impl Profiles, name: &str) -> Result<Binding> {
    let mut binding = Binding {
        name: name.to_string(),
        ..Default::default()
    };
    let mut seen: BTreeSet<String> = BTreeSet::new();
    fold(profiles, name, &mut binding, &mut seen)?;
    Ok(binding)
}

fn fold(
    profiles: &impl Profiles,
    name: &str,
    into: &mut Binding,
    seen: &mut BTreeSet<String>,
) -> Result<()> {
    if !seen.insert(name.to_string()) {
        return Ok(());
    }
    let text = profiles.read(name)?;
    let profile: Profile =
        serde_yaml::from_str(&text).with_context(|| format!("parsing the profile `{name}`"))?;
    if name == into.name {
        into.complete = profile.complete;
    }
    into.sources.push(profile.name.clone());
    into.evidence.extend(profile.evidence.keys().cloned());
    into.effects.extend(profile.effects.keys().cloned());
    into.contract.extend(profile.contract.keys().cloned());
    for (guarantee, mechanism) in profile.guarantees {
        into.guarantees.insert(guarantee, mechanism);
    }
    for parent in profile.inherits {
        fold(profiles, &parent, into, seen)?;
    }
    Ok(())
}

/// Everything wrong with a binding, by name. Empty is a profile; anything else
/// is refused (138–140, 169, 170).
pub fn faults(binding: &Binding, atoms: &Atoms) -> Vec<String> {
    let mut out = Vec::new();
    for name in atoms.evidence.keys() {
        if !binding.evidence.contains(name) {
            out.push(format!("evidence `{name}` is bound to no read"));
        }
    }
    for name in atoms.effects.keys() {
        if !binding.effects.contains(name) {
            out.push(format!("effect `{name}` is bound to no act"));
        }
    }
    for name in &binding.evidence {
        if !atoms.evidence.contains_key(name) {
            out.push(format!(
                "the binding names evidence `{name}`, which the atoms file does not have"
            ));
        }
    }
    for guarantee in GUARANTEES {
        match binding.guarantees.get(*guarantee) {
            Some(mechanism) if !mechanism.trim().is_empty() => {}
            _ => out.push(format!("guarantee `{guarantee}` names no mechanism")),
        }
    }
    for operation in OPERATIONS {
        if !binding.contract.contains(*operation) {
            out.push(format!("operation `{operation}` is bound to nothing"));
        }
    }
    out
}

/// The gate: a binding with a fault is not a profile, and the refusal names
/// what is missing rather than the file (140, 169, 170).
pub fn admit(binding: &Binding, atoms: &Atoms) -> Result<()> {
    if !binding.complete {
        bail!(
            "`{}` is a partial binding, not a profile: it is inherited, never bound on its own",
            binding.name
        );
    }
    let faults = faults(binding, atoms);
    if faults.is_empty() {
        return Ok(());
    }
    bail!(
        "the binding `{}` is not a profile — {}",
        binding.name,
        faults.join("; ")
    )
}
