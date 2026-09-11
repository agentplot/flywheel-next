//! What the binary performs, and what it refuses in the open (task 16.1).
//!
//! The scenarios pass on the harness's own bodies; these run a real `Host` over
//! a real state repository, so an effect the machines name and the binary does
//! not perform is caught here rather than counted as done (81, 127).

use chrono::{DateTime, Duration, TimeZone, Utc};
use flywheel::host::{perform, Bindings, Host, Performed, DEFERRED};
use flywheel_atoms::{Records, Scope, StateStore};
use flywheel_domain::derived::Declaration;
use flywheel_engine::{Object, PlannedEffect};
use flywheel_store_git::store::sandbox;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

fn at(minute: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap() + Duration::minutes(minute)
}

fn dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "flywheel-effects-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn host(name: &str) -> Host {
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
        Declaration {
            repositories: vec!["atlas".into()],
            types: vec![],
            kinds: vec!["all".into()],
        },
        now,
    )
}

fn seed(
    host: &mut Host,
    id: &str,
    machine: &str,
    states: &[(&str, &str)],
    parent: Option<&str>,
    record: &[(&str, serde_json::Value)],
) {
    let mut object = Object {
        id: id.to_string(),
        machine: machine.to_string(),
        parent: parent.map(String::from),
        config: states.iter().map(|(r, s)| (r.to_string(), s.to_string())).collect(),
        entered_at: states.iter().map(|(r, _)| (r.to_string(), host.now())).collect(),
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

/// Every effect name the machines in `definitions/` name, once.
fn named_by_the_machines() -> BTreeSet<String> {
    let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../definitions"));
    let mut out = BTreeSet::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).expect("definitions is readable").flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("a machine file");
            for found in text.match_indices("do: ") {
                let rest = &text[found.0 + 4..];
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_lowercase() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    out.insert(name);
                }
            }
        }
    }
    assert!(out.len() > 60, "the machines name {} effects", out.len());
    out
}

/// The effects of phases this release does not build, with the clause or the
/// deferred scenario that says so. An effect out of phase is not one this
/// release has to refuse in the open — it is one no phase-1 machine reaches,
/// and the deferral is the proposal's (proposal.md — Deferred).
const OUT_OF_PHASE: &[(&str, &str)] = &[
    // A.14: claims, the ledger and verdicts — phase 3.
    ("attach_claim", "claims are A.14, phase 3"),
    ("detach_claim", "claims are A.14, phase 3"),
    ("record_verdict", "the ledger is A.14, phase 3"),
    ("record_citation_choice", "citations are A.14, phase 3"),
    ("set_covers", "the map's scope is A.16, phase 3"),
    // A.5: planning and the proposal document — phase 2.
    ("propose_units", "planning waits on the runner, phase 2"),
    ("record_proposal", "planning waits on the runner, phase 2"),
    ("record_fingerprint", "planning waits on the runner, phase 2"),
    ("record_per_intent", "planning waits on the runner, phase 2"),
    ("request_replan", "planning waits on the runner, phase 2"),
    ("open_request", "the pull-request landing waits on the runner, phase 2"),
    ("seed_request_finding", "the pull-request landing waits on the runner, phase 2"),
    ("seed_take_conflict", "a real take conflict needs a real line, phase 2"),
    ("seed_conflict_job", "a real conflict needs a real line, phase 2"),
    ("remove_stray_places", "worktrees are phase 2"),
    ("remove_stale_layout", "the multiplexer's layout is phase 2"),
    ("forward_answer", "the proposal's per-unit answer waits on planning, phase 2"),
    ("assign_owner", "owners on decisions wait on membership, phase 4"),
    // A.28-A.32: packages, pools, enrolment — phases 3 to 5.
    ("install_package", "packages are A.28, phase 3"),
    ("configure_package", "packages are A.28, phase 3"),
    ("disable_package", "packages are A.28, phase 3"),
    ("remove_package", "packages are A.28, phase 3"),
    ("build_image", "pool images are phase 5"),
    ("provision_pool_host", "pools are phase 5"),
    ("retire_pool_host", "pools are phase 5"),
    ("issue_enrolment_token", "host enrolment is phase 5"),
    ("extend_installation", "the App's installation flow is phase 5"),
    ("place_secrets", "a host's secrets are placed at enrolment, phase 5"),
];

/// The effects the instance machine performs at bootstrap, outside any tick.
///
/// `flywheel init` walks the instance machine to completion before a host ever
/// runs (204, `init.rs`), so these are bound there and never reach
/// `host::perform`.
const AT_BOOTSTRAP: &[&str] = &[
    "create_blueprints",
    "create_state",
    "create_repository",
    "register_app",
    "register_host",
    "register_repository",
    "clone_repositories",
    "retire_instance",
    "repair_layout",
];

