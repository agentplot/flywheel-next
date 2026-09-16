//! A session its type does not keep is ended on the pass that records its
//! exit, and the host's reconciliation closes what exits left open (73, 74,
//! 196, `session.yaml` v5, `engine/host.yaml` v6).

use crate::report::{write_report, Report};
use crate::sessions::{layout_to_close, panes_to_end};
use flywheel_atoms::testing::FakeStore;
use flywheel_engine::{Definitions, Object};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn defs() -> Definitions {
    crate::set::load().expect("the shipped set loads")
}

/// A session fact as the bindings write one: started, with a pane in a herdr
/// session of the host's own.
fn a_session(store: &mut FakeStore, session: &str, fields: &[(&str, Value)]) {
    let mut record: BTreeMap<String, Value> = [
        ("started_at".to_string(), json!("2026-09-15T09:00:00Z")),
        ("ended_at".to_string(), Value::Null),
        ("host".to_string(), json!("mac-studio")),
        ("herdr_agent".to_string(), json!("a-session")),
        ("herdr_pane".to_string(), json!("w1:p1")),
        ("herdr_session".to_string(), json!("flywheel-willdan-machinery")),
    ]
    .into_iter()
    .collect();
    for (name, value) in fields {
        record.insert((*name).to_string(), value.clone());
    }
    store.seed(Object {
        id: format!("fact/session/{session}"),
        machine: "fact".to_string(),
        parent: None,
        config: Default::default(),
        entered_at: Default::default(),
        record,
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    });
}

fn exits(store: &mut FakeStore, session: &str, kind: &str) {
    write_report(
        store,
        session,
        "a-session",
        chrono::Utc::now(),
        &Report::Exit {
            kind: kind.to_string(),
            deliverables: vec![],
            question: None,
            text: None,
        },
    )
    .expect("the exit is written");
}

/// A session with keep_alive false is ended when its exit is recorded — done,
/// stalled and invalid alike, since what went wrong is the exit's report on the
/// thread and the work in the place, and the pane's scrollback was never state
/// (67, 68, 74).
#[test]
fn a_session_not_kept_is_ended_when_its_exit_is_recorded() {
    let defs = defs();
    for kind in ["done", "stalled", "invalid"] {
        let mut store = FakeStore::default();
        let session = format!("curation/willdan/{kind}/1");
        a_session(&mut store, &session, &[("keep_alive", json!(false))]);
        exits(&mut store, &session, kind);
        let ending = panes_to_end(&store, &defs).expect("the panes are read");
        assert_eq!(
            ending.iter().map(|p| p.session.as_str()).collect::<Vec<_>>(),
            vec![session.as_str()],
            "a {kind} exit ends the pane of a session its type does not keep"
        );
    }

    // And the machine is what does it on the pass that records the exit: the
    // transition reads `session.keep_alive` and runs `end_session`.
    let session = defs.get("session").expect("the session machine");
    let fires: Vec<String> = session
        .regions
        .values()
        .flat_map(|region| region.states.values())
        .flat_map(|state| state.transitions.iter())
        .filter(|t| t.effects.iter().any(|e| e.name == "end_session"))
        .map(|t| format!("{:?}", t.when))
        .collect();
    assert!(
        fires.iter().any(|guard| guard.contains("session.keep_alive")),
        "no transition ends a session its type does not keep: {fires:?}"
    );
}

/// Blocked is not an exit: the pane stands for the answer (70).
#[test]
fn a_blocked_session_keeps_its_pane() {
    let defs = defs();
    let mut store = FakeStore::default();
    a_session(&mut store, "unit/atlas/rows/fix/1", &[("keep_alive", json!(false))]);
    exits(&mut store, "unit/atlas/rows/fix/1", "blocked");
    assert_eq!(panes_to_end(&store, &defs).expect("the panes are read"), vec![]);
}

/// A standing or with-operator session stands after its exit until its owner
/// ends it (25, 26, 69).
#[test]
fn a_kept_session_stands_after_its_exit() {
    let defs = defs();
    let mut store = FakeStore::default();
    a_session(&mut store, "operator-session/chuck-1", &[("keep_alive", json!(true))]);
    exits(&mut store, "operator-session/chuck-1", "done");
    assert_eq!(panes_to_end(&store, &defs).expect("the panes are read"), vec![]);

    // A record written before keep_alive existed keeps its pane too: ending one
    // is the direction that cannot be undone.
    let mut store = FakeStore::default();
    a_session(&mut store, "operator-session/chuck-2", &[]);
    exits(&mut store, "operator-session/chuck-2", "done");
    assert_eq!(panes_to_end(&store, &defs).expect("the panes are read"), vec![]);
}

/// Reconciliation ends a pane still open for a session already exited or
/// ended, which covers a pass that recorded an exit and could not end its pane
/// (73, 74, 196).
#[test]
fn an_exited_sessions_open_pane_is_ended_by_reconciliation() {
    let defs = defs();
    let mut store = FakeStore::default();

    // One exited whose type does not keep it, and one whose record is ended
    // with its pane still listed.
    a_session(&mut store, "curation/willdan/main/1", &[("keep_alive", json!(false))]);
    exits(&mut store, "curation/willdan/main/1", "done");
    a_session(
        &mut store,
        "capture/folder-drop-1/main/1",
        &[("ended_at", json!("2026-09-15T10:00:00Z"))],
    );

    let ending = panes_to_end(&store, &defs).expect("the panes are read");
    let mut named: Vec<&str> = ending.iter().map(|p| p.session.as_str()).collect();
    named.sort();
    assert_eq!(named, vec!["capture/folder-drop-1/main/1", "curation/willdan/main/1"]);
    // Each carries the herdr session its pane is in, so the end addresses the
    // server the record names and never the operator's (174).
    assert!(ending.iter().all(|p| p.herdr_session == "flywheel-willdan-machinery"));
    assert!(ending.iter().all(|p| !p.why.is_empty()));
}

/// A machinery run's tab goes once the run is final and its pane is gone; the
/// machinery workspaces stay, because they are per kind and not per object
/// (186, 196).
#[test]
fn a_finished_runs_tab_is_closed_and_its_workspace_stays() {
    let defs = defs();
    let mut store = FakeStore::default();

    // The run that has finished, its pane already closed.
    a_session(
        &mut store,
        "curation/willdan/main/1",
        &[("ended_at", json!("2026-09-15T10:00:00Z"))],
    );
    // A run still going: its tab stays.
    a_session(&mut store, "curation/willdan/main/2", &[("keep_alive", json!(false))]);

    let workspaces = ["curation".to_string(), "capture-reading".to_string(), "planning/atlas".to_string()];
    let tabs = ["curation/willdan/main/1".to_string(), "curation/willdan/main/2".to_string()];
    let closing = layout_to_close(&store, &defs, &workspaces, &tabs).expect("the layout is read");

    assert_eq!(closing.tabs, vec!["curation/willdan/main/1".to_string()]);
    assert_eq!(
        closing.workspaces,
        Vec::<String>::new(),
        "the machinery workspaces are per kind and stay"
    );
}
