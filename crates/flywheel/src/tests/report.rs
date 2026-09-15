//! A session's reports, and the one path they take (65, 66, 67). Each writes
//! through a state repository of its own, which is the store this release
//! binds (92).

use chrono::Utc;
use crate::report::{write_report, Report, Reported};
use flywheel_atoms::Records;
use flywheel_scenario::Store;
use flywheel_world_host::git::Repo;
use serde_json::json;
use std::path::{Path, PathBuf};

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

/// A place: a repository of its own with these documents committed at its
/// head, as a session's worktree holds what the session committed (62).
fn a_place(name: &str, documents: &[&str]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "flywheel-place-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the place's directory");
    let repo = Repo::at(&dir);
    repo.git(&["init", "--quiet", "--initial-branch=main", "."]).expect("git init");
    for document in documents {
        let path = dir.join(document);
        std::fs::create_dir_all(path.parent().expect("a document has a directory")).unwrap();
        std::fs::write(&path, format!("# {document}\n")).unwrap();
    }
    repo.git(&["add", "--all"]).expect("git add");
    repo.git(&["commit", "--quiet", "--allow-empty", "-m", "the session's documents"]).expect("git commit");
    dir
}

fn head_of(place: &Path) -> String {
    Repo::at(place).git(&["rev-parse", "HEAD"]).expect("the head").trim().to_string()
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
            scope: None,
            about: None,
            revision: None,
        },
    );
    write(
        &mut store,
        &Report::Offer {
            kind: "chore".into(),
            document: "chores/1.md".into(),
            scope: None,
            about: None,
            revision: None,
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
            scope: None,
            about: None,
            revision: None,
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

/// A chore offered off every bolt that names no repository the instance tracks
/// is refused on the session's thread with the names it could have given,
/// exits 1 and is never pending; one naming a tracked repository or the
/// blueprints is an offer (60, `sessions.yaml` commands.offer).
#[test]
fn an_offer_off_every_bolt_naming_no_tracked_repository_is_refused() {
    let mut store = a_store();
    let place = a_place("scope", &["chores/1.md", "chores/2.md"]);
    let tracked = || Ok(vec!["atlas".to_string()]);
    let offer = |store: &mut Store, document: &str, scope: Option<&str>| {
        crate::report::offer(store, tracked, &place, SESSION, "chuck", Utc::now(), "chore", document, scope, None)
            .expect("the offer is written")
    };

    for scope in [None, Some("bolt-line"), Some("switchboard")] {
        let out = offer(&mut store, "chores/1.md", scope);
        assert_eq!(crate::report::exit_code(&out), 1, "{scope:?} was not refused: {out:?}");
        let Reported::Refused { reason, .. } = &out else {
            unreachable!("exit 1 is a refusal");
        };
        assert!(reason.contains("atlas, blueprints"), "the refusal names what may be named: {reason}");
    }
    let thread = store.thread(SESSION).unwrap();
    assert_eq!(thread.len(), 3, "each refusal is one entry on the thread");
    assert!(thread.iter().all(|e| e.kind == "offer" && e.fields.contains_key("refused")));
    assert!(thread[2]
        .fields
        .get("refused")
        .and_then(|v| v.as_str())
        .is_some_and(|r| r.contains("`switchboard`")));
    assert!(
        flywheel_domain::offers::pending(&store, SESSION).unwrap().is_empty(),
        "a refused offer is never pending"
    );

    let atlas = offer(&mut store, "chores/1.md", Some("atlas"));
    assert_eq!(crate::report::exit_code(&atlas), 0);
    let blueprints = crate::report::offer(
        &mut store,
        || panic!("the blueprints need no manifest read"),
        &place,
        SESSION,
        "chuck",
        Utc::now(),
        "chore",
        "chores/2.md",
        Some("blueprints"),
        None,
    )
    .unwrap();
    assert_eq!(crate::report::exit_code(&blueprints), 0);
    assert_eq!(flywheel_domain::offers::pending(&store, SESSION).unwrap().len(), 2);
    assert_eq!(store.thread(SESSION).unwrap()[3].fields.get("scope"), Some(&json!("atlas")));
}

/// An offer of a kind that is none of finding, chore and signal is refused on
/// the session's thread naming the three, exits 1 and is never pending, as a
/// report outside the exits is; a signal is an offer, and its scope is not
/// read (65, 66, 80, `sessions.yaml` commands.offer).
#[test]
fn an_offer_of_an_unknown_kind_is_refused() {
    let mut store = a_store();
    let out = crate::report::offer(
        &mut store,
        || panic!("an unknown kind reads no manifest"),
        Path::new("/no/place/is/read/for/an/unknown/kind"),
        SESSION,
        "chuck",
        Utc::now(),
        "opinion",
        "notes/1.md",
        None,
        None,
    )
    .expect("the refusal is written");
    assert_eq!(crate::report::exit_code(&out), 1, "{out:?}");
    let Reported::Refused { reason, .. } = &out else {
        unreachable!("exit 1 is a refusal");
    };
    assert!(reason.contains("finding, chore, signal"), "the refusal names the offers: {reason}");
    let thread = store.thread(SESSION).unwrap();
    assert_eq!(thread.len(), 1, "the refusal is one entry on the thread");
    assert_eq!(thread[0].fields.get("raw"), Some(&json!("opinion")));
    assert!(thread[0].fields.contains_key("refused"));
    assert!(flywheel_domain::offers::pending(&store, SESSION).unwrap().is_empty(), "a refused offer is never pending");

    let signal = crate::report::offer(
        &mut store,
        || panic!("a signal's scope is not read"),
        &a_place("signal", &["notes/2.md"]),
        SESSION,
        "chuck",
        Utc::now(),
        "signal",
        "notes/2.md",
        Some("switchboard"),
        None,
    )
    .unwrap();
    assert_eq!(crate::report::exit_code(&signal), 0, "{signal:?}");
    assert_eq!(flywheel_domain::offers::pending(&store, SESSION).unwrap().len(), 1);
}

/// An offer names the place's head, which holds its document, on its entry
/// beside the kind, the document, what it is about and its scope; the pending
/// offer carries it (62, `sessions.yaml` commands.offer).
#[test]
fn an_offer_writes_the_places_head_on_its_entry() {
    let mut store = a_store();
    let document = "notes/provider-limits.md";
    let place = a_place("head", &[document]);
    let out = crate::report::offer(
        &mut store,
        || panic!("a finding's scope is not read"),
        &place,
        SESSION,
        "chuck",
        Utc::now(),
        "finding",
        document,
        None,
        Some("intent/atlas-provider-limits"),
    )
    .expect("the offer is written");
    assert_eq!(crate::report::exit_code(&out), 0, "{out:?}");
    let head = head_of(&place);
    let entry = store.thread(SESSION).unwrap().remove(0);
    for (field, value) in [
        ("offer", json!("finding")),
        ("document", json!(document)),
        ("about", json!("intent/atlas-provider-limits")),
        ("revision", json!(head)),
    ] {
        assert_eq!(entry.fields.get(field), Some(&value), "the entry's {field}");
    }
    let pending = flywheel_domain::offers::pending(&store, SESSION).unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].revision.as_deref(), Some(head.as_str()));
}

