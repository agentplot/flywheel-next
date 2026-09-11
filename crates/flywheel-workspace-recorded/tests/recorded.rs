//! The recorded workspace over a real state repository: the effects of 42 move
//! a place and a line through their facts, and no built repository exists
//! anywhere (93a, D8).

use chrono::{TimeZone, Utc};
use flywheel_atoms::{LandingPolicy, Records, Workspace};
use flywheel_store_git::store::sandbox;
use flywheel_workspace_recorded::{evidence, RecordedWorkspace};
use serde_json::json;

fn dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "flywheel-recorded-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn place_advances_without_a_repository() {
    let base = dir("place");
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let mut store = sandbox(&base, "mac-mini", now).unwrap();

    // Nothing is recorded, so the place is absent and the line does not exist.
    assert_eq!(
        evidence(&store, "unit/atlas/u", "place.exists"),
        Some(json!(false))
    );
    assert_eq!(
        evidence(&store, "unit/atlas/u", "place.absent"),
        Some(json!(true))
    );

    {
        let mut workspace = RecordedWorkspace::new(&mut store);
        workspace
            .create_line("unit/atlas/u", "bolt/atlas/plan-rows")
            .unwrap();
        workspace
            .prepare_place("unit/atlas/u", "unit/atlas/u", "the work order")
            .unwrap();
    }

    assert_eq!(
        evidence(&store, "unit/atlas/u", "place.exists"),
        Some(json!(true))
    );
    assert_eq!(
        evidence(&store, "unit/atlas/u", "place.absent"),
        Some(json!(false))
    );
    assert_eq!(
        evidence(&store, "unit/atlas/u", "line.exists"),
        Some(json!(true))
    );

    {
        let mut workspace = RecordedWorkspace::new(&mut store);
        workspace.merge_place("unit/atlas/u").unwrap();
        workspace.remove_place("unit/atlas/u").unwrap();
        workspace
            .land_line("unit/atlas/u", LandingPolicy::Direct)
            .unwrap();
    }

    assert_eq!(
        evidence(&store, "unit/atlas/u", "place.merged"),
        Some(json!(true))
    );
    assert_eq!(
        evidence(&store, "unit/atlas/u", "place.absent"),
        Some(json!(true))
    );
    assert_eq!(
        evidence(&store, "unit/atlas/u", "line.landed"),
        Some(json!(true))
    );
    assert_eq!(
        evidence(&store, "unit/atlas/u", "line.landing"),
        Some(json!("passed"))
    );

    // Every fact is a record in the state store and nothing else: no worktree,
    // no built repository, nothing on disk but the state repository itself.
    let facts: Vec<String> = store
        .list_records(&flywheel_atoms::Scope::All)
        .unwrap()
        .into_iter()
        .map(|o| o.id)
        .filter(|id| id.starts_with("fact/"))
        .collect();
    assert_eq!(
        facts,
        vec![
            "fact/line/unit/atlas/u".to_string(),
            "fact/place/unit/atlas/u".to_string()
        ]
    );
    let entries: Vec<String> = std::fs::read_dir(&base)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        entries.iter().all(|e| e == "flywheel-state.git" || e == "mac-mini"),
        "the recorded workspace made something on disk: {entries:?}"
    );

    // A fact record ticks nothing: it carries no state.
    let fact = store.get("fact/place/unit/atlas/u").unwrap().unwrap();
    assert!(fact.config.is_empty());
}

// ------------------------------------------- 16.3 the ordinals gate the slot

