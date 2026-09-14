//! The host loop: what a tick does, what a host takes, what it records, and
//! what it shows (D7, D8, D12, 79-82, 141-150a).
//!
//! Every test runs a real host over a real local state repository. No network,
//! no built repository, no agent: the workspace is recorded and the operator is
//! the session (93a, 93b).

use chrono::{DateTime, Duration, TimeZone, Utc};
use flywheel::host::{Bindings, Host};
use flywheel_atoms::{Records, Scope, StateStore, ThreadEntry};
use flywheel_domain::derived::Declaration;
use flywheel_engine::Object;
use flywheel_store_git::store::sandbox;
use serde_json::json;
use std::collections::BTreeMap;

fn at(minute: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap() + Duration::minutes(minute)
}

fn dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "flywheel-host-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn declaration(repositories: &[&str]) -> Declaration {
    Declaration {
        repositories: repositories.iter().map(|r| r.to_string()).collect(),
        types: vec![],
        kinds: vec!["all".into()],
    }
}

fn host(name: &str, repositories: &[&str]) -> Host {
    let base = dir(name);
    let now = at(0);
    let git = sandbox(&base, "mac-mini", now).unwrap();
    Host::over(
        "mac-mini",
        "willdan",
        flywheel_domain::set::load().unwrap(),
        git,
        Bindings {
            world: "host".into(),
            workspace: "recorded".into(),
            sessions: "operator".into(),
        },
        declaration(repositories),
        now,
    )
}

/// Put a described object in the store: what a scenario's `given` does, and
/// what the world outside these tests would have written. One commit for the
/// whole of it, because a described state is one write (94, D15).
fn seed(host: &mut Host, id: &str, machine: &str, states: &[(&str, &str)], record: &[(&str, serde_json::Value)]) {
    let mut object = Object {
        id: id.to_string(),
        machine: machine.to_string(),
        parent: None,
        config: states
            .iter()
            .map(|(r, s)| (r.to_string(), s.to_string()))
            .collect(),
        entered_at: states
            .iter()
            .map(|(r, _)| (r.to_string(), host.now()))
            .collect(),
        record: record
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    };
    if object.config.is_empty() {
        flywheel_engine::initialise(&host.defs, &mut object, host.now());
    }
    host.store.git.seed_objects(std::slice::from_ref(&object)).unwrap();
}

// ---------------------------------------------------------------- 6.1 the tick

#[test]
fn tick_fetches_first() {
    let mut host = host("fetch", &["atlas"]);
    seed(
        &mut host,
        "bolt/atlas/plan-rows",
        "bolt",
        &[("life", "open")],
        &[("repository", json!("atlas"))],
    );
    host.sweep().unwrap();
    let seen = host.store.since();
    assert_eq!(
        seen.first().map(String::as_str),
        Some("fetch"),
        "a tick read before it fetched: {seen:?}"
    );
    // And nothing decided before the fetch: the first read comes after it.
    let first_read = seen.iter().position(|s| s == "list" || s == "read" || s == "get");
    assert!(first_read.unwrap_or(0) > 0);
}

#[test]
fn sweep_fires_older_guards() {
    let mut host = host("sweep", &["atlas"]);
    // Another host, last seen ten minutes ago: past the five-minute window its
    // life is stale, and only a guard reading the clock says so (130, D7).
    seed(
        &mut host,
        "host/mini-2",
        "host",
        &[],
        &[
            ("last_seen", json!((at(0) - Duration::minutes(10)).to_rfc3339())),
            ("bound", json!(1)),
            ("intermittent", json!(false)),
        ],
    );
    seed(
        &mut host,
        "bolt/atlas/plan-rows",
        "bolt",
        &[("life", "open")],
        &[("repository", json!("atlas"))],
    );

    // A notify-tick on another object leaves it where it was: nothing under
    // `bolt/atlas/plan-rows` reads that host's clock.
    host.tick(&Scope::Under("bolt/atlas/plan-rows".into())).unwrap();
    let held = host.store.get("host/mini-2").unwrap().unwrap();
    assert_eq!(held.config.get("life").map(String::as_str), Some("alive"));

    // The sweep covers every scope, so the `older:` guard fires.
    host.sweep().unwrap();
    let held = host.store.get("host/mini-2").unwrap().unwrap();
    assert_eq!(
        held.config.get("life").map(String::as_str),
        Some("stale"),
        "the sweep did not fire the older guard"
    );
}

// ------------------------------------------------- 6.2 declaration and leases

#[test]
fn lease_only_within_declaration() {
    let mut host = host("declaration", &["atlas"]);
    seed(
        &mut host,
        "bolt/atlas/plan-rows",
        "bolt",
        &[("life", "open")],
        &[("repository", json!("atlas"))],
    );
    seed(
        &mut host,
        "bolt/beta/other",
        "bolt",
        &[("life", "open")],
        &[("repository", json!("beta"))],
    );
    host.sweep().unwrap();

    assert_eq!(
        host.store
            .leases("bolt/atlas/plan-rows")
            .unwrap()
            .map(|l| l.holder),
        Some("mac-mini".to_string())
    );
    // Nothing outside the declaration is held; the lease machine says so
    // rather than waiting silently, and its line is under attention (149).
    let outside = host.store.leases("bolt/beta/other").unwrap();
    assert_eq!(
        outside.as_ref().map(|l| l.holder.as_str()).unwrap_or(""),
        "",
        "the host took a lease outside its declaration (149)"
    );
    assert_eq!(
        outside.as_ref().map(|l| l.state.as_str()),
        Some("uncovered")
    );
    assert!(host
        .attention()
        .unwrap()
        .iter()
        .any(|a| a == "uncovered: lease/bolt/beta/other"));

    // Its declaration and its heartbeat are on the record, so another host can
    // read what this one takes (147, 149, 163).
    let me = host.store.get("host/mac-mini").unwrap().unwrap();
    assert_eq!(
        me.record.get("declares").and_then(|d| d.get("repositories")),
        Some(&json!(["atlas"]))
    );
    assert!(host.store.hosts().unwrap().iter().any(|h| h.host == "mac-mini"));
}

// ------------------------------------------------------- 6.4 the away window

/// A host away: its life is `away`, its leases stand, and no attention line is
/// raised for it (150a).
#[test]
fn away_raises_no_attention() {
    let mut host = host("away", &["atlas"]);
    seed(
        &mut host,
        "bolt/atlas/plan-rows",
        "bolt",
        &[("life", "open")],
        &[("repository", json!("atlas"))],
    );
    host.intermittent = true;
    host.sweep().unwrap();
    let held_before = host
        .store
        .leases("bolt/atlas/plan-rows")
        .unwrap()
        .map(|l| l.holder);

    // The laptop is shut: no heartbeat, and the clock runs on past the stale
    // and the gone windows.
    host.go_away(at(0));
    host.set_now(at(45));
    host.sweep().unwrap();
    host.sweep().unwrap();

    let me = host.store.get("host/mac-mini").unwrap().unwrap();
    assert_eq!(
        me.config.get("life").map(String::as_str),
        Some("away"),
        "an intermittent host past its stale window is away, not gone (150a)"
    );
    assert_eq!(
        host.store
            .leases("bolt/atlas/plan-rows")
            .unwrap()
            .map(|l| l.holder),
        held_before,
        "an away host's leases stand (150a)"
    );
    let attention = host.attention().unwrap();
    assert!(
        !attention.iter().any(|a| a.starts_with("host-gone")),
        "an away host raised an attention line: {attention:?}"
    );

    // Back, with nothing to answer.
    host.set_now(at(46));
    host.come_back().unwrap();
    host.sweep().unwrap();
    host.sweep().unwrap();
    let me = host.store.get("host/mac-mini").unwrap().unwrap();
    assert_eq!(me.config.get("life").map(String::as_str), Some("alive"));
}

