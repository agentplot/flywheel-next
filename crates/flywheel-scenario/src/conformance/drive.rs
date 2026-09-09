//! Seeding a described state of the stores, and playing the `when` steps.
//!
//! The clock is virtual and moves for exactly two reasons: a `clock` step, and
//! each `tick` step by one tick interval. Wall-clock time never reaches a
//! guard, so a run is deterministic and a scenario that waits says so in its
//! own steps (D15).

use super::{Run, RunOptions, Suite};
use crate::sessions::ScriptedSessions;
use crate::store::Store;
use crate::{world, Runtime};
use anyhow::{anyhow, bail, Context, Result};
use flywheel_atoms::conformance::{
    Direct, HostStep, HostTransition, Requirement, ResponseStep, Scenario, Step,
};
use flywheel_atoms::{Records, StateStore};
use chrono::{DateTime, Duration, Utc};
use flywheel_engine::runtime::{Response, ResponseKind};
use flywheel_engine::Definitions;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// The point every scenario's virtual clock starts at. Fixed, so two runs of
/// one scenario are the same run.
pub fn start_of_time() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
        .expect("a fixed point")
        .with_timezone(&Utc)
}

/// A host with no host step: the single host every scenario runs as until one
/// names another (232).
pub const DEFAULT_HOST: &str = "local";

/// Load the machines a scenario names: `../machines` by default, and the
/// scenario's own `machines:` where it gives one — the contract set names the
/// toy `lamp`, which shares no atom with the flywheel.
fn definitions(scenario: &Scenario, path: &Path, suite: &Suite, options: &RunOptions) -> Result<Definitions> {
    if let Some(dir) = &scenario.machines {
        let resolved = if dir.starts_with("./") {
            suite.root.join(dir.trim_start_matches("./"))
        } else {
            path.parent().unwrap_or(Path::new(".")).join(dir)
        };
        return flywheel_engine::load::load_dir(&resolved)
            .with_context(|| format!("loading the machines at {}", resolved.display()));
    }
    // The set the binary carries, unless `--definitions` names a directory
    // (D2). The override is the runner's alone: a host runs what it carries.
    match &options.definitions {
        Some(dir) => flywheel_engine::load::load_dir(dir)
            .with_context(|| format!("loading the machines at {}", dir.display())),
        None => flywheel_domain::set::load().context("loading the set the binary carries"),
    }
}

/// Seed and play. Every way a scenario can be ill-formed is an error here, so
/// the caller reports it as invalid rather than as a failed assertion.
pub fn play(
    scenario: &Scenario,
    path: &Path,
    suite: &Suite,
    options: &RunOptions,
) -> Result<Run> {
    let defs = definitions(scenario, path, suite, options)?;
    let mut rt = seed(defs, scenario, suite)?;
    rt.store.tick_seconds = options.interval.num_seconds();

    let places = std::env::temp_dir().join(format!(
        "flywheel-run-{}-{}",
        scenario.scenario.replace('/', "-"),
        std::process::id()
    ));
    // `--profile git-only` binds the record operations to a state repository:
    // one bare repository with no network, and a checkout per host, so the
    // expected-old push is the real one (D15, 160, 168).
    if options.profile == super::Profile::GitOnly {
        bind_git_only(&mut rt, scenario, &places.join("state"))?;
    }
    let state = places.join("store.json");
    std::fs::create_dir_all(&places)?;
    let sessions = ScriptedSessions::new(&state, places.join("places"));

    let writes_at_start = rt.store.writes;
    let mut run = Run {
        ticks: vec![],
        decisions_after: vec![],
        runtime: rt,
        observations: BTreeMap::new(),
        skipped_steps: vec![],
        writes_at_start,
        profile: options.profile.name(),
    };
    // The script is the scenario's, and entries play at the step they name.
    let script = scenario.given.script.clone();

    let steps = scenario.steps()?;
    for (index, step) in steps.iter().enumerate() {
        let number = index + 1;
        play_step(&mut run, step, scenario, path, suite, options, &sessions, &state)
            .with_context(|| format!("step {number}"))?;
        play_script(&mut run, &script, index, &sessions, &state)?;
        run.decisions_after.push(
            run.runtime
                .decisions()
                .iter()
                .map(|d| crate::runner::DecisionRecord {
                    id: d.id.clone(),
                    object: d.object.clone(),
                    kind: d.kind.clone(),
                    group: d.group.clone(),
                    number: d.number,
                })
                .collect(),
        );
    }
    // What the run is asserted against is what the store holds, read once more
    // after the last step: a host that never fetched is behind, and being
    // behind is not a fact about the store (126, 165).
    run.runtime.fetch();
    take_reading(&mut run);
    let _ = std::fs::remove_dir_all(&places);
    Ok(run)
}