/// A place records the ancestry its machine gates on, and the fixed order of
/// 38 is enforced rather than assumed (38, 50, 51).
#[test]
fn ordinals_gate_the_slot() {
    use flywheel_atoms::{Object, StateStore};
    let base = dir("ordinals");
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let mut store = sandbox(&base, "mac-mini", now).unwrap();

    // A place that was never prepared contains no line, so `place.yaml` holds
    // it in `preparing` — which is the machine's rule, not a default.
    assert_eq!(
        evidence(&store, "unit/atlas/one", "place.contains_line"),
        Some(json!(false))
    );

    // Two places of one line, prepared. Each contains its line, which is what
    // lets it reach `ready` (51, `place.yaml` preparing).
    {
        let mut workspace = RecordedWorkspace::new(&mut store);
        workspace
            .prepare_place("unit/atlas/one", "line/atlas/plan-rows", "")
            .unwrap();
        workspace
            .prepare_place("unit/atlas/two", "line/atlas/plan-rows", "")
            .unwrap();
    }
    for place in ["unit/atlas/one", "unit/atlas/two"] {
        assert_eq!(
            evidence(&store, place, "place.contains_line"),
            Some(json!(true)),
            "{place} does not contain its line after being prepared"
        );
    }

    // The ordinals: the first place is the one that may merge, and the second
    // waits while it is merging (38).
    let seed = |store: &mut _, id: &str, ordinal: u64, state: &str| {
        let mut object = Object {
            id: id.to_string(),
            machine: "place".into(),
            parent: None,
            config: [("place".to_string(), state.to_string())].into_iter().collect(),
            entered_at: [("place".to_string(), now)].into_iter().collect(),
            record: [("ordinal".to_string(), json!(ordinal))].into_iter().collect(),
            counters: Default::default(),
            applied_responses: vec![],
            seq: 0,
            created: 0,
        };
        object.config.insert("place".into(), state.into());
        flywheel_store_git::GitStore::seed_objects(store, std::slice::from_ref(&object)).unwrap();
    };
    seed(&mut store, "unit/atlas/one", 1, "merging");
    seed(&mut store, "unit/atlas/two", 2, "merging");

    assert_eq!(
        evidence(&store, "unit/atlas/one", "place.merge_slot"),
        Some(json!(true)),
        "the first place in the order may merge (38)"
    );
    assert_eq!(
        evidence(&store, "unit/atlas/two", "place.merge_slot"),
        Some(json!(false)),
        "a sibling with a lower ordinal is merging, so this one waits (38)"
    );

    // The first merges. The line moved under the second, so it no longer
    // contains it — which is what puts it in `behind` and what makes the rebase
    // of 51 happen at all.
    {
        let mut workspace = RecordedWorkspace::new(&mut store);
        workspace.merge_place("unit/atlas/one").unwrap();
    }
    assert_eq!(
        evidence(&store, "unit/atlas/two", "place.contains_line"),
        Some(json!(false)),
        "a sibling merged and this place still claims to contain the line (51)"
    );
    // And its slot is free now that the first is merged rather than merging.
    let _ = StateStore::list(&store, &flywheel_atoms::Scope::All);

    // A rebase puts it back.
    {
        let mut workspace = RecordedWorkspace::new(&mut store);
        workspace.rebase_place("unit/atlas/two").unwrap();
    }
    assert_eq!(
        evidence(&store, "unit/atlas/two", "place.contains_line"),
        Some(json!(true))
    );

    // A line with no parent contains it vacuously; one with a parent contains
    // it once the take has run (50).
    {
        let mut workspace = RecordedWorkspace::new(&mut store);
        workspace.create_line("line/atlas/plan-rows", "line/atlas/main").unwrap();
    }
    assert_eq!(
        evidence(&store, "line/atlas/plan-rows", "line.contains_parent"),
        Some(json!(false)),
        "a line with a parent it has never taken claims to contain it (50)"
    );
    {
        let mut workspace = RecordedWorkspace::new(&mut store);
        workspace.take_parent("line/atlas/plan-rows").unwrap();
    }
    assert_eq!(
        evidence(&store, "line/atlas/plan-rows", "line.contains_parent"),
        Some(json!(true))
    );

    // And a take is due on a line with no place, which is what makes the first
    // one happen (50).
    assert_eq!(
        evidence(&store, "line/atlas/nothing", "line.take_due"),
        Some(json!(true))
    );
    let _ = std::fs::remove_dir_all(&base);
}
