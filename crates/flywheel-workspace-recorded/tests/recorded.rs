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