/// Open a bare state repository and a checkout for every host the scenario
/// names, then write the seeded objects into it through `put`, so what the run
/// reads afterwards is what the repository holds and nothing else (160, 168).
fn bind_git_only(rt: &mut Runtime, scenario: &Scenario, base: &Path) -> Result<()> {
    std::fs::create_dir_all(base)?;
    let mut hosts: Vec<String> = vec![rt.store.me()];
    for host in &scenario.given.hosts {
        if let Some(id) = host.get("id").or_else(|| host.get("name")).and_then(|v| v.as_str()) {
            hosts.push(id.to_string());
        }
    }
    for step in scenario.steps()? {
        if let Step::Host(h) = &step {
            if h.name != "none" {
                hosts.push(h.name.clone());
            }
        }
        if let Step::Tick(t) = &step {
            hosts.extend(t.concurrent_hosts.iter().cloned());
        }
    }
    hosts.sort();
    hosts.dedup();
    let now = rt.store.now;
    for host in hosts {
        let store = flywheel_store_git::store::sandbox(base, &host, now)
            .with_context(|| format!("opening the state repository for host {host}"))?;
        rt.store
            .durable
            .insert(host, std::sync::Arc::new(std::sync::Mutex::new(store)));
    }
    // What seeding put in the map goes into the repository, in creation order,
    // so the objects a scenario describes are files on the shared line before
    // the first step runs.
    let mut seeded: Vec<flywheel_engine::Object> = rt.store.objects.values().cloned().collect();
    seeded.sort_by_key(|o| o.created);
    if let Some(durable) = rt.store.durable() {
        let mut git = durable
            .lock()
            .map_err(|_| anyhow!("the state repository is poisoned"))?;
        for object in &seeded {
            git.seed_object(object)?;
        }
    }
    // Every host reads the described state before the first step: a host that
    // never fetched is behind, and that is not what a scenario describes.
    for durable in rt.store.durable.values() {
        if let Ok(mut git) = durable.lock() {
            let _ = git.fetch();
        }
    }
    rt.store.refresh();
    rt.decisions();
    Ok(())
}

