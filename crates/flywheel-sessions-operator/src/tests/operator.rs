//! The operator as the session (93b): the record and the thread are all there
//! is, and the with-operator rules hold over a day of it (25).

use chrono::{Duration, TimeZone, Utc};
use flywheel_atoms::{Records, SessionPresence, Sessions, WorkOrder};
use crate::{evidence, session_fact, OperatorSessions};
use flywheel_atoms::testing::FakeStore;
use serde_json::json;
use std::sync::{Arc, Mutex};

fn order(session: &str) -> WorkOrder {
    WorkOrder {
        session: session.to_string(),
        kind: "elaboration".into(),
        place: "elaboration/atlas/research-1".into(),
        body: "the closed set of inputs, rendered".into(),
    }
}

#[test]
fn no_agent_started() {
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap();
    let store = Arc::new(Mutex::new(FakeStore::new("mac-mini")));
    let sessions = OperatorSessions::new(store.clone(), "mac-mini", now);
    let session = "session/elaboration/atlas/research-1/main";

    sessions.start_session(&order(session)).unwrap();

    // The session is recorded with its place and its work order, and shows as
    // the operator's to run.
    let held = store.lock().unwrap();
    let fact = held.get(&session_fact(session)).unwrap().unwrap();
    assert_eq!(fact.record.get("runner"), Some(&json!("operator")));
    assert_eq!(
        fact.record.get("place"),
        Some(&json!("elaboration/atlas/research-1"))
    );
    assert_eq!(
        fact.record.get("work_order"),
        Some(&json!("the closed set of inputs, rendered"))
    );
    // The work order reached the operator on the session's thread.
    let thread = held.thread(session).unwrap();
    assert_eq!(thread.len(), 1);
    assert_eq!(thread[0].kind, "work-order");
    drop(held);

    // The machinery observes it as a session like any other (72).
    assert_eq!(
        sessions.presence(session).unwrap(),
        SessionPresence::Alive
    );
    let held = store.lock().unwrap();
    assert_eq!(
        evidence(&*held, session, "session.pane"),
        Some(json!("present"))
    );
    assert_eq!(
        evidence(&*held, session, "session.activity"),
        Some(json!("working"))
    );
    drop(held);

    // And no agent was started: this crate names no agent program, no
    // multiplexer and no process at all.
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"),
    )
    .unwrap();
    for named in ["Command", "herdr", "claude", "codex", "opencode", "spawn"] {
        assert!(
            !source.contains(named),
            "the operator runner names `{named}`; it starts no agent (93b)"
        );
    }

    // A second start of the same name is not a second session (72).
    sessions.start_session(&order(session)).unwrap();
    let held = store.lock().unwrap();
    assert_eq!(held.thread(session).unwrap().len(), 1);
}

#[test]
fn idle_offers_nothing() {
    let start = Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap();
    let store = Arc::new(Mutex::new(FakeStore::new("mac-mini")));
    let session = "session/operator-session/1/main";
    OperatorSessions::new(store.clone(), "mac-mini", start)
        .start_session(&order(session))
        .unwrap();

    // A simulated day: the operator touches nothing, and nothing is inferred
    // from that. `session.activity` never reads idle, so the session machine's
    // idle transition never holds and no stall clock ever starts (25, 93b).
    for hour in 0..24 {
        let now = start + Duration::hours(hour);
        let sessions = OperatorSessions::new(store.clone(), "mac-mini", now);
        assert_eq!(
            sessions.presence(session).unwrap(),
            SessionPresence::Alive,
            "hour {hour}"
        );
        let held = store.lock().unwrap();
        assert_eq!(
            evidence(&*held, session, "session.activity"),
            Some(json!("working")),
            "hour {hour}"
        );
        assert_eq!(
            evidence(&*held, session, "session.idle_since"),
            Some(serde_json::Value::Null),
            "hour {hour}"
        );
        assert_eq!(
            evidence(&*held, session, "session.exit"),
            Some(json!("none")),
            "hour {hour}"
        );
    }

    // And the type says the same thing the runner does: the with-operator
    // machine raises no decision anywhere, a done does not end it, and the one
    // transition that finishes it is the operator's dictation (25, 26).
    let defs = flywheel_domain::set::load().unwrap();
    let with_operator = defs
        .machines
        .get("with-operator")
        .expect("the core set carries the with-operator type");
    let mut endings = Vec::new();
    for region in with_operator.regions.values() {
        for (name, state) in &region.states {
            assert!(
                state.decision.is_none(),
                "the with-operator state `{name}` raises a decision (25)"
            );
            for transition in &state.transitions {
                if transition.to != *name {
                    endings.push(format!("{name} -> {}: {:?}", transition.to, transition.when));
                }
            }
        }
    }
    let leaves_the_session: Vec<&String> = endings
        .iter()
        .filter(|e| e.starts_with("session -> "))
        .collect();
    assert_eq!(
        leaves_the_session.len(),
        1,
        "more than one way out of a with-operator session: {leaves_the_session:?}"
    );
    assert!(
        leaves_the_session[0].contains("Response") && leaves_the_session[0].contains("end"),
        "a with-operator session ends by something other than the dictation `end`: {:?}",
        leaves_the_session[0]
    );

    // It ends when that dictation reaches it, and not before.
    let now = start + Duration::days(1);
    let sessions = OperatorSessions::new(store.clone(), "mac-mini", now);
    sessions.end_session(session).unwrap();
    assert_eq!(sessions.presence(session).unwrap(), SessionPresence::Absent);
}