#[test]
fn away_with_work_waiting_raises_takeover() {
    let mut host = host("waiting", &["atlas"]);
    seed(
        &mut host,
        "bolt/atlas/plan-rows",
        "bolt",
        &[("life", "open")],
        &[("repository", json!("atlas"))],
    );
    // Work the operator approved, waiting on the host that holds it: it
    // depends on a unit of the same bolt that has not merged, which is what
    // holds an approved unit in `waiting` (31, `unit.yaml` approved).
    seed(
        &mut host,
        "unit/atlas/first",
        "unit",
        &[("life", "approved")],
        &[("repository", json!("atlas")), ("type", json!("default"))],
    );
    seed(
        &mut host,
        "unit/atlas/u",
        "unit",
        &[("life", "approved")],
        &[
            ("repository", json!("atlas")),
            ("type", json!("default")),
            ("depends_on", json!(["unit/atlas/first"])),
        ],
    );
    host.intermittent = true;
    host.sweep().unwrap();

    host.go_away(at(0));
    host.set_now(at(45));
    host.sweep().unwrap();
    host.sweep().unwrap();
    host.sweep().unwrap();

    let me = host.store.get("host/mac-mini").unwrap().unwrap();
    assert_eq!(
        me.config.get("life").map(String::as_str),
        Some("gone"),
        "work waiting on an away host raises the takeover decision (150a)"
    );
    let attention = host.attention().unwrap();
    assert!(
        attention.iter().any(|a| a.starts_with("host-gone")),
        "no takeover was offered though work waited: {attention:?}"
    );
}

// ----------------------------------------------------------- 6.8 the bindings

#[test]
fn bindings_named_in_run_record() {
    let mut host = host("bindings", &["atlas"]);
    host.record_bindings();
    host.sweep().unwrap();
    let record = host.store.git.run_record().unwrap();
    let binding = record
        .iter()
        .find(|e| e.kind == "binding")
        .expect("the run record names the bindings this host loaded (139)");
    let named: BTreeMap<&str, &str> = binding
        .fields
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();
    assert_eq!(named.get("world"), Some(&"host"));
    assert_eq!(named.get("workspace"), Some(&"recorded"));
    assert_eq!(named.get("sessions"), Some(&"operator"));
    assert!(named.contains_key("definitions"));
}

#[test]
fn a_binding_this_release_lacks_is_refused() {
    // A host never branches on a binding it cannot load: it says so and stops
    // (D8, 299).
    let mut manifest = flywheel_world_host::Manifest {
        instance: "willdan".into(),
        ..Default::default()
    };
    manifest.hosts.insert(
        "local".into(),
        flywheel_world_host::manifest::Host {
            root: "/tmp/flywheel".into(),
            workspace: "host".into(),
            sessions: "operator".into(),
            covers: vec![],
            ..Default::default()
        },
    );
    let refused = Bindings::read(&manifest, "local").unwrap_err().to_string();
    assert!(refused.contains("workspace"), "{refused}");
    assert!(refused.contains("phase 2"), "{refused}");
}

// ----------------------------------------------------------- 6.11-6.14 the run record

#[test]
fn run_record_carries_reason_and_evidence() {
    let mut host = host("reason", &["atlas"]);
    seed(
        &mut host,
        "host/mini-2",
        "host",
        &[],
        &[
            ("last_seen", json!((at(0) - Duration::minutes(10)).to_rfc3339())),
            ("bound", json!(1)),
        ],
    );
    host.sweep().unwrap();
    let record = host.store.git.run_record().unwrap();
    let write = record
        .iter()
        .find(|e| e.kind == "write" && e.object == "host/mini-2")
        .expect("every write is recorded with its reason (79)");
    assert!(
        write.reason.contains("alive") && write.reason.contains("stale"),
        "the reason does not say what moved: {:?}",
        write.reason
    );
    assert!(
        write.evidence.iter().any(|(n, _)| n == "host.last_seen"),
        "the evidence the guard read is not on the entry: {:?}",
        write.evidence
    );
}

#[test]
fn expected_beside_delivered() {
    let mut host = host("delivered", &["atlas"]);
    // A session asked for three things that delivered two.
    flywheel_sessions_operator::set(
        &mut host.store.git,
        "session/unit/atlas/u/main",
        &[
            ("runner", json!("operator")),
            ("place", json!("unit/atlas/u")),
            ("started_at", json!(at(0).to_rfc3339())),
            ("deliverables", json!(["book-chapter", "claim", "context-map"])),
        ],
    )
    .unwrap();
    host.store
        .git
        .append(
            "session/unit/atlas/u/main",
            &ThreadEntry {
                at: at(1),
                kind: "exit".into(),
                by: Some("operator".into()),
                fields: [
                    ("exit".to_string(), json!("done")),
                    ("deliverables".to_string(), json!(["book-chapter", "claim"])),
                ]
                .into_iter()
                .collect(),
            },
        )
        .unwrap();
    host.set_now(at(2));
    host.sweep().unwrap();

    let record = host.store.git.run_record().unwrap();
    let entry = record
        .iter()
        .find(|e| e.kind == "session")
        .expect("what a session delivered is recorded (80)");
    let fields: BTreeMap<&str, &str> = entry
        .fields
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();
    // The difference first (80).
    assert_eq!(entry.fields.first().map(|(n, _)| n.as_str()), Some("entry"));
    assert_eq!(fields.get("missing"), Some(&"context-map"));
    assert_eq!(
        fields.get("expected"),
        Some(&"book-chapter, claim, context-map")
    );
    assert_eq!(fields.get("delivered"), Some(&"book-chapter, claim"));
}

#[test]
fn machinery_problem_is_not_work() {
    let mut host = host("problem", &["atlas"]);
    let before = host.store.list(&Scope::All).unwrap().objects.len();
    host.report_problem("host/mac-mini", "the git host refused the push");
    host.sweep().unwrap();

    let record = host.store.git.run_record().unwrap();
    let problem = record
        .iter()
        .find(|e| e.is_problem())
        .expect("a problem with the machinery is reported (81)");
    assert!(problem.reason.contains("refused the push"));

    // And nothing was made of it: no intent, no unit, no work item (81).
    let after = host.store.list(&Scope::All).unwrap().objects;
    assert!(
        !after
            .iter()
            .any(|o| matches!(o.machine.as_str(), "intent" | "unit" | "work-item" | "capture")),
        "a problem with the machinery became work"
    );
    assert!(after.len() >= before);
}