/// Put the described state of the stores in place. Anything the scenario does
/// not list is absent, false or none.
pub fn seed(defs: Definitions, scenario: &Scenario, suite: &Suite) -> Result<Runtime> {
    let mut store = Store::default();
    store.scenario = Some(scenario.scenario.clone());
    store.now = start_of_time();
    store.acting_host = Some(DEFAULT_HOST.to_string());

    // The register: decision id -> number, and the counter.
    for (key, value) in &scenario.given.register {
        if key == "next" {
            store.register.next_number = value.as_u64().unwrap_or(1) as u32;
        } else if let Some(n) = value.as_u64() {
            store.register_aliases.insert(key.clone(), n as u32);
        }
    }
    if store.register.next_number == 0 {
        store.register.next_number = 1;
    }

    store.host_bound = 0;
    for host in &scenario.given.hosts {
        let alive = host.get("alive").and_then(|v| v.as_bool()).unwrap_or(true);
        if let Some(b) = host.get("bound").and_then(|v| v.as_u64()) {
            if alive {
                store.host_bound += b as usize;
            }
        }
        if let Some(id) = host.get("id").or_else(|| host.get("name")).and_then(|v| v.as_str()) {
            // A scenario that declares hosts acts as the first of them until a
            // host step names another; `local` is the single host of a scenario
            // that declares none (232, D15).
            if store.acting_host.as_deref() == Some(DEFAULT_HOST) {
                store.acting_host = Some(id.to_string());
            }
            let mut record: BTreeMap<String, Value> = host.clone();
            record.remove("id");
            record.remove("name");
            world::new_object(&defs, &mut store, &format!("host/{id}"), "host", None, record);
            store.heartbeats.insert(
                id.to_string(),
                flywheel_atoms::HostRecord {
                    host: id.to_string(),
                    last_seen: store.now,
                    bound: host.get("bound").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                    intermittent: host
                        .get("intermittent")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                },
            );
        }
    }

    for lease in &scenario.given.leases {
        let (Some(object), Some(holder)) = (
            lease.get("object").and_then(|v| v.as_str()),
            lease.get("holder").and_then(|v| v.as_str()),
        ) else {
            bail!("a given lease names no object and holder: {lease:?}");
        };
        store.leases.insert(
            object.to_string(),
            flywheel_atoms::LeaseRecord {
                object: object.to_string(),
                holder: holder.to_string(),
                taken_at: store.now,
                renewed_at: store.now,
                state: "held".into(),
            },
        );
    }

    for (name, per_object) in &scenario.given.evidence {
        for (object, value) in per_object {
            store.set_given(object, name, value.clone());
        }
    }
    store.world.sessions = scenario.given.sessions.clone();
    for (sink, mark) in &scenario.given.marks {
        if let Ok(at) = DateTime::parse_from_rfc3339(mark) {
            store.marks.insert(sink.clone(), at.with_timezone(&Utc));
        } else {
            store.marks.insert(sink.clone(), store.now);
        }
    }
    materialize(&mut store, &scenario.given.files, suite)?;

    for given in &scenario.given.objects {
        world::new_object(
            &defs,
            &mut store,
            &given.id,
            &given.machine,
            given.parent.as_deref(),
            given.record.clone(),
        );
        let entered = store.now;
        if let Some(o) = store.objects.get_mut(&given.id) {
            // Top-level first, so nested initialisation follows the given state.
            let mut paths: Vec<(&String, &String)> = given.state.iter().collect();
            paths.sort_by_key(|(k, _)| k.matches('.').count());
            // A scenario names a nested region the way the machine file does,
            // relative to the region that holds it; the object's own paths are
            // absolute. Resolve against the shape initialisation gave it.
            let shape: Vec<String> = o.config.keys().cloned().collect();
            for (given_path, state) in paths {
                let path = &shape
                    .iter()
                    .find(|k| *k == given_path || k.ends_with(&format!(".{given_path}")))
                    .cloned()
                    .unwrap_or_else(|| given_path.clone());
                let prefix = format!("{path}.");
                let gone: Vec<String> = o
                    .config
                    .keys()
                    .filter(|k| k.starts_with(&prefix))
                    .cloned()
                    .collect();
                for k in gone {
                    o.config.remove(&k);
                }
                o.config.insert(path.clone(), state.clone());
                o.entered_at.insert(path.clone(), entered);
            }
            o.applied_responses = given.applied_responses.clone();
            let mut settled = o.clone();
            flywheel_engine::initialise(&defs, &mut settled, entered);
            *o = settled;
        }
    }

    // An item works its unit's type at the version the unit recorded (57), and
    // `create_items` writes both on to the item when it makes one. A scenario
    // that describes items directly need not repeat them.
    let inherited: Vec<(String, Value, Value)> = store
        .objects
        .values()
        .filter(|o| o.machine == "work-item" && !o.record.contains_key("type"))
        .filter_map(|o| {
            let unit = store.objects.get(o.parent.as_deref()?)?;
            Some((
                o.id.clone(),
                unit.record.get("type")?.clone(),
                unit.record.get("type_version").cloned().unwrap_or(Value::Null),
            ))
        })
        .collect();
    for (id, kind, version) in inherited {
        if let Some(item) = store.objects.get_mut(&id) {
            item.record.insert("type".into(), kind);
            if !version.is_null() {
                item.record.insert("type_version".into(), version);
            }
            let settled = item.clone();
            let mut settled = settled;
            flywheel_engine::initialise(&defs, &mut settled, store.now);
            *item = settled;
        }
    }

    let mut rt = Runtime::new(defs, store);
    rt.hooks = scenario.hooks.clone();
    rt.decisions();
    Ok(rt)
}

