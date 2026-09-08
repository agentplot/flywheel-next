//! The evidence and effect registries, generated from `definitions/atoms.yaml`.
//!
//! One evidence name per question asked of the world, one effect name per act
//! on it (87). Machine definitions may use only these names and a profile binds
//! every one of them (B.3). Nothing here is a label, column, path, service or
//! interface (I13, 138).

/// The type an evidence name answers with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomType {
    Bool,
    Int,
    String,
    Time,
    List,
    State,
    Enum,
    Hash,
    HostId,
}

/// One question asked of the world.
#[derive(Debug, Clone, Copy)]
pub struct EvidenceAtom {
    pub name: &'static str,
    /// The object kinds this evidence is asked of; `any` is every object.
    pub of: &'static [&'static str],
    pub ty: AtomType,
    /// The closed set of answers, for an `enum`; empty otherwise.
    pub values: &'static [&'static str],
    /// The effect this evidence is the proof of, where it is one (73).
    pub proof_of: Option<&'static str>,
}

/// One act on the world.
#[derive(Debug, Clone, Copy)]
pub struct EffectAtom {
    pub name: &'static str,
    pub of: &'static [&'static str],
    pub args: &'static [&'static str],
    /// The evidence that shows the act has already been done (73, 127).
    pub proof: &'static str,
    pub satisfies: &'static [u32],
}

include!(concat!(env!("OUT_DIR"), "/atoms_generated.rs"));

/// The evidence registry.
pub struct Evidence;

impl Evidence {
    pub fn all() -> &'static [EvidenceAtom] {
        EVIDENCE
    }

    pub fn get(name: &str) -> Option<&'static EvidenceAtom> {
        EVIDENCE.iter().find(|a| a.name == name)
    }

    pub fn contains(name: &str) -> bool {
        Self::get(name).is_some()
    }

    pub fn names() -> impl Iterator<Item = &'static str> {
        EVIDENCE.iter().map(|a| a.name)
    }
}

/// The effect registry.
pub struct Effect;

impl Effect {
    pub fn all() -> &'static [EffectAtom] {
        EFFECTS
    }

    pub fn get(name: &str) -> Option<&'static EffectAtom> {
        EFFECTS.iter().find(|a| a.name == name)
    }

    pub fn contains(name: &str) -> bool {
        Self::get(name).is_some()
    }

    pub fn names() -> impl Iterator<Item = &'static str> {
        EFFECTS.iter().map(|a| a.name)
    }

    /// The evidence that proves this effect done, as a registered atom.
    pub fn proof_of(name: &str) -> Option<&'static EvidenceAtom> {
        Self::get(name).and_then(|e| Evidence::get(e.proof))
    }
}

/// FNV-1a over bytes; the same function the build script hashes the atoms file
/// with, so a test can recompute it from the file on disk.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}
