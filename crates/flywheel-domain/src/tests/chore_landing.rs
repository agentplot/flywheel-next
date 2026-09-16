//! A shared-line chore's merge is its landing (60, 62, `unit.yaml` v6).
//!
//! A chore standing under a repository or the instance merges straight onto
//! that shared line, and no bolt stands above it to land with. The shared line
//! is where a landing puts work, so its merge is its landing: it is final on
//! the pass that merges it, with everything a landing brings — its cited
//! claim's cell due again (64), the review batch when it cited none (317), its
//! place released with the merge (55) and its offer's pin stale (62).
//!
//! A bolt's unit still lands with its bolt, and an intent's conflict chore with
//! the intent's line as the intent closes (52).

use crate::commands;
use crate::derived::{self, Reading};
use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::{Records, Scope};
use flywheel_engine::Definitions;
use serde_json::{json, Value};

fn a_store() -> (FakeStore, Definitions) {
    let defs = crate::set::load().expect("the embedded definitions");
    (FakeStore::default(), defs)
}

/// What the record layer answers for one name, as a host reads it: the fake
/// store answers only what a test seeds, so the binding itself is read here and
/// seeded below for the pass that reads it through the guard (D8).
fn read(store: &FakeStore, object: &str, name: &str) -> Value {
    let at = commands::now(store).expect("a point");
    derived::evidence(store, &Reading::new("local", at), object, name)
        .unwrap_or_else(|| panic!("`{name}` is answered by no binding"))
}

/// One pass of the loop over everything, as a host's tick takes it.
fn pass(store: &mut FakeStore, defs: &Definitions) {
    commands::tick(store, defs, &Scope::All, |_, _, _, _| true, |_, _, _| {}).expect("a pass");
}

/// What state an object's life is in, as the store holds it.
fn life(store: &FakeStore, id: &str) -> String {
    Records::get(store, id)
        .expect("a read")
        .expect("the object")
        .config
        .get("life")
        .cloned()
        .unwrap_or_default()
}

/// An object standing where an earlier pass left it: put, then moved to the
/// state its life region names, as the pass that merged or landed it did.
fn standing(
    store: &mut FakeStore,
    defs: &Definitions,
    id: &str,
    machine: &str,
    parent: Option<&str>,
    at_life: &str,
    record: &[(&str, Value)],
) {
    let at = commands::now(store).expect("a point");
    let record = record.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    commands::put_new(store, defs, id, machine, parent, record, at).expect("the object");
    let mut held = Records::get(store, id).expect("a read").expect("the object");
    held.config.retain(|region, _| !region.starts_with("life."));
    held.config.insert("life".into(), at_life.into());
    held.entered_at.insert("life".into(), at);
    let base = held.seq;
    Records::put(store, id, &held, base).expect("the object moved");
}

/// A chore of a repository's shared line, as `record_offers` makes one from a
/// curation session's offer (60, 62).
fn a_chore(repository: &str, claims: Value) -> Vec<(&'static str, Value)> {
    vec![
        ("type", json!("chore")),
        ("type_version", json!(2)),
        ("batch", json!(repository.to_string())),
        ("repository", json!(repository.to_string())),
        ("scope", json!("shared-line")),
        ("document", json!("flywheel/curation/chores/agents-md.md")),
        ("claims", claims),
    ]
}

/// A chore under a repository is done on the pass that merges it: nothing
/// stands above it to land with, so merged takes it to landed at once, final,
/// carrying the claim it cited so that claim's cell is due again as at any
/// landing (60, 62, 64).
#[test]
fn a_shared_line_chore_lands_when_it_merges() {
    let (mut store, defs) = a_store();
    let at = commands::now(&store).expect("a point");
    commands::put_new(&mut store, &defs, "repository/atlas", "repository", None, Default::default(), at)
        .expect("the repository");
    standing(
        &mut store,
        &defs,
        "unit/atlas/chore-1",
        "unit",
        Some("repository/atlas"),
        "merged",
        &a_chore("atlas", json!(["rows-are-numbered@3"])),
    );

    // The unit stands on a shared line: its parent is the repository, which is
    // the same fact its scope records (60, 123, `record-derived.yaml`).
    assert_eq!(read(&store, "unit/atlas/chore-1", "unit.shared_line"), json!(true));
    store.given("unit/atlas/chore-1", "unit.shared_line", json!(true));

    pass(&mut store, &defs);
    assert_eq!(
        life(&store, "unit/atlas/chore-1"),
        "landed",
        "a chore of a repository's shared line waited for a bolt that will never land (60)"
    );

    // Final, and carrying its claim: the landing the ledger reads is this one,
    // and nothing of the chore is reopened for it (64, `unit.yaml` landed).
    let landed = Records::get(&store, "unit/atlas/chore-1").expect("a read").expect("the chore");
    let region = defs
        .for_object("unit")
        .expect("the unit machine")
        .regions
        .get("life")
        .expect("the life region");
    assert!(region.states["landed"].is_final, "landed is not final");
    assert_eq!(landed.record.get("claims"), Some(&json!(["rows-are-numbered@3"])));

    pass(&mut store, &defs);
    assert_eq!(life(&store, "unit/atlas/chore-1"), "landed", "a landed chore moved again");
}