/// The effects the tick performs outside `perform`, in the pass that owns them.
const IN_THE_TICK: &[&str] = &[
    // The register's numbering and the run record's entries are the tick's own
    // writes, made where the tick makes them (15, 79).
    "number_decisions",
    "record_exit",
    "report",
    // The recorded workspace writes these as it performs the effect they belong
    // to (93a, `flywheel-workspace-recorded`).
    "record_endpoints",
    "record_service_endpoint",
    "archive_intent",
    "archive_change",
];

/// Every effect the machines name is performed by the binary, or refused in the
/// open with its reason — and none is silently counted as done (81, 127,
/// task 16.1).
#[test]
fn every_effect_the_machines_name_is_bound_or_refused() {
    let mut host = host("bound-or-refused");
    let defs = host.defs.clone();
    let now = host.now();
    let me = "mac-mini".to_string();

    let mut unbound: Vec<String> = Vec::new();
    for name in named_by_the_machines() {
        if OUT_OF_PHASE.iter().any(|(n, _)| *n == name)
            || AT_BOOTSTRAP.contains(&name.as_str())
            || IN_THE_TICK.contains(&name.as_str())
            || DEFERRED.iter().any(|(n, _)| *n == name)
        {
            continue;
        }
        let effect = PlannedEffect {
            id: format!("test/{name}"),
            name: name.clone(),
            args: BTreeMap::new(),
            note: None,
        };
        let Host { store, sinks, .. } = &mut host;
        let did = perform(&defs, store, sinks, &me, now, "unit/atlas/nothing", "life", &effect);
        if let Performed::Refused(why) = did {
            if why.starts_with("no binding") {
                unbound.push(format!("{name}: {why}"));
            }
        }
    }
    assert!(
        unbound.is_empty(),
        "the machines name these and the binary performs none of them, \
         and none is in the deferral list with a reason (81, 127):\n{}",
        unbound.join("\n")
    );

    // And the fall-through is real: a name no binding covers is refused and
    // says so, rather than counting as done.
    {
        let effect = PlannedEffect {
            id: "test/nothing".into(),
            name: "an_effect_no_binding_covers".into(),
            args: BTreeMap::new(),
            note: None,
        };
        let Host { store, sinks, .. } = &mut host;
        let did = perform(&defs, store, sinks, &me, now, "unit/atlas/nothing", "life", &effect);
        assert_eq!(
            did,
            Performed::Refused(
                "no binding in this release performs `an_effect_no_binding_covers`".into()
            ),
            "an unbound effect is counted as done"
        );
    }

    // The deferral list is reasons, not a hiding place: each entry names an
    // effect the machines actually have, and says why this release does not
    // perform it.
    let machines = named_by_the_machines();
    for (name, why) in DEFERRED {
        assert!(
            machines.contains(*name),
            "`{name}` is deferred and no machine names it"
        );
        assert!(why.len() > 40, "`{name}` is deferred with no reason");
    }

    // And a deferred effect is refused with that reason, never done.
    for (name, why) in DEFERRED {
        let effect = PlannedEffect {
            id: format!("test/{name}"),
            name: (*name).to_string(),
            args: BTreeMap::new(),
            note: None,
        };
        let Host { store, sinks, .. } = &mut host;
        let did = perform(&defs, store, sinks, &me, now, "bolt/atlas/plan-rows", "life", &effect);
        assert_eq!(
            did,
            Performed::Refused((*why).to_string()),
            "`{name}` is not refused with its reason"
        );
    }
}

