//! The stand-in plays a session's reports by running the command a real
//! session runs, and every assertion is about what that command wrote (67, 93).

use flywheel_atoms::scenario::{Offer, ScriptEntry};
use flywheel_atoms::Records;
use flywheel_scenario::sessions::{ScriptedSessions, BINARY_ENV};
use flywheel_scenario::Store;
use serde_json::json;
use std::path::PathBuf;

/// The `flywheel` binary beside the test binary. `cargo test -p
/// flywheel-scenario` does not build another package's binary, so build it
/// when it is not there; the runner itself is the binary and needs none of
/// this.
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

#[test]
fn scripted_exit_goes_through_the_command() {
    let dir = std::env::temp_dir().join(format!("flywheel-scripted-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let state = dir.join("store.json");
    std::env::set_var(BINARY_ENV, flywheel_binary());

    let mut store = Store::default();
    flywheel_scenario::save(&store, &state).unwrap();
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
        .play_entry(&mut store, SESSION, &entry, &state)
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

    // The store on disk holds it too: the command wrote through the store, not
    // into the runner's memory.
    let on_disk = flywheel_scenario::load(&state).unwrap();
    assert_eq!(on_disk.thread(SESSION).unwrap().len(), 2);

    // What the multiplexer reports is a world fact, not a report.
    assert!(store.world.sessions[SESSION].pane);
    assert_eq!(store.world.sessions[SESSION].activity, "working");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_command_runs_from_the_session_place() {
    let sessions = ScriptedSessions::new("state.json", "/tmp/places");
    assert_eq!(
        sessions.place(SESSION),
        PathBuf::from("/tmp/places/elaboration-a-1-self-closing-1"),
        "the command's working directory is the session's place (D8)"
    );
}