#[test]
fn refusal_reaches_attention() {
    let mut host = host("refusal", &["atlas"]);
    flywheel_sessions_operator::set(
        &mut host.store.git,
        "session/unit/atlas/u/main",
        &[
            ("runner", json!("operator")),
            ("place", json!("unit/atlas/u")),
            ("started_at", json!(at(0).to_rfc3339())),
        ],
    )
    .unwrap();
    host.store
        .git
        .append(
            "session/unit/atlas/u/main",
            &ThreadEntry {
                at: at(1),
                kind: "refusal".into(),
                by: Some("session/unit/atlas/u/main".into()),
                fields: [
                    ("operation".to_string(), json!("land_line")),
                    ("reason".to_string(), json!("a session may not land a line")),
                ]
                .into_iter()
                .collect(),
            },
        )
        .unwrap();
    host.set_now(at(2));
    host.sweep().unwrap();

    let record = host.store.git.run_record().unwrap();
    let refusal = record
        .iter()
        .find(|e| e.kind == "refusal")
        .expect("every refusal is recorded (4, 79)");
    let fields: BTreeMap<&str, &str> = refusal
        .fields
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();
    assert_eq!(fields.get("identity"), Some(&"session/unit/atlas/u/main"));
    assert_eq!(fields.get("operation"), Some(&"land_line"));
    assert_eq!(refusal.object, "session/unit/atlas/u/main");
    assert!(
        host.attention()
            .unwrap()
            .iter()
            .any(|a| a == "refusal: session/unit/atlas/u/main"),
        "the refusal never reached attention (81, 82)"
    );
}

// ------------------------------------------------------- 6.15-6.19 the status view

#[test]
fn status_view_groups_every_object() {
    let mut host = host("status", &["atlas"]);
    seed(
        &mut host,
        "bolt/atlas/plan-rows",
        "bolt",
        &[("life", "open")],
        &[("repository", json!("atlas"))],
    );
    seed(
        &mut host,
        "unit/atlas/u",
        "unit",
        &[("life", "approved")],
        &[("repository", json!("atlas")), ("type", json!("default"))],
    );
    seed(
        &mut host,
        "unit/atlas/done",
        "unit",
        &[("life", "landed")],
        &[("repository", json!("atlas")), ("type", json!("default"))],
    );
    host.sweep().unwrap();

    let status = host.status().unwrap();
    let grouped: Vec<&str> = status.rows.iter().map(|r| r.object.as_str()).collect();
    for object in ["bolt/atlas/plan-rows", "unit/atlas/u", "unit/atlas/done"] {
        assert!(grouped.contains(&object), "{object} is on no group: {grouped:?}");
    }
    // Every group is one of the four of 141, and every object is in exactly one.
    for row in &status.rows {
        assert!(
            flywheel_domain::status::GROUPS.contains(&row.group.as_str()),
            "{} is in the group `{}`",
            row.object,
            row.group
        );
    }
    assert_eq!(status.group("done"), vec!["unit/atlas/done"]);

    // Its holder, its runner and that host's liveness are shown (143, 146).
    let held = status
        .rows
        .iter()
        .find(|r| r.object == "bolt/atlas/plan-rows")
        .unwrap();
    assert_eq!(held.holder.as_deref(), Some("mac-mini"));
    assert_eq!(held.liveness.as_deref(), Some("alive"));

    // One place for the instance: one page, with the as-of point on it (145).
    let view = flywheel_domain::status::render(&status);
    assert!(view.body.contains("as of commit"));
    for object in ["bolt/atlas/plan-rows", "unit/atlas/u", "unit/atlas/done"] {
        assert!(view.body.contains(object), "{object} is not on the page");
    }
}

#[test]
fn discussion_stays_with_the_object() {
    let mut host = host("discussion", &["atlas"]);
    seed(
        &mut host,
        "unit/atlas/u",
        "unit",
        &[("life", "approved")],
        &[("repository", json!("atlas")), ("type", json!("default"))],
    );
    for (kind, text) in [
        ("question", "which rows does this cover?"),
        ("answer", "the ones the claim names"),
        ("note", "the claim moved on Tuesday"),
    ] {
        host.store
            .git
            .append(
                "unit/atlas/u",
                &ThreadEntry {
                    at: at(1),
                    kind: kind.into(),
                    by: Some("operator".into()),
                    fields: [("text".to_string(), json!(text))].into_iter().collect(),
                },
            )
            .unwrap();
    }
    host.sweep().unwrap();

    // The session that said them is gone; they are still under the object.
    flywheel_sessions_operator::end(&mut host.store.git, "unit/atlas/u/main", at(2)).unwrap();
    let status = host.status().unwrap();
    let row = status
        .rows
        .iter()
        .find(|r| r.object == "unit/atlas/u")
        .unwrap();
    assert_eq!(
        row.discussion,
        vec![
            ("question".to_string(), "which rows does this cover?".to_string()),
            ("answer".to_string(), "the ones the claim names".to_string()),
            ("note".to_string(), "the claim moved on Tuesday".to_string()),
        ]
    );
}

#[test]
fn drift_rewritten_and_reported() {
    let mut host = host("drift", &["atlas"]);
    seed(
        &mut host,
        "unit/atlas/u",
        "unit",
        &[("life", "approved")],
        &[("repository", json!("atlas")), ("type", json!("default"))],
    );
    host.sweep().unwrap();
    let written = host.store.git.committed_status().unwrap().unwrap();
    assert!(written.contains("unit/atlas/u"));

    // Someone rewrote the projection by hand. It is not the truth, so it is
    // rewritten from its source on the next tick and reported with both values
    // (77, 142).
    host.store.git.commit_status("<html>not what was read</html>").unwrap();
    host.set_now(at(1));
    host.sweep().unwrap();

    let again = host.store.git.committed_status().unwrap().unwrap();
    assert!(again.contains("unit/atlas/u"), "the drift was not rewritten");
    let record = host.store.git.run_record().unwrap();
    let drift = record
        .iter()
        .find(|e| e.kind == "drift")
        .expect("drift is reported (77, 142)");
    let fields: BTreeMap<&str, &str> = drift
        .fields
        .iter()
        .map(|(n, v)| (n.as_str(), v.as_str()))
        .collect();
    assert!(fields.contains_key("was") && fields.contains_key("now"));
    assert_ne!(fields.get("was"), fields.get("now"));
}

// ------------------------------------------------------------- 6.5 the takeover