/// A chore of the blueprints' shared line stands under the instance and lands
/// the same way: its parent names the instance, and its scope says the shared
/// line (60, 123).
#[test]
fn a_blueprints_chore_under_the_instance_lands_when_it_merges() {
    let (mut store, defs) = a_store();
    let at = commands::now(&store).expect("a point");
    commands::put_new(&mut store, &defs, "instance/willdan", "instance", None, Default::default(), at)
        .expect("the instance");
    standing(
        &mut store,
        &defs,
        "unit/blueprints/chore-1",
        "unit",
        Some("instance/willdan"),
        "merged",
        &a_chore("blueprints", json!([])),
    );

    assert_eq!(read(&store, "unit/blueprints/chore-1", "unit.shared_line"), json!(true));
    store.given("unit/blueprints/chore-1", "unit.shared_line", json!(true));

    pass(&mut store, &defs);
    assert_eq!(life(&store, "unit/blueprints/chore-1"), "landed", "the blueprints' chore is still merged (123)");
}

/// A bolt's unit is unchanged: it merges onto the bolt's line and waits there
/// for the bolt, which lands them together (`unit.yaml` merged).
#[test]
fn a_bolts_unit_still_lands_with_its_bolt() {
    let (mut store, defs) = a_store();
    standing(
        &mut store,
        &defs,
        "bolt/atlas/rows",
        "bolt",
        None,
        "open",
        &[("repository", json!("atlas"))],
    );
    standing(
        &mut store,
        &defs,
        "unit/atlas/rows",
        "unit",
        Some("bolt/atlas/rows"),
        "merged",
        &[("type", json!("build")), ("claims", json!(["rows-are-numbered@3"]))],
    );

    assert_eq!(
        read(&store, "unit/atlas/rows", "unit.shared_line"),
        json!(false),
        "a unit of a bolt is not on the shared line"
    );

    pass(&mut store, &defs);
    assert_eq!(
        life(&store, "unit/atlas/rows"),
        "merged",
        "a merged unit landed while its bolt was still open"
    );

    // The bolt lands, and the unit lands with it.
    let mut bolt = Records::get(&store, "bolt/atlas/rows").expect("a read").expect("the bolt");
    bolt.config.retain(|region, _| !region.starts_with("life."));
    bolt.config.insert("life".into(), "landed".into());
    let base = bolt.seq;
    Records::put(&mut store, "bolt/atlas/rows", &bolt, base).expect("the bolt landed");

    pass(&mut store, &defs);
    assert_eq!(life(&store, "unit/atlas/rows"), "landed", "the unit did not land with its bolt");
}

/// A chore that resolves an intent's line take conflict stands under the
/// intent, and its line lands as the intent closes — the second state the
/// parent guard names (52, `unit.yaml` merged).
#[test]
fn an_intents_conflict_chore_lands_as_the_intent_closes() {
    let (mut store, defs) = a_store();
    standing(&mut store, &defs, "intent/mobile-speed", "intent", None, "open", &[]);
    standing(
        &mut store,
        &defs,
        "unit/mobile-speed/chore-1",
        "unit",
        Some("intent/mobile-speed"),
        "merged",
        &[("type", json!("chore")), ("type_version", json!(2)), ("scope", json!("bolt-line"))],
    );

    assert_eq!(
        read(&store, "unit/mobile-speed/chore-1", "unit.shared_line"),
        json!(false),
        "a chore of an intent's line is not on the shared line (52)"
    );

    pass(&mut store, &defs);
    assert_eq!(
        life(&store, "unit/mobile-speed/chore-1"),
        "merged",
        "the conflict chore landed while its intent was open (52)"
    );

    let mut intent = Records::get(&store, "intent/mobile-speed").expect("a read").expect("the intent");
    intent.config.retain(|region, _| !region.starts_with("life."));
    intent.config.insert("life".into(), "closed".into());
    let base = intent.seq;
    Records::put(&mut store, "intent/mobile-speed", &intent, base).expect("the intent closed");

    pass(&mut store, &defs);
    assert_eq!(
        life(&store, "unit/mobile-speed/chore-1"),
        "landed",
        "the conflict chore did not land as its intent closed (52)"
    );
}
