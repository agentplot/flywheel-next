//! The instance's own types, over the core set (57, 85, 223, 224).

use crate::blueprints;
use std::path::{Path, PathBuf};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/blueprints")
}

/// A type file in the blueprints is a machine the engine runs: no code change,
/// no new binary, no host restarted for one (57, 85).
#[test]
fn type_from_blueprints_runs() {
    let core = crate::set::load().expect("the core set parses");
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
    let core = crate::set::load().unwrap();
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

/// A type is defined where the registry in force holds it at some version: the
/// binary's own, bare or pinned, or the instance's once its blueprints are read.
/// Null, empty, a core machine or a name no entry has is not (85a).
#[test]
fn a_type_is_defined_where_the_registry_holds_it() {
    let core = crate::set::load().expect("the core set parses");
    for held in ["chore", "chore@2", "default", "fast", "self-closing", "standing", "with-operator"] {
        assert!(blueprints::type_defined(&core, Some(held)), "`{held}` is registered");
    }
    for missing in [None, Some(""), Some("  "), Some("from-material"), Some("spike"), Some("line"), Some("unit")] {
        assert!(!blueprints::type_defined(&core, missing), "{missing:?} is no registered type");
    }
    let loaded = blueprints::load_over_core(&fixture()).expect("the blueprints read");
    assert!(
        blueprints::type_defined(&loaded.defs, Some("spike")),
        "the instance's own type is defined once its blueprints are read (57, 85)"
    );
}

/// `unit.type_defined` and `elaboration.type_defined` answer from the object's
/// own record, and no other read is theirs (85a).
#[test]
fn the_type_reads_answer_from_the_record() {
    let defs = crate::set::load().unwrap();
    let mut store = flywheel_atoms::testing::FakeStore::default();
    let at = crate::commands::now(&store).unwrap();
    for (id, machine, kind) in [("unit/atlas/one", "unit", "chore"), ("elaboration/limits/one", "elaboration", "from-material")] {
        let record = [("type".to_string(), serde_json::json!(kind))].into_iter().collect();
        crate::commands::put_new(&mut store, &defs, id, machine, None, record, at).unwrap();
    }
    assert_eq!(blueprints::evidence(&store, &defs, "unit/atlas/one", "unit.type_defined"), Some(serde_json::json!(true)));
    assert_eq!(
        blueprints::evidence(&store, &defs, "elaboration/limits/one", "elaboration.type_defined"),
        Some(serde_json::json!(false))
    );
    assert_eq!(blueprints::evidence(&store, &defs, "unit/atlas/one", "unit.type"), None);
}

/// The type reads answered from each unit's and elaboration's own record, as
/// the host answers them, before a tick reads them.
fn answer_the_type_reads(store: &mut flywheel_atoms::testing::FakeStore, defs: &flywheel_engine::Definitions) {
    use flywheel_atoms::Records;
    for held in store.list_records(&flywheel_atoms::Scope::All).unwrap() {
        let name = match held.machine.as_str() {
            "unit" => "unit.type_defined",
            "elaboration" => "elaboration.type_defined",
            _ => continue,
        };
        let value = blueprints::evidence(store, defs, &held.id, name).unwrap();
        store.given(&held.id, name, value);
    }
}

/// Tick until nothing moves, keeping every effect asked for and every state
/// entered.
fn settle(
    store: &mut flywheel_atoms::testing::FakeStore,
    defs: &flywheel_engine::Definitions,
    asked: &mut Vec<String>,
    entered: &mut Vec<String>,
) {
    for _ in 0..30 {
        answer_the_type_reads(store, defs);
        let before = store.writes();
        crate::commands::tick(
            store,
            defs,
            &flywheel_atoms::Scope::All,
            |store, object, _, effect| {
                asked.push(format!("{object} {}", effect.name));
                match effect.name.as_str() {
                    "set_type" => {
                        let kind = effect.args.get("type").and_then(|v| v.as_str()).unwrap_or_default();
                        crate::effects::set_type(store, object, kind).is_ok()
                    }
                    _ => true,
                }
            },
            |_, fired, _| entered.push(format!("{} {}", fired.object, fired.to)),
        )
        .unwrap();
        if store.writes() == before {
            return;
        }
    }
    panic!("the instance did not settle");
}

/// An elaboration or a unit whose type the instance has no definition of never
/// reaches working. A yes leaves it proposed with nothing started, its decision
/// offers `type <name>`, and once a defined type is set a yes takes it on: over
/// an elaboration, a unit proposed on its own, and a unit named by a proposal
/// whose yes did not carry it (85a, S6).
#[test]
fn an_undefined_type_is_refused_at_approval_and_offers_type() {
    use flywheel_atoms::Records;
    use serde_json::json;
    let defs = crate::set::load().unwrap();
    let mut store = flywheel_atoms::testing::FakeStore::default();
    let at = crate::commands::now(&store).unwrap();
    let put = |store: &mut flywheel_atoms::testing::FakeStore, id: &str, machine: &str, parent: Option<&str>, config: &[(&str, &str)], record: &[(&str, serde_json::Value)]| {
        let record = record.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        crate::commands::put_new(store, &defs, id, machine, parent, record, at).unwrap();
        let mut held = Records::get(store, id).unwrap().unwrap();
        for (region, state) in config {
            held.config.insert(region.to_string(), state.to_string());
        }
        let base = held.seq;
        Records::put(store, id, &held, base).unwrap();
    };
    put(&mut store, "intent/limits", "intent", None, &[("life", "open"), ("life.open.material", "settled"), ("life.open.close", "not-offered")], &[]);
    let elaboration = "elaboration/limits/research";
    put(&mut store, elaboration, "elaboration", Some("intent/limits"), &[], &[("type", json!("from-material")), ("covers", json!(["intent/limits"]))]);
    let alone = "unit/atlas/spike";
    put(&mut store, alone, "unit", None, &[], &[("repository", json!("atlas")), ("type", json!("spike")), ("target", json!({"bolt": "bolt/atlas/plan-rows"}))]);
    let carried = "unit/atlas/carried";
    put(&mut store, carried, "unit", None, &[("life", "in-proposal")], &[("repository", json!("atlas")), ("type", json!("spike")), ("proposal", json!("proposal/atlas/1")), ("target", json!({"bolt": "bolt/atlas/plan-rows"}))]);
    // The proposal naming the second unit was approved; a place comes up
    // current, and the units' dependencies are merged.
    store.given(carried, "unit.proposal", json!("approved"));
    for unit in [alone, carried] {
        store.given(unit, "unit.deps_merged", json!(true));
    }
    for name in ["place.exists", "place.contains_line", "place.endpoints_recorded"] {
        store.given("*", name, json!(true));
    }
    let cases = [(elaboration, "self-closing", "working"), (alone, "chore", "in-flight"), (carried, "chore", "in-flight")];
    let life = |store: &flywheel_atoms::testing::FakeStore, id: &str| Records::get(store, id).unwrap().unwrap().config.get("life").cloned().unwrap_or_default();
    let decision = |store: &mut flywheel_atoms::testing::FakeStore, id: &str| {
        let standing = crate::commands::rail(store, &defs).unwrap();
        let held = standing.into_iter().find(|d| d.object == id).unwrap_or_else(|| panic!("{id} stands on no decision"));
        (held.number.expect("a number"), held.answers)
    };
    let (mut asked, mut entered) = (Vec::new(), Vec::new());

    settle(&mut store, &defs, &mut asked, &mut entered);
    for (id, _, _) in cases {
        assert_eq!(life(&store, id), "proposed", "{id} is not standing proposed");
        let (number, answers) = decision(&mut store, id);
        assert!(answers.iter().any(|a| a == "type <name>"), "{id}'s decision offers no type: {answers:?}");
        crate::commands::respond(&mut store, &defs, number, "yes", "chuck").unwrap();
    }
    settle(&mut store, &defs, &mut asked, &mut entered);
    for (id, _, _) in cases {
        assert_eq!(life(&store, id), "proposed", "a yes took {id} on with no type the instance knows (85a)");
        let started: Vec<&String> = asked.iter().filter(|a| a.starts_with(&format!("{id} "))).collect();
        assert!(started.is_empty(), "something was started for {id}: {started:?}");
    }

    for (id, kind, _) in cases {
        let (number, _) = decision(&mut store, id);
        crate::commands::respond(&mut store, &defs, number, &format!("type {kind}"), "chuck").unwrap();
    }
    settle(&mut store, &defs, &mut asked, &mut entered);
    for (id, kind, _) in cases {
        assert_eq!(life(&store, id), "proposed", "setting a type is not a yes");
        let held = Records::get(&store, id).unwrap().unwrap();
        assert_eq!(held.record.get("type"), Some(&json!(kind)));
        let (number, _) = decision(&mut store, id);
        crate::commands::respond(&mut store, &defs, number, "yes", "chuck").unwrap();
    }
    settle(&mut store, &defs, &mut asked, &mut entered);
    for (id, _, working) in cases {
        assert!(
            entered.iter().any(|e| *e == format!("{id} {working}")),
            "{id} never reached {working} once its type was defined: {entered:?}"
        );
    }
}