/// Two hosts over one state repository, told apart by id alone (232).
fn pair(name: &str) -> (Host, Host, std::path::PathBuf) {
    let base = dir(name);
    let now = at(0);
    let defs = flywheel_domain::set::load().unwrap();
    let bindings = Bindings {
        world: "host".into(),
        workspace: "recorded".into(),
        sessions: "operator".into(),
    };
    let a = Host::over(
        "mac-mini",
        "willdan",
        defs.clone(),
        sandbox(&base, "mac-mini", now).unwrap(),
        bindings.clone(),
        declaration(&["atlas"]),
        now,
    );
    let b = Host::over(
        "studio",
        "willdan",
        defs,
        sandbox(&base, "studio", now).unwrap(),
        bindings,
        declaration(&["atlas"]),
        now,
    );
    (a, b, base)
}

#[test]
fn takeover_starts_attempt_two() {
    let (mut a, mut b, _base) = pair("takeover");
    // A host that is always on: past its stale window it is gone and offers
    // takeover, rather than away with its leases standing (150, 150a).
    a.intermittent = false;
    // A laptop holds an item and runs the operator's session on it.
    seed(
        &mut a,
        "bolt/atlas/plan-rows",
        "bolt",
        &[("life", "open")],
        &[("repository", json!("atlas"))],
    );
    let session = "unit/atlas/u/main";
    flywheel_sessions_operator::start(
        &mut a.store.git,
        "mac-mini",
        at(0),
        &flywheel_atoms::WorkOrder {
            session: format!("{session}/1"),
            kind: "unit".into(),
            place: "unit/atlas/u".into(),
            body: "the first attempt".into(),
        },
    )
    .unwrap();
    a.store
        .git
        .lease(&flywheel_atoms::LeaseOp::Take {
            object: "bolt/atlas/plan-rows".into(),
            holder: "mac-mini".into(),
        })
        .unwrap();
    a.sweep().unwrap();

    // The laptop is shut for an hour. The other host sees it gone — past the
    // stale and gone windows, and well inside the long bound that would release
    // it without anyone answering (150).
    a.go_away(at(0));
    b.set_now(at(60));
    b.sweep().unwrap();
    let gone = b.store.get("host/mac-mini").unwrap().unwrap();
    assert_eq!(gone.config.get("life").map(String::as_str), Some("gone"));
    assert!(b
        .attention()
        .unwrap()
        .iter()
        .any(|a| a.starts_with("host-gone")));

    // The operator answers takeover. The leases the gone host held expire and
    // the sessions it was running are closed (150).
    let defs = b.defs.clone();
    flywheel::console::dictate(&mut b.store, &defs, "host/mac-mini", "takeover", "operator").unwrap();
    b.sweep().unwrap();

    // The gone host's lease is expired and the taking host holds it now (150).
    assert_eq!(
        b.store
            .leases("bolt/atlas/plan-rows")
            .unwrap()
            .map(|l| l.holder)
            .unwrap_or_default(),
        "studio",
        "the taking host does not hold what the gone host held"
    );

    // The taking host starts a fresh attempt, not the same session again.
    let fresh = flywheel_sessions_operator::next_attempt(&b.store.git, session);
    assert_eq!(fresh, format!("{session}/2"));
    let taking_at = b.now();
    flywheel_sessions_operator::start(
        &mut b.store.git,
        "studio",
        taking_at,
        &flywheel_atoms::WorkOrder {
            session: fresh.clone(),
            kind: "unit".into(),
            place: "unit/atlas/u".into(),
            body: "the second attempt".into(),
        },
    )
    .unwrap();
    assert!(flywheel_sessions_operator::running(&b.store.git, &fresh));
    assert!(!flywheel_sessions_operator::running(
        &b.store.git,
        &format!("{session}/1")
    ));

    // The laptop comes back, reads that it lost, ends its own session and
    // reports it. It starts nothing again.
    a.set_now(at(61));
    a.come_back().unwrap();
    a.sweep().unwrap();
    let record = a.store.git.run_record().unwrap();
    let reported = record
        .iter()
        .find(|e| e.object == format!("{session}/1") && e.reason.contains("taken over"))
        .expect("the returning host reports that its session was taken over (150)");
    assert_eq!(
        reported
            .fields
            .iter()
            .find(|(n, _)| n == "taken_over_by")
            .map(|(_, v)| v.as_str()),
        Some("studio")
    );
    assert!(!flywheel_sessions_operator::running(
        &a.store.git,
        &format!("{session}/1")
    ));
}

// ----------------------------------------------- 8.10 the three local causes

/// A local cause notifies in-process at once and does not wait for the poll
/// (130, D6). The three real ones are a page response, a chat message and a
/// session's report; each writes through the store, and the store's own notice
/// names what moved.
#[test]
fn local_causes_tick_at_once() {
    for cause in ["page", "chat", "session"] {
        let mut host = host(&format!("cause-{cause}"), &["atlas"]);
        seed(
            &mut host,
            "bolt/atlas/plan-rows",
            "bolt",
            &[("life", "open"), ("life.open.close", "offered")],
            &[("repository", json!("atlas"))],
        );
        // The sink the chat message arrives at, and the presenter lease that
        // lets this host read it (148).
        let defs = host.defs.clone();
        flywheel_domain::sinks::ensure(
            &mut host.store,
            &defs,
            &flywheel_domain::sinks::Spec::chat("chat-chuck", "chuck", "#willdan"),
        )
        .unwrap();

        // One sweep, so the clock for the next one starts now. Everything after
        // this is what a notice does on its own (D7).
        host.sweep().unwrap();
        let swept = host.last_sweep.expect("the sweep ran");
        let number = flywheel_domain::commands::rail(&mut host.store, &defs)
            .unwrap()
            .iter()
            .find(|d| d.object == "bolt/atlas/plan-rows")
            .and_then(|d| d.number)
            .expect("the decision was numbered");
        host.tick(&Scope::All).unwrap();

        // Nothing has moved that this host has not already read.
        let quiet = host.notified().unwrap();
        assert!(
            !quiet.iter().any(|o| o.starts_with("response/")),
            "{cause}: a response stood before the cause: {quiet:?}"
        );

        // The cause, raised locally on this host.
        let named = match cause {
            // A control on the page, through the same tool the chat's grammar
            // calls (193).
            "page" => {
                let call = flywheel_surface::catalogue::Call::new("answer", "chuck", "page")
                    .arg("decision", json!(number))
                    .arg("answer", json!("yes"));
                let called = host
                    .store
                    .with_world(|store, world| {
                        flywheel_surface::catalogue::call(store, world, &defs, &call)
                    })
                    .unwrap();
                format!("response/{}", called.id)
            }
            // A numbered reply in the chat (194).
            "chat" => {
                let mut chat = flywheel_surface::chat::Chat::new(
                    "sink/chat-chuck",
                    &host.name,
                    "http://studio.tailnet.ts.net/willdan",
                    flywheel_surface::chat::Recorded::new(),
                );
                let message = flywheel_surface::chat::Message::new(
                    "1801",
                    "chuck",
                    &format!("yes {number}"),
                );
                let heard = host
                    .store
                    .with_world(|store, world| chat.receive(store, world, &defs, &message))
                    .unwrap();
                match heard {
                    flywheel_surface::chat::Heard::Answered(given) => {
                        format!("response/{}", given[0].id)
                    }
                    other => panic!("the reply was heard as {other:?}"),
                }
            }
            // A session reporting through the command the machinery provides
            // (67, 93b).
            _ => {
                let session = "bolt/atlas/plan-rows/session/1";
                let at = host.now();
                flywheel::report::write_report(
                    &mut host.store,
                    session,
                    "chuck",
                    at,
                    &flywheel::report::Report::Exit {
                        kind: "done".into(),
                        deliverables: vec!["the rows".into()],
                        question: None,
                        text: None,
                    },
                )
                .unwrap();
                session.to_string()
            }
        };

        // The store names it at once: no poll interval passed, and no sweep is
        // due (130, D6, D7).
        assert_eq!(host.now(), swept, "{cause}: the clock moved");
        assert!(
            host.now() - swept < host.sweep_interval(),
            "{cause}: the sweep was due, so this proves nothing"
        );
        let notice = host.notified().unwrap();
        assert!(
            notice.contains(&named),
            "{cause}: the local cause `{named}` was not notified: {notice:?}"
        );

        // And the notify-tick takes it, without the sweep and without the poll:
        // the pass integrates and re-reads past the cause's own commit. With no
        // notice there would have been no tick at all, since no sweep was due.
        let stood_at = host.last_point.mark.clone();
        host.once().unwrap();
        assert_ne!(
            host.last_point.mark, stood_at,
            "{cause}: the pass read nothing; the notice drove no tick"
        );
        assert_eq!(host.now(), swept, "{cause}: the pass moved the clock");
        assert_eq!(
            host.last_sweep, Some(swept),
            "{cause}: the pass ran a sweep rather than the notify-tick"
        );
    }
}