/// A document the place's head does not hold cannot be read anywhere else, so
/// its offer is refused on the thread naming the path, exits 1 and is never
/// pending — a file written and not committed, and an offer made outside any
/// place (62).
#[test]
fn an_offer_of_a_document_not_committed_is_refused() {
    let mut store = a_store();
    let place = a_place("uncommitted", &["notes/committed.md"]);
    std::fs::write(place.join("notes/draft.md"), "not yet committed\n").unwrap();
    let nowhere = std::env::temp_dir().join(format!("flywheel-no-place-{}", std::process::id()));
    std::fs::create_dir_all(&nowhere).unwrap();
    for (from, document) in [(place.as_path(), "notes/draft.md"), (nowhere.as_path(), "notes/elsewhere.md")] {
        let out = crate::report::offer(
            &mut store,
            || panic!("a refused document reads no manifest"),
            from,
            SESSION,
            "chuck",
            Utc::now(),
            "chore",
            document,
            Some("blueprints"),
            None,
        )
        .expect("the refusal is written");
        assert_eq!(crate::report::exit_code(&out), 1, "{document} was not refused: {out:?}");
        let Reported::Refused { reason, entry } = &out else {
            unreachable!("exit 1 is a refusal");
        };
        assert!(reason.contains(document), "the refusal names the path: {reason}");
        assert_eq!(entry.fields.get("document"), Some(&json!(document)));
    }
    let thread = store.thread(SESSION).unwrap();
    assert_eq!(thread.len(), 2, "each refusal is one entry on the thread");
    assert_eq!(thread[0].fields.get("revision"), Some(&json!(head_of(&place))), "the head it looked at");
    assert!(flywheel_domain::offers::pending(&store, SESSION).unwrap().is_empty(), "a refused offer is never pending");
}

/// What an offer is about is written on its entry beside the kind, the document
/// and the scope, and nothing is derived from it: the unit a chore offer makes
/// is the same with or without it, since where the fix lands is the scope's
/// (58, 60, 62, `sessions.yaml` commands.offer).
#[test]
fn an_offers_about_is_written_on_its_entry() {
    use flywheel_atoms::testing::{FakeStore, FakeWorld};
    use flywheel_domain::{commands, offers};

    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let at = Utc::now();
    let session = "curation/main/1";
    let document = "flywheel/curation/chores/agents-md.md";
    let place = a_place("about", &[document]);
    let offered = |about: Option<&str>| {
        let mut store = FakeStore::default();
        let mut world = FakeWorld::new().tracking("atlas");
        commands::put_new(&mut store, &defs, "curation/main", "curation", None, Default::default(), at).expect("the curation");
        let tracked = || Ok(vec!["atlas".to_string()]);
        let out = crate::report::offer(&mut store, tracked, &place, session, "chuck", at, "chore", document, Some("atlas"), about)
            .expect("the offer is written");
        assert_eq!(crate::report::exit_code(&out), 0, "{out:?}");
        let made = offers::record(&mut store, &mut world, &defs, session, "curation/main", at).expect("the offer is recorded");
        assert_eq!(made, vec!["unit/atlas/chore-1".to_string()]);
        let entry = store.thread(session).expect("the thread").remove(0);
        let unit = store.get(&made[0]).expect("a read").expect("the chore unit");
        (entry, unit)
    };

    let (entry, with) = offered(Some("intent/atlas-provider-limits"));
    for (field, value) in [
        ("offer", json!("chore")),
        ("document", json!(document)),
        ("scope", json!("atlas")),
        ("about", json!("intent/atlas-provider-limits")),
    ] {
        assert_eq!(entry.fields.get(field), Some(&value), "the entry's {field}");
    }
    let (plain, without) = offered(None);
    assert!(!plain.fields.contains_key("about"), "an offer about nothing named writes none: {plain:?}");
    assert_eq!(
        (&with.id, &with.parent, &with.record, &with.config),
        (&without.id, &without.parent, &without.record, &without.config),
        "the unit is the same with or without what the offer is about"
    );
}
