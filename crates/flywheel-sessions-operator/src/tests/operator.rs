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

/// An offer is pending while no record points at it, and not one moment
/// longer (62, `atoms.yaml` session.offers_pending).
///
/// Read as "the thread carries an offer" it is true for ever, and the session
/// machine's `working` region prefers `record_offers` to every transition
/// beneath it — so a session that offered anything fired the effect on every
/// pass and never reached `exited`, and the elaboration, unit or curation
/// above it stood in `working` with a finished session inside it. Nothing in
/// the acceptance set could see it: the stand-in store answers this atom
/// correctly, so only a real host over the real binding was ever wrong.
#[test]
fn an_offer_is_pending_only_until_a_record_points_at_it() {
    let now = Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap();
    let store = Arc::new(Mutex::new(FakeStore::new("mac-mini")));
    let session = "session/elaboration/atlas/research-1/main";
    OperatorSessions::new(store.clone(), "mac-mini", now)
        .start_session(&order(session))
        .unwrap();

    // The objects the session runs under, because a finding is recorded on the
    // intent above it and there has to be one.
    let defs = flywheel_domain::set::load().unwrap();
    {
        let mut held = store.lock().unwrap();
        for (id, machine, parent) in [
            ("intent/atlas", "intent", None),
            (
                "elaboration/atlas/research-1",
                "elaboration",
                Some("intent/atlas"),
            ),
        ] {
            flywheel_domain::commands::put_new(
                &mut *held,
                &defs,
                id,
                machine,
                parent,
                Default::default(),
                now,
            )
            .unwrap();
        }
    }

    // Nothing offered: nothing pending.
    {
        let held = store.lock().unwrap();
        assert_eq!(
            evidence(&*held, session, "session.offers_pending"),
            Some(json!(false))
        );
    }

    // The session offers a finding through the reporting command's own path,
    // and exits done in the same breath, which is what a real one does.
    let document = "openspec/changes/atlas/findings/backoff.md";
    {
        let mut held = store.lock().unwrap();
        flywheel_domain::report::write_report(
            &mut *held,
            session,
            "operator",
            now,
            &flywheel_domain::report::Report::Offer {
                kind: "finding".into(),
                document: document.into(),
            },
        )
        .unwrap();
    }
    {
        let held = store.lock().unwrap();
        assert_eq!(
            evidence(&*held, session, "session.offers_pending"),
            Some(json!(true)),
            "an offer no record points at is pending"
        );
        assert_eq!(
            evidence(&*held, session, "session.offers_recorded"),
            Some(json!(false))
        );
    }

    // `record_offers` makes the record, and the offer stops being pending. The
    // two atoms are complements and must never both say yes.
    {
        let mut held = store.lock().unwrap();
        flywheel_domain::offers::record(
            &mut *held,
            &mut flywheel_atoms::testing::FakeWorld::new(),
            &defs,
            session,
            "elaboration/atlas/research-1",
            now,
        )
        .unwrap();
    }
    let held = store.lock().unwrap();
    assert_eq!(
        evidence(&*held, session, "session.offers_pending"),
        Some(json!(false)),
        "the offer is recorded; a session that keeps offering it never exits"
    );
    assert_eq!(
        evidence(&*held, session, "session.offers_recorded"),
        Some(json!(true))
    );

    // And the entry is still on the thread: what changed is that a record now
    // cites it, never that the thread was rewritten (67, 79).
    assert_eq!(
        held.thread(session)
            .unwrap()
            .iter()
            .filter(|e| e.kind == "offer")
            .count(),
        1
    );
}

/// An answer clears the block, and the answer is delivered exactly once
/// (68, 70, `session.yaml` alive.blocked).
///
/// A blocked session stopped only itself; the operator's response sends it back
/// to working and the machinery delivers the answer. Two reads made that
/// impossible over the real binding. `session.answer_delivered` was `true`
/// whatever the thread held, so the engine skipped `deliver_answer` (73, 127)
/// and no answer was ever written; and `session.exit` went on reading the
/// `blocked` entry after the answer, so the next pass read the session as
/// blocked again — `blocks` climbing, the same question standing, and no way
/// out of `blocked` but another response.
#[test]
fn an_answer_clears_the_block_and_is_delivered_once() {
    let start = Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap();
    let store = Arc::new(Mutex::new(FakeStore::new("mac-mini")));
    let session = "session/elaboration/atlas/research-1/main";
    OperatorSessions::new(store.clone(), "mac-mini", start)
        .start_session(&order(session))
        .unwrap();

    // Nothing is owed while nothing has been reported.
    {
        let held = store.lock().unwrap();
        assert_eq!(
            evidence(&*held, session, "session.exit"),
            Some(json!("none"))
        );
        assert_eq!(
            evidence(&*held, session, "session.answer_delivered"),
            Some(json!(true))
        );
    }

    // The session reports blocked with its question.
    let blocked = start + Duration::minutes(5);
    {
        let mut held = store.lock().unwrap();
        flywheel_domain::report::write_report(
            &mut *held,
            session,
            "operator",
            blocked,
            &flywheel_domain::report::Report::Exit {
                kind: "blocked".into(),
                deliverables: vec![],
                question: Some("which sandbox account?".into()),
                text: None,
            },
        )
        .unwrap();
    }
    {
        let held = store.lock().unwrap();
        assert_eq!(
            evidence(&*held, session, "session.exit"),
            Some(json!("blocked"))
        );
        assert_eq!(
            evidence(&*held, session, "session.answer_delivered"),
            Some(json!(false)),
            "the answer is owed, so `deliver_answer` must run"
        );
    }

    // The machinery delivers the operator's answer.
    let answered_at = blocked + Duration::minutes(2);
    let sessions = OperatorSessions::new(store.clone(), "mac-mini", answered_at);
    sessions.deliver_answer(session, "the sandbox under payments-sandbox").unwrap();
    {
        let held = store.lock().unwrap();
        assert_eq!(
            evidence(&*held, session, "session.answer_delivered"),
            Some(json!(true)),
            "delivered once; a second delivery would be a second answer"
        );
        // The block is over: the session is working again, not blocked.
        assert_eq!(
            evidence(&*held, session, "session.exit"),
            Some(json!("none"))
        );
        assert_eq!(
            evidence(&*held, session, "session.activity"),
            Some(json!("working"))
        );
        assert_eq!(
            evidence(&*held, session, "session.pane"),
            Some(json!("present"))
        );
    }

    // It carries on and exits for real; that exit is the one the owner reads.
    let done = answered_at + Duration::minutes(9);
    {
        let mut held = store.lock().unwrap();
        flywheel_domain::report::write_report(
            &mut *held,
            session,
            "operator",
            done,
            &flywheel_domain::report::Report::Exit {
                kind: "done".into(),
                deliverables: vec!["prototype/retry-report.html".into()],
                question: None,
                text: None,
            },
        )
        .unwrap();
    }
    let held = store.lock().unwrap();
    assert_eq!(
        evidence(&*held, session, "session.exit"),
        Some(json!("done"))
    );
    assert_eq!(
        evidence(&*held, session, "session.answer_delivered"),
        Some(json!(false)),
        "an exit newer than the answer is one the answer did not answer"
    );
}