// -------------------------------------------- 8.11 the host delivers to a sink

/// A tick with a standing decision and a sink whose mark is behind delivers
/// once and advances the mark; the next tick delivers nothing, because the mark
/// is no longer behind (148, 216, 14, `surfaces.yaml` effects.deliver_rail).
#[test]
fn host_delivers_to_its_sink() {
    let mut host = host("delivers", &["atlas"]);
    seed(
        &mut host,
        "bolt/atlas/plan-rows",
        "bolt",
        &[("life", "open"), ("life.open.close", "offered")],
        &[("repository", json!("atlas"))],
    );
    let defs = host.defs.clone();
    let sink = flywheel_domain::sinks::ensure(
        &mut host.store,
        &defs,
        &flywheel_domain::sinks::Spec::chat("chat-chuck", "chuck", "#willdan"),
    )
    .unwrap();
    // The channel this host's sink delivers through, loaded by name the way the
    // workspace and the sessions are (D8, D9).
    host.sinks.bind(
        &sink.id,
        Box::new(flywheel_surface::chat::Recorded::new()),
    );

    // The decision stands: the register holds it, as the tick before this one
    // left it. Every guard in one tick reads the state taken before the region
    // loop, so a decision numbered by this tick's own derive is one the sink
    // sees on the next (model.md, the region rule).
    flywheel_domain::commands::rail(&mut host.store, &defs).unwrap();
    assert_eq!(sink.delivered_at, None, "a sink that never delivered has a mark");

    host.sweep().unwrap();

    let after = flywheel_domain::sinks::read(&host.store, &sink.id)
        .unwrap()
        .unwrap();
    let mark = after.delivered_at.expect("the mark advanced with the delivery");
    let delivery = after.delivery.clone().expect("the delivery's own id");
    assert!(delivery.starts_with("#willdan-"), "{delivery}");

    // A sweep settles, so the sink went through `delivering` — where its entry
    // effect performed the delivery — and back to idle, with its mark no longer
    // behind. The sweep after it delivers nothing (127, 78).
    assert_eq!(
        host.store
            .get(&sink.id)
            .unwrap()
            .unwrap()
            .config
            .get("delivery")
            .map(String::as_str),
        Some("idle"),
        "the sink stayed in delivering"
    );
    host.sweep().unwrap();
    let again = flywheel_domain::sinks::read(&host.store, &sink.id)
        .unwrap()
        .unwrap();
    assert_eq!(again.delivered_at, Some(mark), "the mark moved with no delivery");
    assert_eq!(again.delivery, Some(delivery), "it delivered a second time");

    // A second decision comes to stand, and the sink is behind again.
    host.set_now(host.now() + Duration::minutes(2));
    seed(
        &mut host,
        "bolt/atlas/drop-the-tail",
        "bolt",
        &[("life", "open"), ("life.open.close", "offered")],
        &[("repository", json!("atlas"))],
    );
    flywheel_domain::commands::rail(&mut host.store, &defs).unwrap();
    host.sweep().unwrap();
    let third = flywheel_domain::sinks::read(&host.store, &sink.id)
        .unwrap()
        .unwrap();
    assert!(
        third.delivered_at > Some(mark),
        "a decision stood and the sink did not deliver"
    );
}

/// A host that loaded no channel for a sink presents nothing there, whatever
/// lease it holds: the effect's proof stays absent, so the host that does
/// present it performs the delivery (148, 127).
#[test]
fn a_host_with_no_channel_delivers_nothing() {
    let mut host = host("no-channel", &["atlas"]);
    seed(
        &mut host,
        "bolt/atlas/plan-rows",
        "bolt",
        &[("life", "open"), ("life.open.close", "offered")],
        &[("repository", json!("atlas"))],
    );
    let defs = host.defs.clone();
    let sink = flywheel_domain::sinks::ensure(
        &mut host.store,
        &defs,
        &flywheel_domain::sinks::Spec::chat("chat-chuck", "chuck", "#willdan"),
    )
    .unwrap();

    flywheel_domain::commands::rail(&mut host.store, &defs).unwrap();
    host.sweep().unwrap();
    let after = flywheel_domain::sinks::read(&host.store, &sink.id)
        .unwrap()
        .unwrap();
    assert_eq!(
        after.delivered_at, None,
        "a host with no channel bound delivered anyway"
    );
    assert_eq!(after.delivery, None);
}

// ------------------------------------------- 19.6 a lease is not a decision

