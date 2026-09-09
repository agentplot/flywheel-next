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
/// what the world outside these tests would have written.
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
    host.store.git.seed_object(&object).unwrap();
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
    // Work the operator approved, waiting on the host that holds it.
    seed(
        &mut host,
        "unit/atlas/u",
        "unit",
        &[("life", "approved")],
        &[("repository", json!("atlas")), ("type", json!("default"))],
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

    // The laptop is shut for a day. The other host sees it gone.
    a.go_away(at(0));
    b.set_now(at(60 * 25));
    b.sweep().unwrap();
    b.sweep().unwrap();
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
    a.set_now(at(60 * 26));
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
                let called =
                    flywheel_surface::catalogue::call(&mut host.store, &defs, &call).unwrap();
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
                let heard = chat
                    .receive(
                        &mut host.store,
                        &defs,
                        &flywheel_surface::chat::Message::new(
                            "1801",
                            "chuck",
                            &format!("yes {number}"),
                        ),
                    )
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
            host.now() - swept < Duration::seconds(flywheel::host::SWEEP),
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
