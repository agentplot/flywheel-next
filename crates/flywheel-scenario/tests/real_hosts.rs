//! `--hosts real`: every host a scenario names is a process of its own, the
//! runner holds no engine, and it asserts only through the store (232, 134,
//! 162, I15, D15).

use flywheel_atoms::Records;
use flywheel_scenario::conformance::hosts::{HostSpec, RealHosts, BINARY_ENV};
use flywheel_scenario::conformance::{self, Profile, RunOptions};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// The `flywheel` binary beside the test binary. `cargo test -p
/// flywheel-scenario` does not build another package's binary, so build it when
/// it is not there; the runner itself is the binary and needs none of this.
fn flywheel_binary() -> PathBuf {
    let deps = std::env::current_exe().expect("the test binary has a path");
    let target = deps.parent().and_then(|p| p.parent()).expect("target/debug");
    let binary = target.join("flywheel");
    if !binary.exists() {
        let out = std::process::Command::new(env!("CARGO"))
            .args(["build", "--quiet", "-p", "flywheel"])
            .current_dir(root())
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

fn places(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "flywheel-real-hosts-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a place to run in");
    dir
}

fn at(minutes: i64) -> chrono::DateTime<chrono::Utc> {
    conformance::drive::start_of_time() + chrono::Duration::minutes(minutes)
}

// ---- 11.1 the harness

#[test]
fn two_host_harness() {
    std::env::set_var(BINARY_ENV, flywheel_binary());
    let base = places("harness");
    let names = vec!["mac-mini".to_string(), "studio".to_string()];
    let specs: Vec<HostSpec> = names.iter().map(|n| HostSpec::named(n)).collect();
    let mut hosts = RealHosts::open(&base, "conformance", &specs, &[], at(0))
        .expect("the roots and the manifest");

    // Each host is a child of the binary with a root of its own and a port
    // range from its router; two on one computer never collide (232).
    assert_eq!(hosts.root("mac-mini"), base.join("hosts/mac-mini"));
    assert_ne!(hosts.root("mac-mini"), hosts.root("studio"));
    let (low_a, high_a) = hosts.port_range("mac-mini");
    let (low_b, _) = hosts.port_range("studio");
    assert!(high_a < low_b, "one host's range ends before the next begins");
    assert!(low_a < high_a);

    hosts.start("mac-mini").expect("mac-mini starts");
    hosts.start("studio").expect("studio starts");
    assert_eq!(hosts.started(), names);

    // They take the runner's clock and sweep when the runner says to; nothing
    // in a child keeps time (D7, D15).
    hosts.set_clock(at(1)).expect("the clock moves");
    hosts.sweep_all().expect("both sweep");

    // A host lost is stopped without a farewell: no shutdown, no release.
    hosts.lose("mac-mini").expect("mac-mini is lost");
    assert!(!hosts.is_running("mac-mini"));
    assert!(hosts.is_running("studio"));
    // The one still running works on.
    hosts.set_clock(at(7)).expect("the clock moves");
    hosts.sweep_all().expect("studio sweeps alone");

    // And it comes back, as the process it was.
    hosts.returned("mac-mini").expect("mac-mini returns");
    assert!(hosts.is_running("mac-mini"));
    hosts.sweep_all().expect("both sweep again");

    // Both wrote their heartbeat on the one shared state repository, which is
    // what makes the expected-old push a real compare-and-swap (134, 162).
    let reader = flywheel_store_git::GitStore::open(
        &hosts.remote(),
        &base.join("reader"),
        "reader",
        at(7),
    )
    .expect("a reader of the shared line");
    let seen: Vec<String> = reader
        .hosts()
        .expect("the heartbeats")
        .into_iter()
        .map(|h| h.host)
        .collect();
    for name in &names {
        assert!(seen.contains(name), "{name} declared itself: {seen:?}");
    }

    hosts.shutdown();
    let _ = std::fs::remove_dir_all(&base);
}

// ---- 11.3 one rail, one set of numbers

#[test]
fn two_hosts_one_rail() {
    // The rail is derived from the active states and the register on every
    // read, and the register is the rail's record on the shared line: two hosts
    // of one instance therefore derive the same rail with the same numbers, or
    // one of them is deciding on something it holds in memory (7, 15, 147, 148).
    std::env::set_var(BINARY_ENV, flywheel_binary());
    let base = places("one-rail");
    let names = vec!["mac-mini".to_string(), "studio".to_string()];
    let specs: Vec<HostSpec> = names.iter().map(|n| HostSpec::named(n)).collect();
    let mut hosts = RealHosts::open(&base, "conformance", &specs, &["atlas".into()], at(0))
        .expect("the roots and the manifest");

    // A bolt whose close is offered: a decision stands on it, and the register
    // numbers it once, whoever numbered it.
    let mut seeding =
        flywheel_store_git::store::sandbox(&base.join("state"), "seed", at(0)).expect("a seed");
    let defs = flywheel_domain::set::load().expect("the set the binary carries");
    for (id, states) in [
        ("bolt/atlas/plan-rows", vec![("life", "open"), ("life.open.close", "offered")]),
        ("bolt/atlas/drop-the-tail", vec![("life", "open"), ("life.open.close", "offered")]),
    ] {
        let mut object = flywheel_engine::Object {
            id: id.into(),
            machine: "bolt".into(),
            parent: None,
            config: Default::default(),
            entered_at: Default::default(),
            record: [("repository".to_string(), serde_json::json!("atlas"))]
                .into_iter()
                .collect(),
            counters: Default::default(),
            applied_responses: vec![],
            seq: 0,
            created: 0,
        };
        flywheel_engine::initialise(&defs, &mut object, at(0));
        for (region, state) in states {
            object.config.insert(region.into(), state.into());
        }
        seeding.seed_object(&object).expect("the described state");
    }

    hosts.start("mac-mini").expect("mac-mini starts");
    hosts.start("studio").expect("studio starts");
    hosts.set_clock(at(1)).expect("the clock moves");
    hosts.sweep_all().expect("both sweep");
    hosts.set_clock(at(2)).expect("the clock moves");
    hosts.sweep_all().expect("both sweep again");

    let a = hosts.tell("mac-mini", "rail").expect("mac-mini derives the rail");
    let b = hosts.tell("studio", "rail").expect("studio derives the rail");
    assert!(
        a.contains("bolt/atlas/plan-rows") && a.contains("bolt/atlas/drop-the-tail"),
        "the rail carries the decisions standing: {a}"
    );
    assert!(
        a.split_whitespace().skip(1).all(|line| !line.starts_with("-=")),
        "every standing decision carries its number: {a}"
    );
    assert_eq!(a, b, "the two hosts derived different rails");

    hosts.shutdown();
    let _ = std::fs::remove_dir_all(&base);
}

// ---- 11.1a the same virtual clock, and a sweep the runner triggers

#[test]
fn two_host_run_is_deterministic() {
    // The child hosts take the runner's clock through an injected clock source
    // and their sweep is triggered by the runner rather than by a timer, so ten
    // runs of a two-host scenario are one run (D15, D7).
    std::env::set_var(BINARY_ENV, flywheel_binary());
    let scenario = root().join("conformance/scenarios/S13.yaml");
    let (parsed, _) = flywheel_atoms::conformance::load(&scenario).expect("the scenario loads");
    let options = RunOptions {
        definitions: Some(root().join("definitions")),
        profile: Profile::GitOnly,
        hosts_real: true,
        ..Default::default()
    };

    // Ten runs, a few at a time: each has its own roots and its own state
    // repository, and nothing they share is the wall clock.
    let mut traces: Vec<String> = Vec::new();
    for _ in 0..2 {
        let mut running = Vec::new();
        for _ in 0..5 {
            let (scenario, parsed, options) =
                (scenario.clone(), parsed.clone(), options.clone());
            running.push(std::thread::spawn(move || {
                // A suite of its own per run: the validator holds no state a
                // run changes, and nothing is shared but the files on disk.
                let suite = conformance::Suite::for_scenario(&scenario).expect("the suite opens");
                let run = conformance::drive::play(&parsed, &scenario, &suite, &options)
                    .expect("the run plays against real hosts");
                serde_json::to_string_pretty(&conformance::trace::of(&parsed, &run, &options))
                    .expect("the trace renders")
            }));
        }
        for handle in running {
            traces.push(handle.join().expect("the run finished"));
        }
    }

    assert_eq!(traces.len(), 10);
    for (n, trace) in traces.iter().enumerate().skip(1) {
        assert_eq!(
            trace, &traces[0],
            "run {n} of S13 traced differently from the first; a two-host run under `--hosts \
             real` takes one virtual clock and one triggered sweep, so it is one run (D15, D7)"
        );
    }
}

#[test]
fn runner_holds_no_engine_under_hosts_real() {
    std::env::set_var(BINARY_ENV, flywheel_binary());
    // A scenario the embedded set covers: a host runs the definitions in the
    // binary and never a directory, so a scenario naming its own `machines:`
    // is the in-process runner's alone (223, D2).
    let scenario = root().join("conformance/scenarios/S17.yaml");
    let options = RunOptions {
        definitions: Some(root().join("definitions")),
        profile: Profile::GitOnly,
        hosts_real: true,
        ..Default::default()
    };
    let (parsed, _) = flywheel_atoms::conformance::load(&scenario).expect("the scenario loads");
    let suite = conformance::Suite::for_scenario(&scenario).expect("the suite opens");
    let run = conformance::drive::play(&parsed, &scenario, &suite, &options)
        .expect("the run plays against real hosts");

    assert_eq!(
        run.engine_ticks, 0,
        "under `--hosts real` the hosts are processes and the runner ticks nothing (D15)"
    );
    // And it still saw the state: what it asserts came from the store, read
    // back over the shared line, and from nowhere else (136, 167).
    assert!(
        !run.runtime.store.objects.is_empty(),
        "the runner read the objects the hosts wrote"
    );
    assert!(
        run.ticks.iter().any(|t| !t.transitions.is_empty()),
        "the transitions the runner asserts were read from the run record on the shared line"
    );
}