/// A lease a host holds is the machinery's own bookkeeping: which host has the
/// object, and nothing for the operator to answer. It takes no number, and is
/// read where 141 already puts the holder, on the status view (79, 141, 310).
///
/// An object no host's declaration covers is the other thing. 149 makes it a
/// decision under attention and never a silent wait, `engine/lease.yaml`
/// declares that decision on the `uncovered` state, and it is answerable — with
/// "seen", which is the operator saying they know the work is stopped. So it
/// takes a number like any other decision.
#[test]
fn a_lease_is_not_a_decision() {
    let mut host = host("lease-not-a-decision", &["atlas"]);
    // Three the host covers and takes, and two it does not: enough of each that
    // one card per lease would be obvious.
    for name in ["status-writer", "retry-jitter", "chores-1"] {
        seed(
            &mut host,
            &format!("unit/atlas/{name}"),
            "unit",
            &[("life", "proposed")],
            &[("repository", json!("atlas"))],
        );
    }
    for name in ["baseline-1", "spike-1"] {
        seed(
            &mut host,
            &format!("unit/new-repo/{name}"),
            "unit",
            &[("life", "proposed")],
            &[("repository", json!("new-repo"))],
        );
    }
    host.sweep().unwrap();
    host.sweep().unwrap();

    // The host really took the three it covers and none of the two it does not.
    let held: Vec<&str> = ["unit/atlas/status-writer", "unit/atlas/retry-jitter", "unit/atlas/chores-1"]
        .into_iter()
        .filter(|id| {
            host.store
                .leases(id)
                .unwrap()
                .is_some_and(|l| l.holder == "mac-mini")
        })
        .collect();
    assert_eq!(held.len(), 3, "the host took the three it covers: {held:?}");

    let defs = host.defs.clone();
    let rail = flywheel_domain::commands::rail(&mut host.store, &defs).unwrap();
    // Five units are proposed, so five of the work's own decisions stand.
    assert_eq!(
        rail.iter().filter(|d| d.kind == "unit-proposed").count(),
        5,
        "the work's own decisions stand, and they are what the rail is for: {:?}",
        rail.iter().map(|d| (&d.kind, &d.object)).collect::<Vec<_>>()
    );
    // The three this host holds raise nothing: a lease that is merely held is
    // not a decision.
    let held_on_the_rail: Vec<String> = rail
        .iter()
        .filter(|d| {
            ["status-writer", "retry-jitter", "chores-1"]
                .iter()
                .any(|name| d.object == format!("lease/unit/atlas/{name}"))
        })
        .map(|d| format!("{} {} {:?}", d.kind, d.object, d.number))
        .collect();
    assert!(
        held_on_the_rail.is_empty(),
        "a lease this host holds took a number on the rail: {held_on_the_rail:?}"
    );
    // The two nothing covers each raise one, numbered and answerable (149,
    // `engine/lease.yaml` uncovered).
    let uncovered: Vec<&str> = rail
        .iter()
        .filter(|d| d.kind == "uncovered")
        .map(|d| d.object.as_str())
        .collect();
    assert_eq!(
        uncovered,
        vec![
            "lease/unit/new-repo/baseline-1",
            "lease/unit/new-repo/spike-1"
        ],
        "an object no declaration covers is a decision under attention (149)"
    );
    for decision in rail.iter().filter(|d| d.kind == "uncovered") {
        assert_eq!(decision.group, "attention");
        assert!(
            decision.number.is_some(),
            "a decision under attention is numbered like any other (15)"
        );
        assert_eq!(decision.answers, vec!["ok".to_string()]);
    }

    // What the lease is in is read on the status view's row for the object,
    // beside which host holds it.
    let status = host.status().unwrap();
    let row = |id: &str| {
        status
            .rows
            .iter()
            .find(|r| r.object == id)
            .unwrap_or_else(|| panic!("{id} has a row on the status view"))
    };
    assert_eq!(row("unit/atlas/status-writer").holder.as_deref(), Some("mac-mini"));
    assert_eq!(row("unit/atlas/status-writer").lease.as_deref(), Some("held"));
    assert_eq!(
        row("unit/new-repo/baseline-1").holder.as_deref().unwrap_or(""),
        "",
        "nothing holds it"
    );
    assert_eq!(
        row("unit/new-repo/baseline-1").lease.as_deref(),
        Some("uncovered"),
        "an object no declaration covers says so where its holder would be (149)"
    );
    let rendered = flywheel_domain::status::render(&status).body;
    assert!(
        rendered.contains("no host's declaration covers this"),
        "and the view says it in words a person reads"
    );

    // And it is under attention, which is where 149 puts it.
    let attention = host.attention().unwrap();
    assert!(
        attention.iter().any(|a| a == "uncovered: lease/unit/new-repo/baseline-1"),
        "the uncovered object is under attention: {attention:?}"
    );
}

// ------------------------------------------------------- 16.10 the tick's cost

/// A host over the manifest `flywheel init` writes, so the tick reads its
/// blueprints through the world the binary binds and not a stand-in: the
/// cost measured is the path the binary takes (169, audit 8).
fn hosted(name: &str) -> Host {
    let dir = dir(name);
    let report = flywheel::init::run(flywheel::init::Init {
        instance: "willdan".into(),
        host: "mac-mini".into(),
        root: dir.join("root"),
        git_host: dir.join("git-host"),
        app: "12345".into(),
        app_key_from: format!("FLYWHEEL_TEST_KEY_{}", name.to_uppercase()),
        app_key: Some("the operator placed this".into()),
        address: "http://laptop.example".into(),
        manifest: dir.join("flywheel.yaml"),
        repositories: vec![],
        curation: None,
        at: at(0),
    })
    .expect("init runs");
    assert_eq!(report.state, "hosted", "{report:?}");
    Host::open(&dir.join("flywheel.yaml"), "mac-mini", None, at(0)).expect("the host opens")
}

/// A real tick spends what the git-only profile's mechanism says it spends,
/// counted on the path the binary takes and not the scenario runner's: one
/// fetch before it decides, no renewal while none is due, no process on the
/// read path, every process it spawns a push, and the tick's commits at the
/// shared line in one push (165, 167, 169, `git-only.yaml` observations;
/// audit 8). The blueprints are read through the world the binary binds, and
/// that read spawns nothing either (`host.yaml` Tools; audit 9).
///
/// Three ticks under `conformance/contract/cost.yaml`'s shape — nothing
/// moves, one transition, nothing moves — with the clock still, so no lease
/// comes due.
///
/// The contract's `pushes_per_tick: [0, 1, 0]` is asserted as written: a
/// tick that moved nothing pushes nothing, the tick that moved pushes once,
/// with the heartbeat on the sweep's cadence and the rail's projection
/// current (78, 167, 169; `cost.yaml`).
#[test]
fn a_real_tick_spends_what_the_profile_says() {
    let mut host = hosted("cost");
    // The first sweep takes the leases and writes what a fresh host writes;
    // what it spends is the cost of arriving, not of a tick (147, 149).
    host.sweep().unwrap();

    let world_spawned_before = flywheel_world_host::git::spawned();
    let mut costs = vec![];
    // Nothing moves.
    host.tick(&Scope::All).unwrap();
    costs.push(flywheel_atoms::StateStore::cost(&host.store.git));
    // Another host is described as last seen ten minutes ago, so the `older:`
    // guard on its life fires on the next tick without the clock moving —
    // one transition, one commit (130, D7).
    seed(
        &mut host,
        "host/mini-2",
        "host",
        &[],
        &[
            ("last_seen", json!((at(0) - Duration::minutes(10)).to_rfc3339())),
            ("bound", json!(1)),
            ("intermittent", json!(false)),
        ],
    );
    host.tick(&Scope::All).unwrap();
    costs.push(flywheel_atoms::StateStore::cost(&host.store.git));
    let held = host.store.get("host/mini-2").unwrap().unwrap();
    assert_eq!(held.config.get("life").map(String::as_str), Some("stale"), "the guard did not fire");
    // Nothing moves.
    host.tick(&Scope::All).unwrap();
    costs.push(flywheel_atoms::StateStore::cost(&host.store.git));

    for (n, cost) in costs.iter().enumerate() {
        // One fetch, before deciding, and no other (165).
        assert_eq!(cost.fetches, 1, "tick {n} fetched {} times: {costs:?}", cost.fetches);
        // No lease came due, so none was renewed (128, 150).
        assert_eq!(cost.lease_renewals, 0, "tick {n} renewed a lease that was not due: {costs:?}");
        // The read path spawns nothing (126, 165).
        assert_eq!(cost.read_processes, 0, "tick {n} spawned a process to read: {costs:?}");
        // Every process a tick spawns is a push (169, `git-only.yaml` Tools).
        assert_eq!(cost.subprocesses, cost.pushes, "tick {n} spawned something that was not a push: {costs:?}");
    }
    // The contract's numbers, per tick in order: nothing moves, one write,
    // nothing moves (`cost.yaml` pushes_per_tick, subprocesses_per_tick).
    let pushes: Vec<u32> = costs.iter().map(|c| c.pushes).collect();
    assert_eq!(pushes, vec![0, 1, 0], "a quiet tick pushed, or the write did not: {costs:?}");
    // Reading the blueprints through the bound world forked nothing (audit 9).
    assert_eq!(
        flywheel_world_host::git::spawned() - world_spawned_before,
        0,
        "the world spawned a process to read the blueprints"
    );
}

