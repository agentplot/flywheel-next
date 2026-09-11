//! A session's reports, and the one path they take (65, 66, 67). Each writes
//! through a state repository of its own, which is the store this release
//! binds (92).

use chrono::Utc;
use crate::report::{write_report, Report, Reported};
use flywheel_atoms::Records;
use flywheel_scenario::Store;
use serde_json::json;

const SESSION: &str = "elaboration/a/1/self-closing/1";

/// A store over a state repository of its own (92, 160).
fn a_store() -> Store {
    let base = std::env::temp_dir().join(format!(
        "flywheel-report-{}-{:?}",
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

fn write(store: &mut Store, report: &Report) -> Reported {
    write_report(store, SESSION, "chuck", Utc::now(), report).expect("the report is written")
}

#[test]
fn exit_writes_one_thread_entry() {
    let mut store = a_store();

    // done with deliverables
    let out = write(
        &mut store,
        &Report::Exit {
            kind: "done".into(),
            deliverables: vec!["research.md".into()],
            question: None,
            text: None,
        },
    );
    assert!(matches!(out, Reported::Accepted(_)));

    // blocked on a question
    write(
        &mut store,
        &Report::Exit {
            kind: "blocked".into(),
            deliverables: vec![],
            question: Some("which provider?".into()),
            text: None,
        },
    );
    // stalled
    write(
        &mut store,
        &Report::Exit {
            kind: "stalled".into(),
            deliverables: vec![],
            question: None,
            text: None,
        },
    );
    // offering a finding, offering a chore — the other two of 65's five
    write(
        &mut store,
        &Report::Offer {
            kind: "finding".into(),
            document: "findings/1.md".into(),
        },
    );
    write(
        &mut store,
        &Report::Offer {
            kind: "chore".into(),
            document: "chores/1.md".into(),
        },
    );
    write(&mut store, &Report::Note { text: "halfway".into() });
    write(
        &mut store,
        &Report::Refuse {
            reason: "the place is not mine".into(),
        },
    );

    let thread = store.thread(SESSION).unwrap();
    assert_eq!(thread.len(), 7, "one report, one entry");
    assert_eq!(
        thread.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        vec!["exit", "exit", "exit", "offer", "offer", "note", "refusal"]
    );
    assert_eq!(thread[0].fields.get("exit"), Some(&json!("done")));
    assert_eq!(
        thread[0].fields.get("deliverables"),
        Some(&json!(["research.md"]))
    );
    assert_eq!(
        thread[1].fields.get("question"),
        Some(&json!("which provider?"))
    );
    assert_eq!(
        thread[3].fields.get("document"),
        Some(&json!("findings/1.md")),
        "an offer points at its document; the record never holds the text"
    );

    // The command writes nowhere but the thread: the machinery's state is
    // untouched by a report (66).
    assert!(store.objects.is_empty());
    assert!(store.responses.is_empty());
}

#[test]
fn exit_outside_the_five_is_refused() {
    let mut store = a_store();
    let out = write(
        &mut store,
        &Report::Exit {
            kind: "finished".into(),
            deliverables: vec![],
            question: None,
            text: Some("I think it is done".into()),
        },
    );
    let Reported::Refused { reason, .. } = &out else {
        panic!("a report that is none of the exits must be refused: {out:?}");
    };
    assert!(reason.contains("done, blocked, stalled"));

    // The refusal is recorded rather than dropped: one entry, read as invalid,
    // carrying the raw report (80).
    let thread = store.thread(SESSION).unwrap();
    assert_eq!(thread.len(), 1);
    assert_eq!(thread[0].fields.get("exit"), Some(&json!("invalid")));
    assert_eq!(thread[0].fields.get("raw"), Some(&json!("finished")));
    assert!(thread[0].fields.contains_key("refused"));

    // An offer of an unknown kind is refused the same way.
    let out = write(
        &mut store,
        &Report::Offer {
            kind: "opinion".into(),
            document: "d.md".into(),
        },
    );
    assert!(matches!(out, Reported::Refused { .. }));
    assert_eq!(store.thread(SESSION).unwrap().len(), 2);
}

#[test]
fn a_report_with_no_session_is_an_error() {
    let mut store = a_store();
    let err = write_report(
        &mut store,
        "",
        "chuck",
        Utc::now(),
        &Report::Note { text: "x".into() },
    )
    .expect_err("a report with no session names nothing to write on");
    assert!(err.to_string().contains("FLYWHEEL_SESSION"));
}
