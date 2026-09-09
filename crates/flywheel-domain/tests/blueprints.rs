//! The instance's own types, over the core set (57, 85, 223, 224).

use flywheel_domain::blueprints;
use std::path::{Path, PathBuf};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/blueprints")
}

/// A type file in the blueprints is a machine the engine runs: no code change,
/// no new binary, no host restarted for one (57, 85).
#[test]
fn type_from_blueprints_runs() {
    let core = flywheel_domain::set::load().expect("the core set parses");
    assert!(
        core.machines.get("spike").is_none(),
        "the type is the instance's, not the release's"
    );

    let loaded = blueprints::load_over_core(&fixture()).expect("the blueprints read");
    assert!(loaded.refusals.is_empty(), "{:#?}", loaded.refusals);
    let spike = loaded.machines_get("spike").expect("the instance's own type is in force");
    assert_eq!(spike.version, 1);
    assert!(
        spike.regions.contains_key("stages"),
        "it is a machine the engine can run, not a document"
    );
    // Nothing the binary carries was disturbed by reading it.
    assert!(loaded.defs.machines.contains_key("instance"));
    assert_eq!(
        loaded.defs.machines.get("chore").map(|m| m.version),
        core.machines.get("chore").map(|m| m.version)
    );
}

/// An object holds the version of the type it started with. A new type file at
/// a new version is for the objects made after it, not for the ones in flight
/// (57, 224).
#[test]
fn type_version_held_in_flight() {
    let dir = std::env::temp_dir().join(format!("flywheel-types-{}", std::process::id()));
    let types = dir.join("flywheel/unit-types");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&types).unwrap();
    let one = std::fs::read_to_string(fixture().join("flywheel/unit-types/spike@1.yaml")).unwrap();
    std::fs::write(types.join("spike@1.yaml"), &one).unwrap();

    let before = blueprints::load_over_core(&dir).unwrap();
    assert_eq!(blueprints::version_of(&before.defs, "spike"), Some(1));
    // An object created now is stamped with the version in force.
    let stamped = blueprints::version_of(&before.defs, "spike").unwrap();

    // The operator writes a new version of the same type.
    std::fs::write(
        types.join("spike@2.yaml"),
        one.replace("version: 1", "version: 2"),
    )
    .unwrap();
    let after = blueprints::load_over_core(&dir).unwrap();
    assert_eq!(
        blueprints::version_of(&after.defs, "spike"),
        Some(2),
        "a bare reference takes the newest"
    );
    assert_eq!(
        blueprints::machine_for(&after.defs, "spike", Some(stamped)).map(|m| m.version),
        Some(1),
        "the object in flight holds the version it started with"
    );
    assert_eq!(
        blueprints::machine_for(&after.defs, "spike", None).map(|m| m.version),
        Some(2),
        "and one made now takes the new one"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A blueprints file that would override a core machine is refused by name, the
/// refusal is what the run record carries, and the core machine goes on running
/// (223).
#[test]
fn core_machine_override_refused() {
    let dir = std::env::temp_dir().join(format!("flywheel-override-{}", std::process::id()));
    let types = dir.join("flywheel/unit-types");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&types).unwrap();
    let core = flywheel_domain::set::load().unwrap();
    let was = core.machines.get("instance").expect("a core machine").version;
    std::fs::write(
        types.join("instance@99.yaml"),
        "machine: instance\nversion: 99\nkind: object\ntier: core\nregions: {}\n",
    )
    .unwrap();

    let loaded = blueprints::load_over_core(&dir).unwrap();
    assert_eq!(loaded.refusals.len(), 1, "{:#?}", loaded.refusals);
    let said = &loaded.refusals[0];
    assert!(said.contains("instance"), "the refusal names the machine: {said}");
    assert!(said.contains("223"), "and the rule it broke: {said}");
    assert_eq!(
        loaded.defs.machines.get("instance").map(|m| m.version),
        Some(was),
        "what the binary carries is what runs"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