/// A yes on a unit creates its work items, on the binary and not only in the
/// harness (13, 36, 37, 57).
#[test]
fn a_yes_on_a_unit_creates_its_work_items() {
    let mut host = host("a-yes-on-a-unit");
    seed(
        &mut host,
        "bolt/atlas/plan-rows",
        "bolt",
        &[("life", "open")],
        None,
        &[("repository", json!("atlas"))],
    );
    seed(
        &mut host,
        "unit/atlas/plan-rows/rail-tail",
        "unit",
        &[("life", "proposed")],
        Some("bolt/atlas/plan-rows"),
        &[
            ("repository", json!("atlas")),
            ("type", json!("default")),
            ("items", json!(3)),
            ("serial", json!(true)),
            ("target", json!({"bolt": "bolt/atlas/plan-rows"})),
        ],
    );

    // The decision the unit raises, and the operator's yes through the one tool
    // the reply grammar calls (15, 193).
    host.sweep().unwrap();
    let defs = host.defs.clone();
    let number = flywheel_domain::commands::rail(&mut host.store, &defs)
        .unwrap()
        .iter()
        .find(|d| d.object == "unit/atlas/plan-rows/rail-tail")
        .and_then(|d| d.number)
        .expect("the unit's proposal is a numbered decision on the rail");
    let call = flywheel_surface::catalogue::Call::new("answer", "chuck", "page")
        .arg("decision", json!(number))
        .arg("answer", json!("yes"));
    host.store
        .with_world(|store, world| {
            flywheel_surface::catalogue::call(store, world, &defs, &call)
        })
        .expect("the answer is recorded");

    // The tick applies it, and the unit's items exist (13, 36).
    for minute in 1..6 {
        host.set_now(at(minute));
        host.sweep().unwrap();
    }
    let unit = Records::get(&host.store, "unit/atlas/plan-rows/rail-tail")
        .unwrap()
        .expect("the unit");
    // Approved, and past it: `approved` falls through to `waiting` while a
    // dependency of the same bolt is unmerged, and `create_items` fires on the
    // way in (31, `unit.yaml` approved).
    assert!(
        matches!(unit.top_state(), Some("approved") | Some("waiting") | Some("in-flight")),
        "the yes did not approve the unit: {:?}",
        unit.config
    );
    let items: Vec<Object> = StateStore::list(&host.store, &Scope::All)
        .unwrap()
        .objects
        .into_iter()
        .filter(|o| {
            o.machine == "work-item" && o.parent.as_deref() == Some("unit/atlas/plan-rows/rail-tail")
        })
        .collect();
    assert_eq!(
        items.len(),
        3,
        "a yes on a unit created no work items on the binary (13, 36): {:?}",
        items.iter().map(|o| &o.id).collect::<Vec<_>>()
    );

    // Each one carries its ordinal and the type in force, and a serial unit's
    // items depend on the one before (31, 32, 37, 57).
    for (ordinal, item) in items.iter().enumerate() {
        assert_eq!(item.record.get("ordinal"), Some(&json!(ordinal + 1)));
        assert_eq!(item.record.get("type"), Some(&json!("default")));
        match ordinal {
            0 => assert!(item.record.get("depends_on").is_none()),
            _ => assert_eq!(
                item.record.get("depends_on"),
                Some(&json!([format!(
                    "work-item/atlas/plan-rows/rail-tail/wi-{ordinal}"
                )])),
                "a serial unit's items run in order (31)"
            ),
        }
    }

    // And a second tick makes no more: the proof holds, so the act is not run
    // again (73, 127).
    host.set_now(at(9));
    host.sweep().unwrap();
    let again = StateStore::list(&host.store, &Scope::All)
        .unwrap()
        .objects
        .into_iter()
        .filter(|o| o.machine == "work-item")
        .count();
    assert_eq!(again, 3, "the items were made twice");
}

// ------------------------------------ 16.2 a failing effect is not a done one

/// An effect whose binding fails leaves its proof absent, is reported with its
/// reason, and the tick carries on (81, 127, 148).
///
/// The failure is a real one and not a stub: the host declares a source no
/// adapter of this release enumerates, so `run_adapters` runs, fails and says
/// why (215, D13).
#[test]
fn a_failing_effect_is_reported_and_leaves_no_proof() {
    let mut host = host("a-failing-effect");
    let defs = host.defs.clone();
    let now = host.now();
    let me = "mac-mini".to_string();

    // The host record names a source, and the source names no adapter.
    seed(
        &mut host,
        "host/mac-mini",
        "host",
        &[],
        None,
        &[("sources", json!(["a-source-no-adapter-reads"]))],
    );
    let effect = PlannedEffect {
        id: "host/mac-mini/alive/host.adapters_run/0".into(),
        name: "run_adapters".into(),
        args: BTreeMap::new(),
        note: None,
    };
    let Host { store, sinks, .. } = &mut host;
    let did = perform(&defs, store, sinks, &me, now, "host/mac-mini", "life", &effect);
    assert!(
        matches!(did, Performed::Failed(_)),
        "a binding that failed reported success: {did:?}"
    );
    assert!(
        did.why().contains("names no adapter"),
        "the failure does not carry the binding's own reason (81): {}",
        did.why()
    );

    // The proof is where it was: no capture was written, so the act is still
    // owed and the next tick attempts it again (127).
    let captures = StateStore::list(&host.store, &Scope::Machine("capture".into()))
        .unwrap()
        .objects;
    assert!(captures.is_empty(), "the failing effect wrote a capture");

    // And the tick carries on: a host whose effect failed still ticks, still
    // writes its run record, and reports the reason under attention (81, 79).
    seed(
        &mut host,
        "bolt/atlas/plan-rows",
        "bolt",
        &[("life", "open"), ("life.open.close", "offered")],
        None,
        &[("repository", json!("atlas"))],
    );
    host.set_now(at(1));
    host.sweep().expect("the tick carries on past a failing effect");
    let decisions = flywheel_domain::commands::rail(&mut host.store, &defs).unwrap();
    assert!(
        decisions.iter().any(|d| d.object == "bolt/atlas/plan-rows"),
        "the tick stopped at the failure"
    );
}

