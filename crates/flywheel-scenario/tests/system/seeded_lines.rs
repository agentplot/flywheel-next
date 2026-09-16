//! A seed covers what it seeds: an elaboration approved on a seeded intent is
//! placed off the line that intent says it has (19.6, 125, 149).
//!
//! Third tier by subject and by what it needs, as `storefront.rs` is: the
//! scenario's session actions report through the real binary, and what is
//! proved is the product carrying the storefront's own arc (D17, 93).

use flywheel_scenario::conformance::{drive, Profile, RunOptions, Suite};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// The `flywheel` binary beside the test binary; a session's exit reports
/// through it, and a test is not it (D8, 93).
fn flywheel_binary_beside_the_test() -> PathBuf {
    let deps = std::env::current_exe().expect("the test binary has a path");
    let target = deps.parent().and_then(|p| p.parent()).expect("target/debug");
    let binary = target.join("flywheel");
    if !binary.exists() {
        let out = std::process::Command::new(env!("CARGO"))
            .args(["build", "--quiet", "-p", "flywheel"])
            .current_dir(root())
            .output()
            .expect("building the flywheel binary");
        assert!(
            binary.exists(),
            "no flywheel binary at {}: {}",
            binary.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    binary
}

/// The storefront seeds `intent/storefront-mobile-speed` with its line current.
/// Curation routes the mobile signal to it, which proposes an elaboration
/// there; the operator's yes approves it and its place is taken off that line.
///
/// Nothing made the line before, so the guard the place waits on read it as
/// absent: the elaboration got no place and the scenario could not run past it
/// (19.6, 125, 149).
#[test]
fn an_elaboration_of_a_seeded_intent_gets_its_place() {
    const INTENT: &str = "intent/storefront-mobile-speed";
    std::env::set_var(
        flywheel_scenario::sessions::BINARY_ENV,
        flywheel_binary_beside_the_test(),
    );
    let path = root().join("scenarios/storefront");
    let (scenario, _) = flywheel_atoms::conformance::load(&path).expect("the storefront loads");
    let suite = Suite::for_scenario(&path).expect("the suite opens");
    let options = RunOptions {
        profile: Profile::GitOnly,
        // Through curation's delivery: its moves attach the mobile signal to
        // the seeded intent, which is what proposes an elaboration there.
        through: Some(6),
        ..Default::default()
    };
    let mut run = drive::play(&scenario, &path, &suite, &options).expect("the storefront plays");

    // The line the seed said was there is there, so the place has something to
    // be taken off.
    let line = run
        .runtime
        .store
        .world
        .lines
        .get(INTENT)
        .unwrap_or_else(|| panic!("the seed made no line for {INTENT}"));
    assert!(line.exists && !line.absent, "the seeded line is not there: {line:?}");

    // The elaboration curation proposed on it, standing with its number. It is
    // the intent's by its parent, not by its id: an elaboration is named under
    // itself.
    let proposed: Vec<(String, Option<u32>)> = run
        .runtime
        .decisions()
        .iter()
        .filter(|d| d.kind == "elaboration-proposed")
        .map(|d| (d.object.clone(), d.number))
        .collect();
    let (object, number) = proposed
        .into_iter()
        .find(|(object, _)| {
            run.runtime.store.objects.get(object).and_then(|o| o.parent.as_deref()) == Some(INTENT)
        })
        .expect("an elaboration of the seeded intent stands");
    let number = number.expect("the register numbered it");
    let before = run.runtime.store.objects.get(&object).expect("the elaboration").clone();
    assert_eq!(before.config.get("life").map(String::as_str), Some("proposed"));
    assert_eq!(before.config.get("place").map(String::as_str), Some("none"), "it is placed before it is approved");

    // The operator's yes, through the register's number as any answer is.
    run.runtime.respond(number, "yes", "chuck");
    run.runtime.settle(80);

    // The approval applied, and the place is made and current.
    let held = run.runtime.store.objects.get(&object).expect("the elaboration").clone();
    assert_ne!(
        held.config.get("life").map(String::as_str),
        Some("proposed"),
        "the yes left the elaboration proposed: {:?}",
        held.config
    );
    assert_eq!(
        held.config.get("place.place.life").map(String::as_str),
        Some("ready"),
        "the elaboration's place is not ready: {:?}",
        held.config
    );
    let place = format!("{object}#own");
    let made = run
        .runtime
        .store
        .world
        .places
        .get(&place)
        .unwrap_or_else(|| panic!("no place was made at {place}: {:?}", run.runtime.store.world.places.keys()));
    assert!(made.exists && !made.absent, "the place is not there: {made:?}");

    let _ = std::fs::remove_dir_all(&run.places);
}
