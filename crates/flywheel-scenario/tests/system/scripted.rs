//! The scripted session as a process of its own: `flywheel exit` run as the
//! real command from the session's place (D17, 67, 93b).

use flywheel_atoms::scenario::{Offer, ScriptEntry};
use flywheel_atoms::Records;
use flywheel_scenario::sessions::{ScriptedSessions, BINARY_ENV};
use flywheel_scenario::Store;
use serde_json::json;
use std::path::PathBuf;

/// The `flywheel` binary beside the test binary.
fn flywheel_binary() -> PathBuf {
    let deps = std::env::current_exe().expect("the test binary has a path");
    let target = deps.parent().and_then(|p| p.parent()).expect("target/debug");
    let binary = target.join("flywheel");
    if !binary.exists() {
        let out = std::process::Command::new(env!("CARGO"))
            .args(["build", "--quiet", "-p", "flywheel"])
            .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
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

const SESSION: &str = "elaboration/a/1/self-closing/1";

/// The host whose checkout the command writes through (232).
const HOST: &str = "local";

#[test]
fn scripted_exit_goes_through_the_command() {
    let dir = std::env::temp_dir().join(format!("flywheel-scripted-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let state = dir.join("state");
    std::env::set_var(BINARY_ENV, flywheel_binary());

    // The command writes through this host's checkout of the state repository,
    // which is the only store there is (92).
    let mut store = Store::default();
    let git = flywheel_store_git::store::sandbox(&state, HOST, store.now).unwrap();
    store.acting_host = Some(HOST.to_string());
    store
        .durable
        .insert(HOST.to_string(), std::sync::Arc::new(std::sync::Mutex::new(git)));
    let sessions = ScriptedSessions::new(&state, dir.join("places"));

    let entry = ScriptEntry {
        after: Some("2".into()),
        pane: Some("present".into()),
        activity: Some("working".into()),
        exit: Some("done".into()),
        deliverables: vec!["research.md".into()],
        offers: vec![Offer {
            kind: "chore".into(),
            document: "chores/1.md".into(),
        }],
        ..Default::default()
    };
    sessions
        .play_entry(&mut store, SESSION, &entry)
        .expect("the script plays");

    // The assertion is about the thread the command wrote, never the script.
    let thread = store.thread(SESSION).unwrap();
    assert_eq!(
        thread.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        vec!["exit", "offer"]
    );
    assert_eq!(thread[0].fields.get("exit"), Some(&json!("done")));
    assert_eq!(
        thread[0].fields.get("deliverables"),
        Some(&json!(["research.md"]))
    );
    assert_eq!(thread[1].fields.get("document"), Some(&json!("chores/1.md")));

    // The state repository holds it too: the command wrote through the store,
    // not into the runner's memory (67).
    let mut read = flywheel_store_git::store::sandbox(&state, "reader", store.now).unwrap();
    read.fetch().unwrap();
    assert_eq!(Records::thread(&read, SESSION).unwrap().len(), 2);

    // What the multiplexer reports is a world fact, not a report.
    assert!(store.world.sessions[SESSION].pane);
    assert_eq!(store.world.sessions[SESSION].activity, "working");

    let _ = std::fs::remove_dir_all(&dir);
}