/// A deferred effect is reported under attention on the tick that reaches it,
/// with the reason the deferral names, and is never recorded as performed
/// (81, 127).
#[test]
fn a_deferred_effect_reaches_attention() {
    let mut host = host("deferred-to-attention");
    let defs = host.defs.clone();
    let now = host.now();
    let me = "mac-mini".to_string();
    let (name, why) = DEFERRED[0];

    let effect = PlannedEffect {
        id: format!("bolt/atlas/plan-rows/declared/{name}/0"),
        name: name.to_string(),
        args: BTreeMap::new(),
        note: None,
    };
    let Host { store, sinks, .. } = &mut host;
    let did = perform(&defs, store, sinks, &me, now, "bolt/atlas/plan-rows", "services", &effect);
    assert_eq!(did, Performed::Refused(why.to_string()));
    assert!(!did.done(), "a deferred effect counted as done");
}

// --------------------------------- 16.3 a real host brings a place to ready

/// A real `flywheel host` brings a place to `ready` (50, 51, `place.yaml`).
///
/// `place.yaml` needs `place.exists && place.contains_line` to leave
/// `preparing`, and until the recorded workspace answered the second the host
/// re-ran `prepare_place` every tick and every elaboration and work item
/// stalled behind it.
#[test]
fn a_real_host_brings_a_place_to_ready() {
    let mut host = host("place-to-ready");
    seed(
        &mut host,
        "bolt/atlas/plan-rows",
        "bolt",
        &[("life", "open")],
        None,
        &[("repository", json!("atlas"))],
    );
    seed(
        &mut host,
        "unit/atlas/plan-rows/rail-tail",
        "unit",
        &[("life", "proposed")],
        Some("bolt/atlas/plan-rows"),
        &[
            ("repository", json!("atlas")),
            ("type", json!("default")),
            ("items", json!(1)),
            ("target", json!({"bolt": "bolt/atlas/plan-rows"})),
        ],
    );

    // The operator's yes, which is what makes the work (13, 193).
    host.sweep().unwrap();
    let defs = host.defs.clone();
    let number = flywheel_domain::commands::rail(&mut host.store, &defs)
        .unwrap()
        .iter()
        .find(|d| d.object == "unit/atlas/plan-rows/rail-tail")
        .and_then(|d| d.number)
        .expect("the unit's proposal is a numbered decision");
    let call = flywheel_surface::catalogue::Call::new("answer", "chuck", "page")
        .arg("decision", json!(number))
        .arg("answer", json!("yes"));
    host.store
        .with_world(|store, world| {
            flywheel_surface::catalogue::call(store, world, &defs, &call)
        })
        .expect("the answer is recorded");

    for minute in 1..10 {
        host.set_now(at(minute));
        host.sweep().unwrap();
    }

    // The item the approval created, and the place beside it (13, 36).
    let item = StateStore::list(&host.store, &Scope::All)
        .unwrap()
        .objects
        .into_iter()
        .find(|o| o.machine == "work-item")
        .expect("the approved unit created its work item");

    // The place reached `ready`: it exists and it contains its line, which is
    // what `place.yaml` asks before any session starts in it.
    let place = flywheel_domain::regions::place_key(&item.id, "place.place.life");
    assert_eq!(
        flywheel_workspace_recorded::evidence(&host.store, &place, "place.exists"),
        Some(json!(true)),
        "the host never prepared the item's place"
    );
    assert_eq!(
        flywheel_workspace_recorded::evidence(&host.store, &place, "place.contains_line"),
        Some(json!(true)),
        "the place is prepared and does not contain its line, so it can never leave \
         `preparing` (51, `place.yaml`)"
    );
    let states: Vec<String> = item
        .config
        .iter()
        .filter(|(region, _)| region.contains("place"))
        .map(|(_, state)| state.clone())
        .collect();
    assert!(
        states.iter().any(|s| s == "ready" || s == "merging" || s == "merged"),
        "the item's place never reached ready: {:?}",
        item.config
    );
}
