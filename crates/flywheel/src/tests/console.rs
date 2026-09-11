//! The commands over the trait surface: nothing here names one store's fields.
//!
//! Each one runs over a state repository of its own — a bare repository on this
//! computer and a checkout of it, which is the store this release binds (92).

use chrono::Utc;
use flywheel_scenario::console;
use flywheel_atoms::{Records, Scope, StateStore};
use flywheel_engine::runtime::Register;
use flywheel_scenario::Store;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A store over a state repository of its own (92, 160).
fn a_store() -> Store {
    let base = std::env::temp_dir().join(format!(
        "flywheel-console-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&base);
    let mut store = Store::default();
    store
        .bind_state_repository(&base, &["local".to_string()])
        .expect("the state repository opens");
    store
}

fn lamp() -> flywheel_engine::Definitions {
    let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .join("conformance/contract/lamp");
    flywheel_engine::load::load_dir(&dir).expect("the toy machine loads")
}

#[test]
fn the_register_is_the_rail_record() {
    // The rail's register is the rail record, reachable through `get` like
    // anything else (`profiles/record-derived.yaml`) — no seventh operation.
    let mut store = a_store();
    let mut register = Register::default();
    register.next_number = 412;
    register.entries.insert(
        "lamp/1/lamp-off".into(),
        flywheel_engine::runtime::RegisterEntry { number: 412, ..Default::default() },
    );
    console::set_register(&mut store, &register, &["lamp/1/lamp-off".into()]).unwrap();

    let read = console::register(&store).unwrap();
    assert_eq!(read.next_number, 412);
    assert_eq!(read.number_of("lamp/1/lamp-off"), Some(412));

    // And it is reachable through `get`, not through a field.
    let record = store.get(console::RAIL).unwrap().expect("the rail record");
    assert_eq!(record.machine, "rail");
    assert!(record.record.contains_key("numbers"));

    // It is not an object the engine ticks over.
    assert!(store
        .list(&Scope::All)
        .unwrap()
        .objects
        .iter()
        .all(|o| o.id != console::RAIL));
}

#[test]
fn seed_rail_and_tick_go_through_the_store() {
    let defs = lamp();
    let mut store = a_store();

    // seed: every object through `put`.
    let mut lamp = flywheel_engine::runtime::Object {
        id: "lamp/1".into(),
        machine: "lamp".into(),
        parent: None,
        config: Default::default(),
        entered_at: Default::default(),
        record: Default::default(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    };
    flywheel_engine::initialise(&defs, &mut lamp, Utc::now());
    assert_eq!(console::seed(&mut store, &defs, &[lamp], Some(412)).unwrap(), 1);
    assert_eq!(store.get("lamp/1").unwrap().unwrap().seq, 1);
    assert_eq!(console::register(&store).unwrap().next_number, 412);

    // rail: the objects from `list`, the numbers from the rail record.
    let decisions = console::rail(&mut store, &defs).unwrap();
    assert_eq!(decisions.len(), 1, "the lamp is off and asks to be turned on");
    assert_eq!(decisions[0].kind, "lamp-off");
    assert_eq!(decisions[0].number, Some(412));
    assert_eq!(
        console::register(&store).unwrap().next_number,
        413,
        "a number given is a number spent (15)"
    );

    // respond: through `receive`, with the record written before the transition.
    let (id, journal) = console::respond(&mut store, &defs, 412, "on", "chuck").unwrap();
    assert_eq!(id, "page-1");
    assert!(journal.iter().any(|n| n.kind == "create"));
    assert!(store.get(&format!("response/{id}")).unwrap().is_some());

    // tick: read through `list` and `read`, write through `put`, and hand the
    // effects to the binding.
    let mut performed: Vec<String> = vec![];
    let mut noted: Vec<String> = vec![];
    let ticked = console::tick(
        &mut store,
        &defs,
        &Scope::All,
        |_store, _object, _region, effect| {
            performed.push(effect.name.clone());
            true
        },
        |_store, fired, _tail| noted.push(format!("{} {}→{}", fired.object, fired.from, fired.to)),
    )
    .unwrap();
    assert_eq!(ticked.transitions, 1, "the lamp lights on the response");
    assert_eq!(performed, vec!["light".to_string()]);
    assert!(noted.iter().any(|n| n.contains("lamp/1 off→on")));
    assert_eq!(
        store
            .get("lamp/1")
            .unwrap()
            .unwrap()
            .config
            .get("power")
            .map(String::as_str),
        Some("on")
    );
    // The response took effect exactly once (137).
    assert_eq!(
        store.get("lamp/1").unwrap().unwrap().applied_responses,
        vec!["page-1".to_string()]
    );
    assert!(console::rail(&mut store, &defs).unwrap().is_empty());
}

#[test]
fn a_response_number_is_never_reused() {
    // The delivery's number comes from what the store holds, so a gap in the
    // ids does not hand one back (15).
    let defs = lamp();
    let mut store = a_store();
    let record: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    console::put_new(
        &mut store,
        &defs,
        "response/lost-1",
        "response",
        None,
        record,
        Utc::now(),
    )
    .unwrap();
    let (id, _) = console::dictate(&mut store, &defs, "lamp/1", "on", "chuck").unwrap();
    assert_eq!(id, "page-1", "a response of another delivery takes no page number");
    let (id, _) = console::dictate(&mut store, &defs, "lamp/1", "on", "chuck").unwrap();
    assert_eq!(id, "page-2");
}
