//! The binding gate (138–140, 169, 170).

use flywheel_domain::profile::{self, Embedded};

fn atoms() -> flywheel_engine::defs::Atoms {
    flywheel_domain::set::load().expect("the embedded set parses").atoms
}

#[test]
fn the_shipped_profile_is_admitted() {
    let binding = profile::bind(&Embedded, "git-only").expect("the profile reads");
    let faults = profile::faults(&binding, &atoms());
    assert!(faults.is_empty(), "git-only is a profile: {faults:#?}");
    profile::admit(&binding, &atoms()).expect("admitted");
}

/// A binding that leaves a name unbound is not a profile. The refusal names
/// what is missing, so an operator is told which name nobody bound rather than
/// meeting it at the first tick that reads it (140, 169, 170).
#[test]
fn incomplete_binding_is_refused() {
    let dir = std::env::temp_dir().join(format!("flywheel-binding-{}", std::process::id()));
    let profiles = dir.join("profiles");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&profiles).unwrap();

    // A profile of its own, complete on its face, binding nothing.
    std::fs::write(
        profiles.join("hollow.yaml"),
        "format: flywheel-profile/1\nname: hollow\ncomplete: true\n",
    )
    .unwrap();
    let binding = profile::bind(&profile::Dir(dir.clone()), "hollow").expect("it reads");
    let refused = profile::admit(&binding, &atoms()).expect_err("a hollow binding is no profile");
    let said = format!("{refused}");
    assert!(said.contains("hollow"), "{said}");
    assert!(
        said.contains("is bound to no read"),
        "the refusal names an unbound evidence name: {said}"
    );
    assert!(
        said.contains("is bound to no act"),
        "and an unbound effect name: {said}"
    );
    assert!(
        said.contains("guarantee `durable` names no mechanism"),
        "and the guarantee with no mechanism: {said}"
    );

    // One name short of the shipped profile is still short.
    let mut text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../definitions/profiles/record-derived.yaml"),
    )
    .unwrap();
    let dropped = "  seq:            {read: \"get(id).seq\"}\n";
    assert!(text.contains(dropped), "the line to drop is there");
    text = text.replace(dropped, "");
    for name in ["git-only", "host", "sessions", "blueprints", "surfaces", "identity", "context", "deliverables", "tracker", "sessions-stand-in"] {
        let from = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../../definitions/profiles/{name}.yaml"));
        std::fs::copy(&from, profiles.join(format!("{name}.yaml"))).unwrap();
    }
    std::fs::write(profiles.join("record-derived.yaml"), &text).unwrap();
    let short = profile::bind(&profile::Dir(dir.clone()), "git-only").expect("it reads");
    let refused = profile::admit(&short, &atoms()).expect_err("one name short is refused");
    let said = format!("{refused}");
    assert!(
        said.contains("evidence `seq` is bound to no read"),
        "the refusal names the one missing name: {said}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// A partial binding is not a profile on its own, however complete the ones it
/// is folded into are.
#[test]
fn a_partial_binding_is_not_a_profile() {
    let binding = profile::bind(&Embedded, "record-derived").expect("it reads");
    let refused = profile::admit(&binding, &atoms()).expect_err("partial is not a profile");
    assert!(format!("{refused}").contains("partial binding"));
}