// ------------------------------------------------------ 16.12 a session refused

/// A session that attempts a line operation is refused at the catalogue, the
/// refusal stands on its own thread, and the next tick carries it to the run
/// record and to attention with the identity and the operation (43, 79, 81;
/// audit 10).
#[test]
fn a_session_is_refused_a_line_operation() {
    let mut host = host("session-refused", &["atlas"]);
    let session = "session/unit/atlas/u/main";
    flywheel_sessions_operator::set(
        &mut host.store.git,
        session,
        &[
            ("runner", json!("operator")),
            ("place", json!("unit/atlas/u")),
            ("started_at", json!(at(0).to_rfc3339())),
        ],
    )
    .unwrap();

    // The session, as the caller, orders a take on its line.
    let defs = host.defs.clone();
    let call = flywheel_surface::catalogue::Call::new("take", session, "session")
        .arg("line", json!("line/atlas/u"));
    let refused = host
        .store
        .with_world(|store, world| flywheel_surface::catalogue::call(&mut store.git, world, &defs, &call))
        .expect_err("a session never merges (43)");
    assert!(format!("{refused}").contains("43"), "{refused}");

    // The refusal is the session's own entry, naming the operation.
    let entry = host
        .store
        .git
        .thread(session)
        .unwrap()
        .into_iter()
        .find(|e| e.kind == "refusal")
        .expect("the refusal is on the session's thread");
    assert_eq!(entry.fields.get("operation"), Some(&json!("take")));
    assert_eq!(entry.fields.get("object"), Some(&json!("line/atlas/u")));
    assert_eq!(entry.by.as_deref(), Some(session));

    // And nothing was taken: no response record was written for it.
    let responses = host.store.git.list(&Scope::Machine("response".into())).unwrap().objects;
    assert!(responses.is_empty(), "the refused call wrote a response: {responses:?}");

    // The next tick carries it to the run record and to attention (79, 81).
    host.set_now(at(2));
    host.sweep().unwrap();
    let record = host.store.git.run_record().unwrap();
    let refusal = record
        .iter()
        .find(|e| e.kind == "refusal")
        .expect("the refusal is in the run record (4, 79)");
    let fields: BTreeMap<&str, &str> = refusal.fields.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
    assert_eq!(fields.get("identity"), Some(&session));
    assert_eq!(fields.get("operation"), Some(&"take"));
    assert!(
        host.attention().unwrap().iter().any(|a| a == &format!("refusal: {session}")),
        "the refusal never reached attention (81, 82)"
    );

    // The operator, as the caller, is not a session: the same tool is theirs.
    let call = flywheel_surface::catalogue::Call::new("take", "chuck", "page").arg("line", json!("line/atlas/u"));
    host.store
        .with_world(|store, world| flywheel_surface::catalogue::call(&mut store.git, world, &defs, &call))
        .expect("the operator orders a take (50)");
}

/// Liveness is recorded on the sweep's cadence, never on every pass: a tick
/// that moved nothing writes no heartbeat, so `cost.yaml`'s quiet tick costs
/// no push on the host's own ref, and a host is still read alive inside the
/// five-minute window because the sweep is a minute (78, 130, 147, 169).
#[test]
fn a_quiet_tick_writes_no_heartbeat() {
    let mut host = hosted("heartbeat");
    host.sweep().unwrap();
    let seen = |host: &Host| {
        host.store
            .git
            .hosts()
            .unwrap()
            .into_iter()
            .find(|h| h.host == "mac-mini")
            .map(|h| h.last_seen)
            .expect("the host has a heartbeat")
    };
    let first = seen(&host);

    // Two passes inside the interval, the clock moving less than a sweep.
    host.set_now(host.now() + Duration::seconds(10));
    host.tick(&Scope::All).unwrap();
    host.set_now(host.now() + Duration::seconds(10));
    host.tick(&Scope::All).unwrap();
    assert_eq!(seen(&host), first, "a pass inside the interval re-stamped the heartbeat");

    // The interval passes: the next tick records liveness again.
    host.set_now(host.now() + host.sweep_interval());
    host.tick(&Scope::All).unwrap();
    assert!(seen(&host) > first, "the sweep's cadence did not write the heartbeat");
}

// -------------------------------------------------- a tick that moves nothing

/// Reading the same stores twice with nothing changed produces the same
/// conclusion and no writes (78). A host that has settled keeps ticking — the
/// sweep is what makes `older:` guards fire and a never-notified host converge
/// (D7, 130) — and every one of those ticks must leave the shared line where it
/// found it.
///
/// It did not. The rail's record was rewritten on every derive, the host's own
/// record on every declare, and each rewrite moved the shared line, which the
/// next pass read as news and ticked again. A settled instance wrote a commit
/// every couple of seconds with nothing happening in it, and the pass that made
/// them is the pass a page request waits behind (167, D11).
#[test]
fn a_tick_that_moves_nothing_writes_nothing() {
    let mut host = host("moves-nothing", &["atlas"]);
    seed(
        &mut host,
        "unit/atlas/status-writer",
        "unit",
        &[("life", "proposed")],
        &[("repository", json!("atlas")), ("type", json!("default"))],
    );
    let commits = |host: &Host| -> usize {
        host.store
            .git
            .repo
            .git(&["log", "--format=%H", "HEAD"])
            .unwrap()
            .lines()
            .count()
    };

    // Settle: sweep until the machinery stops moving. What is left standing is
    // a proposed unit, which is the operator's to answer and moves no further.
    for minute in 1..6 {
        host.set_now(at(minute));
        host.sweep().unwrap();
        if !host.moved {
            break;
        }
    }
    assert!(
        !host.moved,
        "the machinery had not settled, so there is nothing to prove about a tick that moves \
         nothing"
    );
    let settled = commits(&host);
    let seq = |host: &Host, id: &str| host.store.get(id).unwrap().map(|o| o.seq).unwrap_or(0);
    let rail_at = seq(&host, flywheel_domain::RAIL);
    let host_at = seq(&host, "host/mac-mini");

    // Three more sweeps over a settled instance. The clock moves, because a
    // host's does; nothing else does.
    for minute in 6..9 {
        host.set_now(at(minute));
        host.sweep().unwrap();
    }

    assert_eq!(
        commits(&host),
        settled,
        "a settled instance wrote {} commits over three sweeps in which nothing moved (78, 167)",
        commits(&host) - settled
    );
    assert_eq!(seq(&host, flywheel_domain::RAIL), rail_at, "the rail's record was rewritten");
    assert_eq!(seq(&host, "host/mac-mini"), host_at, "the host's own record was rewritten");
}

