//! The rail's own record: the register, and the states its own machine is in
//! (`engine/rail.yaml`, 148).

use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::Records;
use flywheel_engine::runtime::Register;

/// The rail is an object like any other. The register is four fields of its
/// record; the states its two regions are in are the machine's, and writing the
/// register back must not take them away.
///
/// A writer that built a fresh object here erased them on every derive, so the
/// rail's machine started from nothing on every tick, its `status` region never
/// held a state to re-enter, and the rail's own record was rewritten on the
/// shared line every pass for no change (78, 148).
#[test]
fn writing_the_register_keeps_the_rails_own_state() {
    let mut store = FakeStore::default();
    let defs = crate::set::load().expect("the shipped set");

    // The rail as a tick makes it: the object, initialised into its machine's
    // initial states.
    let mut rail = flywheel_engine::Object {
        id: crate::RAIL.into(),
        machine: "rail".into(),
        parent: None,
        config: Default::default(),
        entered_at: Default::default(),
        record: Default::default(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    };
    flywheel_engine::initialise(&defs, &mut rail, chrono::Utc::now());
    let states = rail.config.clone();
    assert!(!states.is_empty(), "the rail machine has regions to be in");
    store.put(crate::RAIL, &rail, 0).unwrap();

    let mut register = Register::default();
    let number = register.number_for("lamp/1/lamp-proposed/2026-01-01T00:00:00Z", None);
    crate::commands::set_register(
        &mut store,
        &register,
        &["lamp/1/lamp-proposed/2026-01-01T00:00:00Z".to_string()],
    )
    .unwrap();

    let held = store.get(crate::RAIL).unwrap().expect("the rail is there");
    assert_eq!(
        held.config, states,
        "writing the register took the rail's own states away"
    );
    assert_eq!(
        crate::commands::register(&store).unwrap().number_of("lamp/1/lamp-proposed/2026-01-01T00:00:00Z"),
        Some(number),
        "and the register it wrote reads back"
    );
    assert_eq!(
        crate::commands::standing(&store).unwrap(),
        vec!["lamp/1/lamp-proposed/2026-01-01T00:00:00Z".to_string()]
    );
}
