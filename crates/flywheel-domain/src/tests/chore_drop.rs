//! A drop on a proposed chore lands (6, 129, `unit.yaml` v5 proposed).
//!
//! The proposed state offered `drop` among its answers and no transition took
//! it, so a drop on one row of a fold, or on the whole card, was recorded and
//! never applied — and never reported either, since the response machine read
//! only whether the decision still stood. The operator dropped a chore and the
//! chore stayed on the rail. One response is enough and the operator never
//! nudges (13), and a response that cannot be applied is reported and never
//! silently discarded (6, 154).

use crate::commands;
use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::{Records, Scope};
use flywheel_engine::runtime::{Response, ResponseKind};
use flywheel_engine::Definitions;
use serde_json::json;

fn a_store() -> (FakeStore, Definitions) {
    let defs = crate::set::load().expect("the embedded definitions");
    (FakeStore::default(), defs)
}

/// A repository's proposed shared-line chores, as `record_offers` makes them
/// from a curation session's offers: one batch, so they fold into one decision
/// (60, 62).
fn chores_of_repository(store: &mut FakeStore, defs: &Definitions, repository: &str, names: &[&str]) {
    let at = commands::now(store).expect("a point");
    for (row, name) in names.iter().enumerate() {
        let record = [
            ("type", json!("chore")),
            ("type_version", json!(2)),
            ("batch", json!(repository)),
            ("repository", json!(repository)),
            ("document", json!(format!("flywheel/curation/chores/{name}.md"))),
            ("sources", json!([format!("curation/willdan/main/1#{row}")])),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        let id = format!("unit/{repository}/chore-{}", row + 1);
        commands::put_new(store, defs, &id, "unit", Some(&format!("repository/{repository}")), record, at)
            .expect("the chore");
    }
}

/// One pass of the loop over everything, as a host's tick takes it.
fn pass(store: &mut FakeStore, defs: &Definitions) {
    commands::tick(store, defs, &Scope::All, |_, _, _, _| true, |_, _, _| {}).expect("a pass");
}

/// What state a chore's life is in, as the store holds it.
fn life(store: &mut FakeStore, id: &str) -> String {
    Records::get(store, id)
        .expect("a read")
        .expect("the chore")
        .config
        .get("life")
        .cloned()
        .unwrap_or_default()
}

/// The operator's answer, as the catalogue records it: the fold's number, and
/// the chore itself where the answer named one row (S232, 193).
fn answer(id: &str, number: u32, object: Option<&str>, said: &str) -> Response {
    Response {
        id: id.to_string(),
        kind: ResponseKind::Answer,
        decision: Some(number),
        object: object.map(String::from),
        answer: said.to_string(),
        given_by: "chuck".into(),
        given_at: chrono::Utc::now(),
        delivery: "page".into(),
    }
}

/// The drop the card offers is taken: on one row of a fold it drops that chore
/// and leaves the rest standing under the same number, and on the whole card it
/// drops every row still standing (6, 129, S232, `unit.yaml` v5).
#[test]
fn a_drop_on_a_proposed_chore_drops_it() {
    let (mut store, defs) = a_store();
    chores_of_repository(&mut store, &defs, "atlas", &["agents-md", "rename-ref", "citation-fix"]);
    let standing = commands::rail(&mut store, &defs).expect("the rail derives");
    let number = standing
        .iter()
        .find(|d| d.object == "unit/atlas/chore-1")
        .and_then(|d| d.number)
        .expect("the fold is numbered");

    // One row: `415b`, which the catalogue records against that chore alone.
    flywheel_atoms::StateStore::receive(
        &mut store,
        &answer("page-1", number, Some("unit/atlas/chore-2"), "drop"),
    )
    .expect("the drop is recorded");
    pass(&mut store, &defs);
    assert_eq!(
        life(&mut store, "unit/atlas/chore-2"),
        "dropped",
        "the drop on one row of the fold was recorded and never applied (6, 129)"
    );
    for standing in ["unit/atlas/chore-1", "unit/atlas/chore-3"] {
        assert_eq!(
            life(&mut store, standing),
            "proposed",
            "{standing} was dropped by an answer that named another row (S232)"
        );
    }

    // The fold keeps its number while any chore of its batch stands, and the
    // card's own drop declines every row still standing (15, S232).
    let after = commands::rail(&mut store, &defs).expect("the rail derives");
    let held = after
        .iter()
        .find(|d| d.folds.iter().any(|folded| folded == "unit/atlas/chore-1"))
        .expect("the fold stands");
    assert_eq!(held.number, Some(number), "the fold took a new number when a row left (15)");

    flywheel_atoms::StateStore::receive(&mut store, &answer("page-2", number, None, "drop"))
        .expect("the drop is recorded");
    pass(&mut store, &defs);
    for dropped in ["unit/atlas/chore-1", "unit/atlas/chore-3"] {
        assert_eq!(
            life(&mut store, dropped),
            "dropped",
            "the card's drop left {dropped} standing (6, S232)"
        );
    }

    // And nothing is left on the rail to answer twice (13).
    let rail = commands::rail(&mut store, &defs).expect("the rail derives");
    assert!(
        !rail.iter().any(|d| d.kind == "unit-proposed"),
        "a dropped fold is still asking: {rail:?}"
    );
}