/// The status projection is rewritten whenever what it projects moved, and that
/// rewrite is not drift. Drift is the projection saying something its source
/// does not with nothing to account for it, which is the one case the report is
/// for — a report on every ordinary rewrite buries it (77, 142).
#[test]
fn an_ordinary_rewrite_of_the_projection_is_not_drift() {
    let mut host = host("not-drift", &["atlas"]);
    host.sweep().unwrap();
    seed(
        &mut host,
        "unit/atlas/status-writer",
        "unit",
        &[("life", "proposed")],
        &[("repository", json!("atlas")), ("type", json!("default"))],
    );
    for minute in 1..6 {
        host.set_now(at(minute));
        host.sweep().unwrap();
    }

    let written = host.store.git.committed_status().unwrap().unwrap();
    assert!(
        written.contains("unit/atlas/status-writer"),
        "the projection was rewritten from its source as the state moved"
    );
    let drift: Vec<String> = host
        .store
        .git
        .run_record()
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == "drift")
        .map(|e| e.reason)
        .collect();
    assert!(
        drift.is_empty(),
        "the projection following its source was reported as drift: {drift:?}"
    );
}

/// One response is enough, and the machinery takes it up on the pass the
/// response causes (13, 129, 130, D6).
///
/// An answer is a local cause: it reaches this process and does not wait for
/// the poll. But it moves no object file, so the notice a host takes from the
/// changed object files named nothing at all, the notify-tick had nothing to
/// tick, and the answer sat until the sweep came round — up to a minute in
/// which the operator had clicked and nothing had happened. An operator with no
/// sign their answer was taken is an operator who nudges, which is the thing 13
/// says they never do.
#[test]
fn an_answer_is_taken_up_on_the_pass_it_causes() {
    let mut host = host("answered-at-once", &["atlas"]);
    seed(
        &mut host,
        "unit/atlas/status-writer",
        "unit",
        &[("life", "proposed")],
        &[("repository", json!("atlas")), ("type", json!("default"))],
    );
    // Settle, so the decision stands and is numbered.
    for minute in 1..6 {
        host.set_now(at(minute));
        host.sweep().unwrap();
        if !host.moved {
            break;
        }
    }
    let defs = host.defs.clone();
    let number = flywheel_domain::commands::rail(&mut host.store, &defs)
        .unwrap()
        .iter()
        .find(|d| d.object == "unit/atlas/status-writer")
        .and_then(|d| d.number)
        .expect("the proposed unit is a numbered decision");

    // The operator answers on the page. The response is a record of its own and
    // moves no object file.
    host.set_now(at(6));
    host.store
        .receive(&flywheel_engine::runtime::Response {
            id: "page-1".into(),
            kind: flywheel_engine::runtime::ResponseKind::Answer,
            decision: Some(number),
            object: None,
            answer: "yes".into(),
            given_by: "chuck".into(),
            given_at: at(6),
            delivery: "page".into(),
        })
        .unwrap();

    // What the host is told moved names the object the answer answers, so the
    // notify-tick has something to tick without waiting for a sweep.
    let notified = host.notified().unwrap();
    assert!(
        notified.iter().any(|o| o == "unit/atlas/status-writer"),
        "the answer named no object to tick: {notified:?}"
    );

    // And the pass that notice drives applies it. The sweep is not what took
    // it: this ticks the notified chain alone.
    for scope in [
        Scope::Under("unit/atlas/status-writer".to_string()),
    ] {
        host.tick(&scope).unwrap();
    }
    let after = host
        .store
        .get("unit/atlas/status-writer")
        .unwrap()
        .expect("the unit")
        .config
        .get("life")
        .cloned()
        .unwrap_or_default();
    assert_ne!(
        after, "proposed",
        "the answer was not applied by the pass it caused"
    );
}

/// A refusal is reported once, and not again for every pass in which it is
/// still true (81, 167).
///
/// An effect this release does not bind leaves its proof absent, so the engine
/// plans it on every tick and the binding refuses it on every tick. Writing
/// that refusal each time put a run-record commit a second on the shared line
/// for a deferral nothing was going to act on — an instance with nothing
/// happening on it, committing for ever. 81 asks that a refusal be reported and
/// never dropped; it does not ask that it be repeated.
#[test]
fn a_refusal_is_said_once_and_not_on_every_pass() {
    let mut host = host("refused-once", &["all"]);
    // The scenario the page's walkthrough stands on: it holds objects whose
    // effects this phase defers, so the engine plans them on every tick and the
    // binding refuses them on every tick (93a).
    let scenario = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scenarios/rail-mockup.yaml");
    let at_now = host.now();
    let declaration = host.declaration.clone();
    flywheel::seed::from_scenario(&mut host.store.git, &scenario, at_now, &declaration)
        .expect("the scenario's given state");
    // Two passes is the whole of the proof: the second is the one that would
    // say it again.
    for minute in 1..3 {
        host.set_now(at(minute));
        host.sweep().unwrap();
    }

    let refusals: Vec<(String, String)> = host
        .store
        .git
        .run_record()
        .unwrap()
        .into_iter()
        .filter(|e| e.fields.iter().any(|(name, _)| name == "refused"))
        .map(|e| (e.object.clone(), e.reason.clone()))
        .collect();
    assert!(
        !refusals.is_empty(),
        "an effect no binding covers is refused in the open (81); nothing was refused here"
    );
    let mut once: BTreeMap<(String, String), usize> = BTreeMap::new();
    for said in &refusals {
        *once.entry(said.clone()).or_default() += 1;
    }
    let repeated: Vec<&(String, String)> = once
        .iter()
        .filter(|(_, how_many)| **how_many > 1)
        .map(|(said, _)| said)
        .collect();
    assert!(
        repeated.is_empty(),
        "a refusal was written again for a pass in which nothing about it changed, \
         which is a commit at the shared line saying nothing new (81, 167): {repeated:?}"
    );
}
