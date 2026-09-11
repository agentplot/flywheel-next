//! The registry is the atoms file, in Rust. These tests are the proof of that.

use crate::{Effect, Evidence};

const ATOMS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../definitions/atoms.yaml");

fn atoms_file() -> serde_yaml::Value {
    let text = std::fs::read_to_string(ATOMS_PATH).expect("definitions/atoms.yaml is readable");
    serde_yaml::from_str(&text).expect("definitions/atoms.yaml parses")
}

#[test]
fn registry_covers_atoms_file() {
    let file = atoms_file();

    let evidence = file["evidence"].as_mapping().expect("evidence is a mapping");
    assert!(!evidence.is_empty());
    for key in evidence.keys() {
        let name = key.as_str().expect("an evidence name is a string");
        assert!(
            Evidence::contains(name),
            "evidence {name} is in the atoms file and not in the registry"
        );
    }
    assert_eq!(
        Evidence::all().len(),
        evidence.len(),
        "the registry holds a name the atoms file does not"
    );

    let effects = file["effects"].as_mapping().expect("effects is a mapping");
    assert!(!effects.is_empty());
    for key in effects.keys() {
        let name = key.as_str().expect("an effect name is a string");
        assert!(
            Effect::contains(name),
            "effect {name} is in the atoms file and not in the registry"
        );
    }
    assert_eq!(
        Effect::all().len(),
        effects.len(),
        "the registry holds an effect the atoms file does not"
    );
}

#[test]
fn atoms_file_change_is_seen_by_the_build() {
    // Adding a name to the atoms file without regenerating leaves the built
    // registry describing an older file. The generated hash is what catches it.
    let bytes = std::fs::read(ATOMS_PATH).expect("definitions/atoms.yaml is readable");
    assert_eq!(
        crate::atoms::ATOMS_FILE_HASH,
        crate::atoms::fnv1a(&bytes),
        "the registry was generated from a different definitions/atoms.yaml"
    );
}

#[test]
fn every_effect_names_a_registered_proof() {
    // Every effect names its proof, and the proof is an evidence name (73, 127).
    for effect in Effect::all() {
        assert!(
            Evidence::contains(effect.proof),
            "effect {} names proof {}, which is no evidence",
            effect.name,
            effect.proof
        );
    }
}