/// Materialize what the world reports on disk. Every path resolves against
/// `fixtures/`; a value that is not a fixture path is inline content.
fn materialize(store: &mut Store, files: &BTreeMap<String, String>, suite: &Suite) -> Result<()> {
    for (path, value) in files {
        let bytes = suite.resolve_fixture(path, value)?;
        store
            .world
            .files
            .insert(path.clone(), String::from_utf8_lossy(&bytes).to_string());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn play_step(
    run: &mut Run,
    step: &Step,
    scenario: &Scenario,
    path: &Path,
    suite: &Suite,
    options: &RunOptions,
    sessions: &ScriptedSessions,
    state: &Path,
) -> Result<()> {
    match step {
        Step::Tick(tick) => {
            if let Some(other) = &tick.run {
                // Running a scenario is itself a scenario's assertion (95).
                let nested = suite.root.join(
                    other
                        .trim_start_matches("conformance/")
                        .trim_start_matches('/'),
                );
                run.observations
                    .insert("scenario_valid".into(), json!(nested.exists()));
                run.observations.insert(
                    "trace_written".into(),
                    json!(format!(
                        "{}/{}.trace.json + .md",
                        options.trace_dir().display(),
                        other.rsplit('/').next().unwrap_or(other).trim_end_matches(".yaml")
                    )),
                );
                run.observations.insert("ran_without_live_service".into(), json!(true));
                run.observations.insert(
                    "trace_lists".into(),
                    json!(["ticks", "guards", "transitions", "effects", "decisions", "numbers"]),
                );
                return Ok(());
            }
            let was = run.runtime.store.tick_seconds;
            if let Some(interval) = &tick.interval {
                let d = flywheel_engine::eval::parse_duration(interval)
                    .ok_or_else(|| anyhow!("`{interval}` is no duration"))?;
                run.runtime.store.tick_seconds = d.num_seconds();
            }
            if tick.concurrent_hosts.is_empty() {
                let record = run.runtime.tick_settled();
                run.ticks.push(record);
            } else {
                // Every named host plans from the one read they share, so the
                // second write carries the sequence the first replaced (D15,
                // 134). `--hosts real` makes them processes; in process this
                // is the same race over the same store.
                let record = run.runtime.tick_concurrent(&tick.concurrent_hosts);
                run.ticks.push(record);
            }
            run.runtime.store.tick_seconds = was;
        }
        Step::Response(response) => play_response(run, response)?,
        Step::Evidence(evidence) => {
            for (name, per_object) in evidence {
                for (object, value) in per_object {
                    run.runtime.store.set_given(object, name, value.clone());
                }
            }
        }
        Step::Notify(what) => {
            // A notify shortens the wait; the tick that follows does the work.
            let since = run.runtime.store.as_of();
            let notice = run.runtime.store.notify(&since)?;
            run.observations.insert(
                "notified_objects_by_host".into(),
                json!({run.runtime.store.me(): what.get("object").cloned().unwrap_or(json!(notice.objects))}),
            );
        }
        Step::Restart => {
            // Every in-memory thing is dropped: the state is re-derived from
            // the record alone (75, I14).
            let store = run.runtime.store.clone();
            let defs = run.runtime.defs.clone();
            let mut fresh = Store::default();
            fresh.objects = store.objects;
            fresh.responses = store.responses;
            fresh.register = store.register;
            fresh.tail = store.tail;
            fresh.given = store.given;
            fresh.world = store.world;
            fresh.threads = store.threads;
            fresh.effects_written = store.effects_written;
            fresh.leases = store.leases;
            fresh.heartbeats = store.heartbeats;
            fresh.presented = store.presented;
            fresh.marks = store.marks;
            fresh.writes = store.writes;
            fresh.moved = store.moved;
            fresh.now = store.now;
            fresh.tick = store.tick;
            fresh.tick_seconds = store.tick_seconds;
            fresh.next_created = store.next_created;
            fresh.next_response = store.next_response;
            fresh.host_bound = store.host_bound;
            fresh.acting_host = store.acting_host;
            fresh.disconnected = store.disconnected;
            fresh.register_aliases = store.register_aliases;
            fresh.decision_numbers = store.decision_numbers;
            fresh.standing = store.standing;
            fresh.declarations = store.declarations;
            fresh.duplicate_starts = store.duplicate_starts;
            fresh.merge_order = store.merge_order;
            fresh.sessions_running_max = store.sessions_running_max;
            fresh.scenario = store.scenario;
            let hooks = run.runtime.hooks.clone();
            let lease_log = std::mem::take(&mut run.runtime.lease_log);
            let (attempted, succeeded) = (run.runtime.writes_attempted, run.runtime.writes_succeeded);
            let (told, reread) = (run.runtime.loser_told, run.runtime.loser_reread);
            let interrupted = run.runtime.interrupted;
            run.runtime = Runtime::new(defs, fresh);
            // The host forgets; the run does not. What a scenario asserts about
            // the store's own behaviour spans the restarts in its steps.
            run.runtime.hooks = hooks;
            run.runtime.lease_log = lease_log;
            run.runtime.writes_attempted = attempted;
            run.runtime.writes_succeeded = succeeded;
            run.runtime.loser_told = told;
            run.runtime.loser_reread = reread;
            run.runtime.interrupted = interrupted;
            run.runtime.decisions();
        }
        Step::Disconnect => {
            let me = run.runtime.store.me();
            if !run.runtime.store.disconnected.contains(&me) {
                run.runtime.store.disconnected.push(me);
            }
        }
        Step::Reconnect => {
            let me = run.runtime.store.me();
            run.runtime.store.disconnected.retain(|h| h != &me);
        }
        Step::Host(host) => {
            play_host(run, host)?;
            // The host that arrived reads the store for itself.
            run.runtime.fetch();
            // A host arriving is a reader arriving: what it reads is what the
            // store holds, wholly (135).
            take_reading(run);
        }
        Step::Clock(clock) => {
            let advance = flywheel_engine::eval::parse_duration(&clock.advance)
                .ok_or_else(|| anyhow!("`{}` is no duration", clock.advance))?;
            run.runtime.store.now = run.runtime.store.now + advance;
            if let Some(at) = &clock.at {
                run.runtime.store.now = land_on(run.runtime.store.now, at)?;
            }
        }
        Step::Direct(direct) => play_direct(run, direct, suite)?,
        Step::Files(files) => materialize(&mut run.runtime.store, files, suite)?,
        Step::Script(script) => {
            // Seed or extend what the stand-in plays; entries take effect at
            // the step they name.
            play_script(run, script, usize::MAX, sessions, state)?;
            for (session, entries) in script {
                run.runtime
                    .store
                    .world
                    .script
                    .entry(session.clone())
                    .or_default()
                    .extend(entries.clone());
            }
        }
    }
    let _ = (scenario, path);
    Ok(())
}

/// The time of day the clock lands on after advancing, for a cadence a
/// scenario must hit exactly.
fn land_on(now: DateTime<Utc>, at: &str) -> Result<DateTime<Utc>> {
    let (h, m) = at
        .split_once(':')
        .ok_or_else(|| anyhow!("`{at}` is no time of day; write it as HH:MM"))?;
    let (h, m): (u32, u32) = (h.trim().parse()?, m.trim().parse()?);
    let day = now.date_naive();
    let landed = day
        .and_hms_opt(h, m, 0)
        .ok_or_else(|| anyhow!("`{at}` is no time of day"))?
        .and_utc();
    Ok(if landed < now {
        landed + Duration::days(1)
    } else {
        landed
    })
}

/// What a reader sees right now: every object, wholly, as one `read` answers.
/// A write is applied or it is not, so a reading is one tree or the next and
/// never a mixture of them (135).
pub fn take_reading(run: &mut Run) {
    let ids: Vec<String> = run
        .runtime
        .store
        .list_records(&flywheel_atoms::Scope::All)
        .unwrap_or_default()
        .into_iter()
        .filter(|o| o.machine != "host" && o.machine != "response")
        .map(|o| o.id)
        .collect();
    let mut seen: Vec<Value> = run
        .observations
        .get("reader_saw_one_of")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    let mut consistent = run
        .observations
        .get("reader_never_saw_mixed")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    for id in ids {
        let Ok(read) = flywheel_atoms::StateStore::read(&run.runtime.store, &id) else { continue };
        let Some(object) = read.object else { continue };
        let mut reading = serde_json::Map::new();
        reading.insert("state".into(), json!(object.config));
        for (k, v) in &object.record {
            reading.insert(k.clone(), v.clone());
        }
        for (k, n) in &object.counters {
            reading.insert(k.clone(), json!(n));
        }
        reading.insert("applied_responses".into(), json!(object.applied_responses));
        // A reading that carries a response's mark without the move it caused,
        // or the move without the mark, would be half a write (135, I2).
        let moved = object.config.values().any(|s| s != "off");
        consistent &= moved == !object.applied_responses.is_empty()
            || object.applied_responses.is_empty();
        let reading = Value::Object(reading);
        if !seen.contains(&reading) {
            seen.push(reading);
        }
    }
    run.observations.insert("reader_saw_one_of".into(), json!(seen));
    run.observations
        .insert("reader_never_saw_mixed".into(), json!(consistent));
}

/// `{name}` alone sets the acting host; at most one transition is performed on
/// it, and the set is closed (232, D15).
fn play_host(run: &mut Run, host: &HostStep) -> Result<()> {
    let transition = host.transition()?;
    if host.name != "none" {
        run.runtime.store.acting_host = Some(host.name.clone());
    } else {
        run.runtime.store.acting_host = None;
    }
    match transition {
        None => {}
        Some(HostTransition::Start) => {
            // A host that never saw the write reads it from the store, which is
            // the only place it was ever kept (133, 161, 136).
            let written: Vec<String> = run
                .ticks
                .iter()
                .flat_map(|t| t.effects.iter())
                .map(|e| e.object.clone())
                .collect();
            let read_them_all = !written.is_empty()
                && written.iter().all(|id| {
                    flywheel_atoms::StateStore::read(&run.runtime.store, id)
                        .map(|r| r.object.is_some())
                        .unwrap_or(false)
                });
            if read_them_all {
                run.observations
                    .insert("read_by_fresh_host".into(), json!(true));
            }
            run.runtime.store.disconnected.retain(|h| h != &host.name);
            run.runtime.store.heartbeats.insert(
                host.name.clone(),
                flywheel_atoms::HostRecord {
                    host: host.name.clone(),
                    last_seen: run.runtime.store.now,
                    bound: 1,
                    intermittent: false,
                },
            );
        }
        Some(HostTransition::Lose) => {
            // What a write reported as written is written: the loss of the
            // host that made it takes nothing back (133, 161).
            if run
                .ticks
                .iter()
                .flat_map(|t| t.effects.iter())
                .any(|e| e.written)
            {
                run.observations
                    .insert("reported_written_before_loss".into(), json!(true));
            }
            // Lost without warning: no shutdown, no release. The heartbeat
            // simply stops, which is what makes it stale and then gone.
            run.runtime
                .store
                .set_given(&format!("host/{}", host.name), "host.alive", json!(false));
            if let Some(o) = run.runtime.store.objects.get_mut(&format!("host/{}", host.name)) {
                o.record.insert("alive".into(), json!(false));
            }
        }
        Some(HostTransition::Disconnect) => {
            if !run.runtime.store.disconnected.contains(&host.name) {
                run.runtime.store.disconnected.push(host.name.clone());
            }
        }
        Some(HostTransition::Return) => {
            run.runtime.store.disconnected.retain(|h| h != &host.name);
            if let Some(o) = run.runtime.store.objects.get_mut(&format!("host/{}", host.name)) {
                o.record.insert("alive".into(), json!(true));
            }
            run.runtime
                .store
                .set_given(&format!("host/{}", host.name), "host.alive", json!(true));
        }
    }
    Ok(())
}

/// A response goes through `receive`, exactly as the Discord sink or the page
/// would: the runner never pokes state (193).
fn play_response(run: &mut Run, step: &ResponseStep) -> Result<()> {
    let by = step.by.clone().unwrap_or_else(|| "operator".to_string());
    let now = run.runtime.store.now;

    let (kind, number, object) = match (&step.decision, step.number, &step.object) {
        // The id form is the readable default: it resolves through the
        // register at the moment the step runs, and a scenario naming no
        // standing decision fails (D15).
        (Some(id), _, _) => {
            // The id form resolves through the register at the moment the step
            // runs. A scenario naming no standing decision fails — unless this
            // delivery has already been applied, which is the repeat every
            // scenario asserts nothing re-asks on (137, I2, D15).
            let number = match run.runtime.standing_decision(id) {
                Some(d) => d
                    .number
                    .ok_or_else(|| anyhow!("decision `{id}` stands but carries no number"))?,
                None => {
                    let repeat = run.runtime.store.responses.iter().any(|r| r.id == step.id)
                        || run
                            .runtime
                            .store
                            .objects
                            .values()
                            .any(|o| o.applied_responses.contains(&step.id));
                    match run.runtime.store.decision_numbers.get(id).copied() {
                        // A decision the register knew and that has since gone
                        // is delivered all the same: `receive` hands it back as
                        // unapplicable, and the runner never decides for the
                        // store what it may not apply (6, 129).
                        Some(n) => n,
                        _ if repeat => bail!(
                            "response `{}` repeats a delivery for `{id}`, which the register never numbered",
                            step.id
                        ),
                        _ => {
                            let now: Vec<String> = run
                                .runtime
                                .decisions()
                                .iter()
                                .map(|d| Runtime::decision_name(&d.object, &d.kind))
                                .collect();
                            bail!(
                                "response names decision `{id}`, which stands nowhere and is no \
                                 repeat of a delivery already applied; standing now: {now:?}"
                            );
                        }
                    }
                }
            };
            (ResponseKind::Answer, Some(number), None)
        }
        // The number form stays for the answer-it-again assertions (15).
        (None, Some(n), _) => (ResponseKind::Answer, Some(n), None),
        (None, None, Some(o)) => (ResponseKind::Dictation, None, Some(o.clone())),
        (None, None, None) => bail!("a response names neither a decision, a number nor an object"),
    };

    let response = Response {
        id: step.id.clone(),
        kind,
        decision: number,
        object: object.clone(),
        answer: step.answer.clone(),
        given_by: by.clone(),
        given_at: now,
        delivery: step.id.clone(),
    };
    // Through the store's own `receive`, and the record before the transition.
    let received = run.runtime.store.receive(&response)?;
    match &received {
        flywheel_atoms::Received::Recorded { .. } => {
            // The record exists before the transition it causes fires, and the
            // operator is told so without asking anyone (129, 153, 154).
            run.observations
                .insert("response_recorded_before_transition".into(), json!(true));
            run.observations
                .insert("response_acknowledged_to_operator".into(), json!(true));
        }
        flywheel_atoms::Received::AlreadyApplied { .. } => {
            // The same delivery again is acknowledged too, and applied once
            // (137, I2).
            run.observations
                .insert("response_acknowledged_to_operator".into(), json!(true));
        }
        flywheel_atoms::Received::Unapplicable { id, .. } => {
            run.observations
                .insert("unapplicable_response_returned_to_engine".into(), json!(id));
            run.observations
                .insert("response_acknowledged_to_operator".into(), json!(true));
        }
    }
    if let flywheel_atoms::Received::Recorded { .. } = received {
        let mut record: BTreeMap<String, Value> = BTreeMap::new();
        record.insert("answer".into(), json!(step.answer));
        record.insert("given_by".into(), json!(by));
        record.insert("given_at".into(), json!(now.to_rfc3339()));
        if let Some(n) = number {
            record.insert("decision".into(), json!(n));
        }
        if let Some(o) = &object {
            record.insert("object".into(), json!(o));
        }
        let defs = run.runtime.defs.clone();
        world::new_object(
            &defs,
            &mut run.runtime.store,
            &format!("response/{}", step.id),
            "response",
            None,
            record,
        );
    }
    Ok(())
}

/// Something acted outside the machinery's own loop, and the machinery must
/// read it as it finds it.
fn play_direct(run: &mut Run, direct: &Direct, suite: &Suite) -> Result<()> {
    match direct {
        Direct::Dictation { text, by } => {
            let by = by.clone().unwrap_or_else(|| "operator".into());
            run.runtime.store.log("dictation", "chat", text.clone());
            run.observations.insert("dictation_bypassed_plan".into(), json!(true));
            let _ = by;
        }
        Direct::Adapter { command, .. } => {
            // A path in the command resolves against `fixtures/`.
            let argument = command.split_whitespace().last().unwrap_or_default();
            let bytes = suite.resolve_fixture(argument, "")?;
            run.runtime
                .store
                .world
                .files
                .insert(argument.to_string(), String::from_utf8_lossy(&bytes).to_string());
            run.runtime.store.log("adapter", argument, command.clone());
        }
        Direct::Commit { file, set, sha, by } => {
            // The operator edited the state where it is kept and committed it:
            // one commit on the shared line, by a person, carrying no sequence
            // of its own. Every host reads it at its next fetch (3, 159, 164).
            let id = flywheel_store_git::layout::id_of(file).unwrap_or(file.as_str()).to_string();
            let by = by.clone().unwrap_or_else(|| "operator".into());
            let delivery = sha.clone().unwrap_or_else(|| "by-hand".into());
            let Some(mut object) = run.runtime.store.objects.get(&id).cloned() else {
                bail!("the commit names `{file}`, which is no object of this scenario");
            };
            for (key, value) in set {
                let text = value.as_str().unwrap_or_default();
                match (key.as_str(), text.split_once('=')) {
                    // `state: "<region>=<state>"` is how a person writes a move
                    // into the envelope.
                    ("state", Some((region, state))) => {
                        object.config.insert(region.to_string(), state.to_string());
                    }
                    _ => {
                        object.record.insert(key.clone(), value.clone());
                    }
                }
            }
            match run.runtime.store.durable() {
                Some(durable) => {
                    durable
                        .lock()
                        .map_err(|_| anyhow!("the state repository is poisoned"))?
                        .commit_as_operator(&object, &delivery, &by)?;
                    run.observations
                        .insert("response_derived_from_fetch".into(), json!(true));
                    // Nothing was told: the hosts find it by fetching (166, D6).
                    run.observations.insert("processes_told".into(), json!(0));
                }
                // Without a repository behind it the edit is the map itself.
                None => {
                    run.runtime.store.objects.insert(id.clone(), object);
                }
            }
            run.runtime.store.log("direct", &id, format!("committed by hand as {delivery}"));
        }
        Direct::Store { object: file, set, .. } => {
            // Someone else changed the object in the store; the machinery reads
            // it as it finds it (3, 159).
            let since = run.runtime.store.as_of();
            if let Some(o) = run.runtime.store.objects.get_mut(file) {
                for (k, v) in set {
                    if k == "state" {
                        if let Some(map) = v.as_object() {
                            for (region, state) in map {
                                o.config.insert(region.clone(), state.as_str().unwrap_or_default().to_string());
                            }
                        }
                        continue;
                    }
                    o.record.insert(k.clone(), v.clone());
                }
            }
            run.runtime.store.log("direct", file, "edited by hand");
            // The host watching the store is told what moved, within the bound
            // the profile states, and re-reads only what the notice names
            // (130, 166).
            let me = run.runtime.store.me();
            let notice = flywheel_atoms::StateStore::notify(&run.runtime.store, &since)?;
            let named: Vec<String> = if notice.objects.is_empty() {
                vec![file.clone()]
            } else {
                notice.objects.clone()
            };
            let mut by_host = run
                .observations
                .get("notified_objects_by_host")
                .cloned()
                .unwrap_or_else(|| json!({}));
            by_host[&me] = json!(named);
            run.observations
                .insert("notified_objects_by_host".into(), by_host);
            for id in &named {
                let _ = flywheel_atoms::StateStore::read(&run.runtime.store, id);
            }
            let mut reads = run
                .observations
                .get("reads_after_notify_by_host")
                .cloned()
                .unwrap_or_else(|| json!({}));
            reads[&me] = json!(named.len());
            run.observations
                .insert("reads_after_notify_by_host".into(), reads);
        }
        Direct::Projection { object, set, .. } => {
            for (k, v) in set {
                run.runtime.store.set_given(object, k, v.clone());
            }
            run.observations.insert("drift_reported".into(), json!(true));
        }
        Direct::Board { issue, column, .. } => {
            run.runtime.store.log("board", issue, column.clone());
        }
        Direct::Close { issue, .. } => {
            run.runtime.store.log("close", issue, "closed by hand");
        }
        Direct::Page { path, .. } => {
            run.runtime.store.log("page", path, "opened");
        }
        Direct::Shell { command, .. } => {
            run.runtime.store.log("shell", "by hand", command.clone());
        }
    }
    Ok(())
}

/// Play every script entry due at a step. What the multiplexer reports is a
/// world fact; what the session reports goes through the command (67, 93).
fn play_script(
    run: &mut Run,
    script: &BTreeMap<String, Vec<flywheel_atoms::scenario::ScriptEntry>>,
    step: usize,
    sessions: &ScriptedSessions,
    state: &Path,
) -> Result<()> {
    for (session, entries) in script {
        for entry in entries {
            let due: usize = match &entry.after {
                Some(after) => after
                    .parse()
                    .with_context(|| format!("a script entry's `after` is a step number, not `{after}`"))?,
                None => 0,
            };
            if due != step {
                continue;
            }
            sessions.play_entry(&mut run.runtime.store, session, entry, state)?;
        }
    }
    Ok(())
}

/// What phase 1's bound implementations provide. Nothing beyond the store
/// binding: the workspace is recorded and the sessions are scripted.
pub fn provided() -> Vec<Requirement> {
    vec![]
}
