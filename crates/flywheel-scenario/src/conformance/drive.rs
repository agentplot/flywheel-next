//! Seeding a described state of the stores, and playing the `when` steps.
//!
//! The clock is virtual and moves for exactly two reasons: a `clock` step, and
//! each `tick` step by one tick interval. Wall-clock time never reaches a
//! guard, so a run is deterministic and a scenario that waits says so in its
//! own steps (D15).

use super::hosts::RealHosts;
use super::{interpreter, Run, RunOptions, Suite};
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

/// `now` in a seeded value or an evidence step is the run's clock at the
/// moment it is read — the virtual one, so a scenario that says a session went
/// idle now and then advances 31 minutes is saying one thing (D15).
fn at_now(value: &Value, now: DateTime<Utc>) -> Value {
    match value {
        Value::String(text) if text == "now" => json!(now.to_rfc3339()),
        Value::Array(items) => Value::Array(items.iter().map(|v| at_now(v, now)).collect()),
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(name, v)| (name.clone(), at_now(v, now)))
                .collect(),
        ),
        _ => value.clone(),
    }
}

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
        // A scenario's own `machines:` replaces the domain, not the engine:
        // the lease, the rail, the sink, the host and the response are the
        // engine's own machines, and every object is worked through them
        // (model.md §2.5, 86, 87, 128, 148).
        let defs = flywheel_engine::load::load_dir(&resolved)
            .with_context(|| format!("loading the machines at {}", resolved.display()))?;
        return flywheel_domain::set::with_engine(defs)
            .context("folding the engine's own machines into the set");
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

    // One directory per run, not per scenario: two runs of one scenario at
    // once — the suite and a test of it — must not share a working tree.
    static RUNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let places = std::env::temp_dir().join(format!(
        "flywheel-run-{}-{}-{}",
        scenario.scenario.replace('/', "-"),
        std::process::id(),
        RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
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

    // `--hosts real`: every host the scenario names is a process of its own,
    // and from here the runner holds no engine — it drives those processes and
    // reads what they wrote (232, D15).
    let mut real = match options.hosts_real {
        true => Some(start_real(&mut rt, scenario, &places)?),
        false => None,
    };

    let writes_at_start = rt.store.work_writes;
    let mut run = Run {
        ticks: vec![],
        decisions_after: vec![],
        runtime: rt,
        observations: BTreeMap::new(),
        skipped_steps: vec![],
        writes_at_start,
        tail_after: vec![],
        status_after: vec![],
        profile: options.profile.name(),
        dictations: 0,
        engine_ticks: 0,
        read_at: Default::default(),
        offline: Default::default(),
    };
    // The script is the scenario's, and entries play at the step they name.
    let script = scenario.given.script.clone();

    let steps = scenario.steps()?;
    for (index, step) in steps.iter().enumerate() {
        let number = index + 1;
        play_step(
            &mut run,
            step,
            scenario,
            path,
            suite,
            options,
            &sessions,
            &state,
            real.as_mut(),
        )
        .with_context(|| format!("step {number}"))?;
        play_script(&mut run, &script, index, &sessions, &state)?;
        // As a sink that has never delivered reads it: everything that has
        // reached done, landed, closed or dropped since before the run, which
        // is what a scenario's `tail:` names (14).
        let tail = flywheel_engine::rail::tail(
            &run.runtime.defs,
            &run.runtime.store.objects,
            DateTime::<Utc>::from_timestamp_nanos(0),
        );
        run.tail_after.push(tail);
        let reading = status_reading(&run);
        run.status_after.push(reading);
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
    // Every host is told to stop before the run is read: a process still
    // writing is not a state to assert against.
    if let Some(hosts) = &mut real {
        hosts.shutdown();
        // What the run did, in the order it was done. A host whose route was
        // cut wrote at the time it wrote and the shared line learned of it
        // later (161, D4a), so the record is read once more, whole, and the
        // ticks are the times the writes were made rather than the moments the
        // runner could see them.
        read_all_the_hosts_did(&mut run)?;
    }
    // What the run is asserted against is what the store holds, read once more
    // after the last step: a host that never fetched is behind, and being
    // behind is not a fact about the store (126, 165).
    run.runtime.fetch();
    take_reading(&mut run);
    // The status projection as a reader with no host running finds it: the
    // committed file on the shared line, read before the run's own directories
    // go (145, 160, 167, S20). On a profile that keeps no files it is the body
    // the projection wrote, which the trace carries (stand-in.yaml status).
    let committed = match run.runtime.store.durable() {
        Some(durable) => durable.lock().ok().and_then(|mut held| {
            let _ = held.fetch();
            held.committed_status().ok().flatten()
        }),
        None => Some(run.runtime.store.status_body.clone()).filter(|b| !b.is_empty()),
    };
    if let Some(body) = committed {
        // The grouping and the holders as the view shows them, read from the
        // same state the engine reads (132, 141, 143, 146).
        if let Ok(status) = flywheel_domain::status::read(
            &run.runtime.store,
            &run.runtime.defs,
            &flywheel_atoms::ReadPoint {
                mark: format!("write {}", run.runtime.store.writes),
                seq: run.runtime.store.writes,
                at: run.runtime.store.now,
            },
            run.runtime.store.now,
            chrono::Duration::minutes(5),
            chrono::Duration::minutes(30),
        ) {
            let mut groups = serde_json::Map::new();
            for group in flywheel_domain::status::GROUPS {
                let objects = status.group(group);
                if objects.is_empty() {
                    continue;
                }
                groups.insert(group.to_string(), json!(objects));
            }
            run.observations
                .insert("status_groups".into(), Value::Object(groups));
            run.observations.insert(
                "status_holder_shown".into(),
                json!(status.rows.iter().all(|row| body.contains(&format!(
                    "data-holder=\"{}\"",
                    row.holder.as_deref().unwrap_or("none")
                )))),
            );
        }
        run.observations
            .insert("status_written".into(), json!(body));
    }
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

/// The region of a seeded object that a scenario's `state:` key names.
///
/// A scenario names a region the way the machine file does — relative to
/// whatever holds it, and by the singular of a plural region (`session.life`
/// for the `sessions` region's session) or by the phase the region belongs to
/// (`placing.place` for the place prepared while the item was placing). The
/// object's own paths are absolute. This resolves the one against the other by
/// the rule `enter:` already uses, and the state named is what settles it: a
/// region that cannot hold it is not the region meant.
///
/// Nothing, or more than one, gives back nothing, and the caller writes the key
/// as it stands rather than guessing at which region was meant.
fn resolve_given_region(
    defs: &Definitions,
    object: &flywheel_engine::Object,
    shape: &[String],
    given: &str,
    state: &str,
) -> Option<String> {
    let admits = |path: &String| {
        flywheel_engine::tick::state_def(defs, object, path)
            .is_some_and(|(region, _)| region.states.contains_key(state))
    };
    // The absolute path, or one ending in it, is the plain reading.
    let plain = |k: &&String| **k == *given || k.ends_with(&format!(".{given}"));
    if let Some(path) = shape.iter().find(|k| plain(k) && admits(k)) {
        return Some(path.clone());
    }
    let names = |segment: &str, wanted: &str| segment == wanted || segment == format!("{wanted}s");
    let wanted: Vec<&str> = given.split('.').collect();
    // Every segment the scenario named, in order, among the path's own.
    let all_named = |path: &String| {
        let segments: Vec<&str> = path.split('.').collect();
        let mut at = 0;
        wanted.iter().all(|w| {
            match segments[at..].iter().position(|s| names(s, w)) {
                Some(found) => {
                    at += found + 1;
                    true
                }
                None => false,
            }
        })
    };
    // Failing that, the last segment alone: the leading ones name the phase the
    // region belongs to rather than a region of their own.
    let last = *wanted.last()?;
    let by_last = |path: &String| path.split('.').any(|s| names(s, last));

    for rule in [&all_named as &dyn Fn(&String) -> bool, &by_last] {
        let matched: Vec<&String> = shape.iter().filter(|k| admits(k) && rule(k)).collect();
        if let [one] = matched.as_slice() {
            return Some((*one).clone());
        }
    }
    // No region can hold the state named. The plain reading is still the region
    // the scenario meant, and writing it there is what says so.
    shape.iter().find(|k| plain(k)).cloned()
}

/// Enter the one state that would bring a region the scenario named into being.
///
/// Only where exactly one state of exactly one region would: anything less
/// certain is left for the caller to write as it stands, because entering the
/// wrong state would be the harness deciding what the scenario meant.
fn open_a_region(
    defs: &Definitions,
    object: &mut flywheel_engine::Object,
    shape: &[String],
    left: &[(&String, &String)],
    entered: DateTime<Utc>,
) -> bool {
    for (given_path, state) in left {
        let last = given_path.rsplit('.').next().unwrap_or(given_path);
        for region_path in shape {
            let segment = region_path.rsplit('.').next().unwrap_or(region_path);
            if segment != last && segment != format!("{last}s") {
                continue;
            }
            let Some((region, _)) = flywheel_engine::tick::state_def(defs, object, region_path)
            else {
                continue;
            };
            let opens: Vec<String> = region
                .states
                .keys()
                .filter(|candidate| {
                    let mut trial = object.clone();
                    set_region(&mut trial, region_path, candidate, entered);
                    flywheel_engine::initialise(defs, &mut trial, entered);
                    let opened: Vec<String> = trial.config.keys().cloned().collect();
                    resolve_given_region(defs, &trial, &opened, given_path, state).is_some()
                })
                .cloned()
                .collect();
            if let [one] = opens.as_slice() {
                set_region(object, region_path, one, entered);
                flywheel_engine::initialise(defs, object, entered);
                return true;
            }
        }
    }
    false
}

/// Put one region of a seeded object in the state the scenario named, dropping
/// whatever was nested under the state it leaves.
fn set_region(
    object: &mut flywheel_engine::Object,
    path: &str,
    state: &str,
    entered: DateTime<Utc>,
) {
    let prefix = format!("{path}.");
    let gone: Vec<String> = object
        .config
        .keys()
        .filter(|k| k.starts_with(&prefix))
        .cloned()
        .collect();
    for k in gone {
        object.config.remove(&k);
    }
    object.config.insert(path.to_string(), state.to_string());
    object.entered_at.insert(path.to_string(), entered);
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
            // `now` in a host's record is the run's clock, like anywhere else:
            // a host seeded as last seen now and then left for six minutes is
            // saying one thing (D15).
            let now = store.now;
            let mut record: BTreeMap<String, Value> = host
                .iter()
                .map(|(name, value)| (name.clone(), at_now(value, now)))
                .collect();
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
            let value = at_now(value, store.now);
            store.set_given(object, name, value);
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
        let now = store.now;
        let record = given
            .record
            .iter()
            .map(|(name, value)| (name.clone(), at_now(value, now)))
            .collect();
        world::new_object(
            &defs,
            &mut store,
            &given.id,
            &given.machine,
            given.parent.as_deref(),
            record,
        );
    }

    // An item works its unit's type at the version the unit recorded (57), and
    // `create_items` writes both on to the item when it makes one. A scenario
    // that describes items directly need not repeat them. This is settled
    // before the states are, because the type is what says which regions the
    // item has: a scenario naming a stage names a region of the type's machine,
    // and there is none until the item knows its type.
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
            let mut settled = item.clone();
            flywheel_engine::initialise(&defs, &mut settled, store.now);
            *item = settled;
        }
    }

    for given in &scenario.given.objects {
        let entered = store.now;
        if let Some(o) = store.objects.get_mut(&given.id) {
            // A scenario names a nested region the way the machine file does,
            // relative to the region that holds it; the object's own paths are
            // absolute. Resolve each against the shape the object has, then
            // initialise, because setting a region is what brings the regions
            // nested under it into being: `in-type.stages` is a path only once
            // `life` holds `in-type`. The given state is a map, so the order it
            // was written in is gone; take whichever paths the shape can place
            // this round and go round again until none can. Anything left over
            // names no region of this machine and is written as it stands, the
            // way it always was.
            let mut described: Vec<String> = Vec::new();
            let mut left: Vec<(&String, &String)> = given.state.iter().collect();
            while !left.is_empty() {
                let shape: Vec<String> = o.config.keys().cloned().collect();
                // The outermost region the shape can place goes first, and the
                // shape is read again after it: setting a region drops whatever
                // was nested under the state it left, so a child placed before
                // its parent would be thrown away with it.
                let next = left
                    .iter()
                    .enumerate()
                    .filter_map(|(at, (given_path, state))| {
                        resolve_given_region(&defs, o, &shape, given_path, state)
                            .map(|path| (path.matches('.').count(), path, at))
                    })
                    .min();
                let Some((_, path, at)) = next else {
                    // No region the object has can hold what is left. A region
                    // that names a submachine has to be entered before the
                    // submachine's own regions exist: a scenario naming
                    // `placing.place` names the place prepared while the item
                    // was placing, which is a region of the place machine.
                    // Enter the state that opens it and go round again.
                    if open_a_region(&defs, o, &shape, &left, entered) {
                        continue;
                    }
                    break;
                };
                let (_, state) = left.remove(at);
                described.push(path.clone());
                set_region(o, &path, state, entered);
                let mut settled = o.clone();
                flywheel_engine::initialise(&defs, &mut settled, entered);
                *o = settled;
            }
            for (given_path, state) in left {
                described.push(given_path.clone());
                set_region(o, given_path, state, entered);
            }
            o.applied_responses = given.applied_responses.clone();
            let mut settled = o.clone();
            flywheel_engine::initialise(&defs, &mut settled, entered);
            *o = settled;
            store.described.insert(given.id.clone(), described);
        }
    }

    // A described state includes the world under it: an object seeded with its
    // own place standing has a place, and the effects that act on one — a
    // removal above all — have something to act on. Only where the scenario
    // said nothing about it; what it did say stands (D15).
    let standing: Vec<(String, String)> = store
        .objects
        .values()
        .flat_map(|o| {
            o.config
                .iter()
                .filter(|(path, state)| {
                    path.starts_with("place")
                        && path.ends_with(".life")
                        && !matches!(state.as_str(), "absent" | "removed" | "none")
                })
                .map(|(path, _)| (o.id.clone(), path.clone()))
        })
        .collect();
    for (id, region) in standing {
        let key = flywheel_domain::regions::place_key(&id, &region);
        store.world.places.entry(key).or_insert(crate::store::PlaceFact {
            exists: true,
            ..Default::default()
        });
    }

    // And the sessions under it: an object seeded with a session alive has a
    // session in the world, with the pane and the activity the state it was
    // seeded in implies. Only where the scenario said nothing about it; what
    // it did say stands (D15).
    let alive: Vec<(String, String, String)> = store
        .objects
        .values()
        .flat_map(|o| {
            o.config
                .iter()
                .filter(|(path, state)| {
                    // A stage's region is `sessions`, a type's is `session`;
                    // both name one session at a time (`stage.yaml`,
                    // `session.yaml`).
                    path.split('.')
                        .any(|segment| segment == "session" || segment == "sessions")
                        && path.ends_with(".life")
                        && state.as_str() == "alive"
                })
                .map(|(path, _)| {
                    // The activity region beside it says what the session is
                    // doing; working is what a session with none is doing.
                    let activity = path
                        .strip_suffix(".life")
                        .map(|stem| format!("{stem}.life.alive.activity"))
                        .and_then(|region| o.config.get(&region).cloned())
                        .unwrap_or_else(|| "working".to_string());
                    (o.id.clone(), path.clone(), activity)
                })
        })
        .collect();
    for (id, region, activity) in alive {
        let object = store.objects.get(&id).cloned();
        let key = match &object {
            Some(object) => flywheel_domain::regions::session_key_of_in(&defs, object, &region),
            None => flywheel_domain::regions::session_key(&id, &region),
        };
        store
            .world
            .sessions
            .entry(key)
            .or_insert(crate::store::SessionFact {
                pane: true,
                activity,
                ..Default::default()
            });
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

/// The status view as it stands, for the assertions a scenario makes about what
/// it showed after a numbered step (141, 143, 146).
///
/// It is read the way any reader reads it — from `list` and `read` alone — so
/// what a scenario asserts is what a person would have seen (132, D12).
fn status_reading(run: &Run) -> Value {
    let store = &run.runtime.store;
    let Ok(status) = flywheel_domain::status::read(
        store,
        &run.runtime.defs,
        &flywheel_atoms::ReadPoint {
            mark: format!("write {}", store.writes),
            seq: store.writes,
            at: store.now,
        },
        store.now,
        Duration::minutes(5),
        Duration::minutes(30),
    ) else {
        return json!({});
    };
    // What the view shows as stale: the work whose holder has stopped renewing,
    // and the host itself (146, 150).
    let mut stale: Vec<String> = status
        .rows
        .iter()
        .filter(|row| row.liveness.as_deref() == Some("stale"))
        .map(|row| row.object.clone())
        .collect();
    for host in flywheel_atoms::Records::hosts(store).unwrap_or_default() {
        let since = store.now - host.last_seen;
        if since >= Duration::minutes(5) && since < Duration::minutes(30) {
            stale.push(host.host.clone());
        }
    }
    json!({ "stale": stale })
}

/// Every session a seeded object described as alive: the object, the region
/// that holds it and the session's name. The same reading the stand-in world
/// makes when it puts a pane behind a seeded `session.life: alive`.
fn sessions_described(rt: &Runtime) -> Vec<(String, String, String)> {
    let defs = rt.defs.clone();
    rt.store
        .objects
        .values()
        .flat_map(|o| {
            o.config
                .iter()
                .filter(|(path, state)| {
                    path.split('.')
                        .any(|segment| segment == "session" || segment == "sessions")
                        && path.ends_with(".life")
                        && state.as_str() == "alive"
                })
                .map(|(path, _)| {
                    (
                        o.id.clone(),
                        path.clone(),
                        flywheel_domain::regions::session_key_of_in(&defs, o, path),
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// The hosts a scenario names, in the order it named them; the single host
/// `local` where it names none (232, D15).
pub fn host_names(scenario: &Scenario) -> Vec<String> {
    let mut names: Vec<String> = scenario
        .given
        .hosts
        .iter()
        .filter_map(|h| {
            h.get("id")
                .or_else(|| h.get("name"))
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .collect();
    for step in scenario.steps().unwrap_or_default() {
        match &step {
            Step::Host(h) if h.name != "none" && !names.contains(&h.name) => {
                names.push(h.name.clone())
            }
            Step::Tick(t) => {
                for name in &t.concurrent_hosts {
                    if !names.contains(name) {
                        names.push(name.clone());
                    }
                }
            }
            _ => {}
        }
    }
    if names.is_empty() {
        names.push(DEFAULT_HOST.to_string());
    }
    names
}

/// Put the described state where a process can read it, and start every host
/// the scenario names as one (232, D15).
///
/// The leases and the world's answers go on to the shared line rather than into
/// this process's memory, because the hosts that read them are other processes:
/// a described state a child cannot read is no described state at all.
fn start_real(rt: &mut Runtime, scenario: &Scenario, places: &Path) -> Result<RealHosts> {
    let now = rt.store.now;
    if let Some(durable) = rt.store.durable() {
        let mut git = durable
            .lock()
            .map_err(|_| anyhow!("the state repository is poisoned"))?;
        for (name, per_object) in &scenario.given.evidence {
            for (object, value) in per_object {
                git.commit_given(object, name, &at_now(value, now))?;
            }
        }
        for (object, held) in &rt.store.leases {
            git.lease(&flywheel_atoms::LeaseOp::Take {
                object: object.clone(),
                holder: held.holder.clone(),
            })?;
            // A lease the scenario described as held is held: taking one puts
            // its machine at `free`, and a lease seeded held has already been
            // read as held by the host that holds it (D5, 128).
            git.lease(&flywheel_atoms::LeaseOp::Mark {
                object: object.clone(),
                state: held.state.clone(),
            })?;
        }
        // A described state includes the sessions under it: an object seeded
        // with a session alive has one, recorded against the host holding its
        // lease, because that is the host running it (93b, 147).
        for (object, region, session) in sessions_described(rt) {
            let host = rt
                .store
                .leases
                .get(&object)
                .map(|l| l.holder.clone())
                .unwrap_or_else(|| rt.store.me());
            flywheel_sessions_operator::start(
                &mut *git,
                &host,
                now,
                &flywheel_atoms::WorkOrder {
                    session: session.clone(),
                    kind: "work".into(),
                    place: flywheel_domain::regions::place_key(&object, &region),
                    body: String::new(),
                },
            )?;
        }
    }
    // What the scenario said each host is, as manifest entries: a host is what
    // the manifest says it is, and the harness decides nothing (149, 150a, 183).
    let described: BTreeMap<String, &BTreeMap<String, Value>> = scenario
        .given
        .hosts
        .iter()
        .filter_map(|h| {
            let id = h
                .get("id")
                .or_else(|| h.get("name"))
                .and_then(|v| v.as_str())?;
            Some((id.to_string(), h))
        })
        .collect();
    let specs: Vec<super::hosts::HostSpec> = host_names(scenario)
        .into_iter()
        .map(|name| {
            let mut spec = super::hosts::HostSpec::named(&name);
            if let Some(given) = described.get(&name) {
                if let Some(bound) = given.get("bound").and_then(|v| v.as_u64()) {
                    spec.bound = bound as u32;
                }
                if let Some(laptop) = given.get("intermittent").and_then(|v| v.as_bool()) {
                    spec.intermittent = laptop;
                }
                if let Some(covers) = given.get("covers").and_then(|v| v.as_array()) {
                    spec.covers = covers
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();
                }
            }
            spec
        })
        .collect();
    // Every repository the scenario's objects name is one the instance tracks;
    // a host takes leases only within what it declares, and a repository the
    // manifest does not name is covered by nobody (149, 199, 205).
    let mut repositories: Vec<String> = rt
        .store
        .objects
        .values()
        .filter_map(|o| o.record.get("repository").and_then(|v| v.as_str()))
        .map(String::from)
        .collect();
    repositories.sort();
    repositories.dedup();
    let names: Vec<String> = specs.iter().map(|s| s.name.clone()).collect();
    let mut hosts = RealHosts::open(places, "conformance", &specs, &repositories, now)?;
    // A scenario's hosts are running when it begins; a `start` step starts one
    // it named and has not.
    for name in &names {
        hosts.start(name)?;
    }
    hosts.set_clock(now)?;
    Ok(hosts)
}

/// What the hosts wrote since the last reading, as the ticks a scenario is
/// asserted against. The runner holds no engine under `--hosts real`: every
/// transition and every effect it knows about was read from the run record on
/// the shared line (79, 167, D15).
fn read_what_the_hosts_did(run: &mut Run) -> Result<()> {
    let now = run.runtime.store.now;
    let by_file = match run.runtime.store.durable() {
        Some(durable) => {
            let mut git = durable
                .lock()
                .map_err(|_| anyhow!("the state repository is poisoned"))?;
            git.now = now;
            git.fetch()?;
            git.run_records_by_file()?
        }
        None => vec![],
    };
    let mut fresh: Vec<flywheel_domain::records::RunEntry> = Vec::new();
    for (file, entries) in by_file {
        let mark = run.read_at.entry(file).or_insert(0);
        fresh.extend(entries[(*mark).min(entries.len())..].iter().cloned());
        *mark = entries.len();
    }
    fresh.sort_by(|a, b| a.at.cmp(&b.at));
    let mut record = crate::runner::TickRecord {
        tick: run.ticks.len() as u64 + 1,
        at: now,
        guards: vec![],
        transitions: vec![],
        effects: vec![],
        decisions: vec![],
    };
    for entry in &fresh {
        match entry.kind.as_str() {
            "write" => record.transitions.push(as_transition(entry)),
            "effect" => record.effects.push(as_effect(entry)),
            _ => {}
        }
    }
    run.ticks.push(record);
    // What the runner reads is what the store holds, wholly (135).
    run.runtime.store.refresh();
    run.runtime.decisions();
    Ok(())
}

/// This host's route was cut, at the run's clock (151, D4a).
fn cut(run: &mut Run, host: &str) {
    let now = run.runtime.store.now;
    run.offline.entry(host.to_string()).or_default().push((now, None));
}

/// And came back.
fn restored(run: &mut Run, host: &str) {
    let now = run.runtime.store.now;
    if let Some(window) = run.offline.get_mut(host).and_then(|w| w.last_mut()) {
        if window.1.is_none() {
            window.1 = Some(now);
        }
    }
}

/// One run-record entry as the tick record holds it.
fn as_transition(entry: &flywheel_domain::records::RunEntry) -> crate::runner::TransitionRecord {
    let field = |name: &str| {
        entry
            .fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    };
    crate::runner::TransitionRecord {
        object: entry.object.clone(),
        region: field("region").unwrap_or_default(),
        from: field("from").unwrap_or_default(),
        to: field("to").unwrap_or_default(),
        response: field("response").filter(|r| !r.is_empty()),
        reason: Some(entry.reason.clone()),
        host: Some(entry.host.clone()),
    }
}

fn as_effect(entry: &flywheel_domain::records::RunEntry) -> crate::runner::EffectRecord2 {
    crate::runner::EffectRecord2 {
        effect_id: format!("{}/{}", entry.object, entry.reason),
        name: entry.reason.clone(),
        object: entry.object.clone(),
        args: BTreeMap::new(),
        written: entry
            .fields
            .iter()
            .any(|(n, v)| n == "performed" && v == "true"),
        recalled: false,
    }
}

/// The whole run record, as the ticks it describes.
///
/// One tick is one moment: every write a host made at one point on the virtual
/// clock belongs to the same tick, whichever host made it and whenever the
/// shared line learned of it. That is what puts a disconnected host's writes
/// where they happened rather than where they landed (161, D4a, D15).
fn read_all_the_hosts_did(run: &mut Run) -> Result<()> {
    let now = run.runtime.store.now;
    let entries = match run.runtime.store.durable() {
        Some(durable) => {
            let mut git = durable
                .lock()
                .map_err(|_| anyhow!("the state repository is poisoned"))?;
            git.now = now;
            git.fetch()?;
            git.all_run_records()?
        }
        None => return Ok(()),
    };
    let mut ticks: Vec<crate::runner::TickRecord> = Vec::new();
    for entry in &entries {
        if !matches!(entry.kind.as_str(), "write" | "effect") {
            continue;
        }
        let tick = match ticks.last_mut().filter(|t| t.at == entry.at) {
            Some(held) => held,
            None => {
                ticks.push(crate::runner::TickRecord {
                    tick: ticks.len() as u64 + 1,
                    at: entry.at,
                    guards: vec![],
                    transitions: vec![],
                    effects: vec![],
                    decisions: vec![],
                });
                ticks.last_mut().expect("the tick just pushed")
            }
        };
        match entry.kind.as_str() {
            "write" => tick.transitions.push(as_transition(entry)),
            _ => tick.effects.push(as_effect(entry)),
        }
    }
    run.ticks = ticks;
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
    real: Option<&mut RealHosts>,
) -> Result<()> {
    if let Some(hosts) = real {
        return play_step_real(run, step, suite, options, hosts);
    }
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
            run.engine_ticks += 1;
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
            // A response for an operation no tool carries stands under
            // attention until a tick has reported it once (4, 6, 129).
            if run
                .runtime
                .store
                .objects
                .values()
                .any(|o| o.machine == "response" && o.top_state().as_deref() == Some("unapplicable"))
            {
                run.observations
                    .insert("dictation_asserting_done_reported".into(), json!(true));
            }
        }
        Step::Response(response) => play_response(run, response)?,
        Step::Evidence(evidence) => {
            for (name, per_object) in evidence {
                for (object, value) in per_object {
                    // `*` says this is now true of every object, so what an
                    // earlier step said about one of them by name no longer
                    // stands: the world moved, and it moved for all of them
                    // (S23). What a step says about one object by name joins
                    // what stands about the rest: a world that changed for one
                    // did not change for its siblings
                    // (`contract/present-receive.yaml`).
                    if object == "*" {
                        for per in run.runtime.store.given.values_mut() {
                            per.remove(name);
                        }
                    }
                    let now = run.runtime.store.now;
                    let value = at_now(value, now);
                    run.runtime.store.set_given(object, name, value);
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

/// One step, with every host a process of its own.
///
/// The runner drives those processes and reads the store; it holds no engine
/// and ticks nothing (D15). What a step means is the same as in process — the
/// difference is who performs it.
fn play_step_real(
    run: &mut Run,
    step: &Step,
    suite: &Suite,
    options: &RunOptions,
    hosts: &mut RealHosts,
) -> Result<()> {
    match step {
        Step::Tick(tick) => {
            let interval = match &tick.interval {
                Some(interval) => flywheel_engine::eval::parse_duration(interval)
                    .ok_or_else(|| anyhow!("`{interval}` is no duration"))?,
                None => options.interval,
            };
            hosts.set_clock(run.runtime.store.now)?;
            // Every named host sweeps from the one read they share, so the
            // second write meets the head the first left (134, D15). With none
            // named it is the acting host's tick: a scenario says which host a
            // step runs as, and says so precisely because which host acted is
            // what it is asserting (232, `schema.json` step.host).
            match tick.concurrent_hosts.is_empty() {
                true => hosts.sweep(&[run.runtime.store.me()])?,
                false => hosts.sweep_concurrently(&tick.concurrent_hosts)?,
            };
            run.runtime.store.now = run.runtime.store.now + interval;
            run.runtime.store.tick += 1;
            read_what_the_hosts_did(run)?;
        }
        Step::Clock(clock) => {
            let advance = flywheel_engine::eval::parse_duration(&clock.advance)
                .ok_or_else(|| anyhow!("`{}` is no duration", clock.advance))?;
            run.runtime.store.now = run.runtime.store.now + advance;
            if let Some(at) = &clock.at {
                run.runtime.store.now = land_on(run.runtime.store.now, at)?;
            }
            hosts.set_clock(run.runtime.store.now)?;
        }
        Step::Host(host) => {
            let transition = host.transition()?;
            if host.name != "none" {
                run.runtime.store.acting_host = Some(host.name.clone());
            }
            match transition {
                None => {}
                Some(HostTransition::Start) => hosts.start(&host.name)?,
                // Stopped without a farewell: no shutdown, no release. The
                // heartbeat simply stops (S13, 150).
                Some(HostTransition::Lose) => hosts.lose(&host.name)?,
                Some(HostTransition::Disconnect) => {
                    hosts.disconnect(&host.name)?;
                    cut(run, &host.name);
                }
                Some(HostTransition::Return) => {
                    hosts.returned(&host.name)?;
                    restored(run, &host.name);
                }
            }
            run.runtime.fetch();
            take_reading(run);
        }
        Step::Disconnect => {
            let me = run.runtime.store.me();
            hosts.disconnect(&me)?;
            cut(run, &me);
        }
        Step::Reconnect => {
            let me = run.runtime.store.me();
            hosts.reconnect(&me)?;
            restored(run, &me);
        }
        // The world changed, and every host reads it from the shared line (B.3).
        Step::Evidence(evidence) => {
            let now = run.runtime.store.now;
            for (name, per_object) in evidence {
                for (object, value) in per_object {
                    let value = at_now(value, now);
                    run.runtime.store.set_given(object, name, value.clone());
                    if let Some(durable) = run.runtime.store.durable() {
                        durable
                            .lock()
                            .map_err(|_| anyhow!("the state repository is poisoned"))?
                            .commit_given(object, name, &value)?;
                    }
                    // And told to every host that is running, so one whose
                    // route is cut learns what happened on its own machine
                    // (B.3, 151).
                    hosts.tell_given(object, name, &value)?;
                }
            }
        }
        // A restart drops what a process holds; under `--hosts real` that is
        // the process itself, started again on the state it left (75, I14).
        Step::Restart => {
            for name in hosts.started() {
                hosts.lose(&name)?;
                hosts.start(&name)?;
            }
            run.runtime.fetch();
        }
        Step::Response(response) => {
            play_response(run, response)?;
            // The record reaches the hosts the way the page's control and the
            // sink's reply grammar leave it: on the shared line (129, 193).
            if let Some(durable) = run.runtime.store.durable() {
                let mut git = durable
                    .lock()
                    .map_err(|_| anyhow!("the state repository is poisoned"))?;
                let last = run
                    .runtime
                    .store
                    .responses
                    .last()
                    .cloned()
                    .ok_or_else(|| anyhow!("a response was received and none was recorded"))?;
                git.receive(&last)?;
            }
        }
        Step::Files(files) => materialize(&mut run.runtime.store, files, suite)?,
        // A notify only shortens the wait; the tick that follows does the work
        // (130). Under `--hosts real` the hosts poll the shared line for
        // themselves, so there is nothing here to tell them.
        Step::Notify(_) => {}
        Step::Direct(direct) => play_direct(run, direct, suite)?,
        Step::Script(script) => {
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
                .filter(|e| !machinery(&run.runtime.store, &e.object))
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
            // The operator said this in chat. The host's agent proposes one
            // call of the catalogue and the operator's confirmation is the
            // response; the runner stands in for the agent, and the call it
            // makes is the same tool the page's control calls (12, 193, 194).
            let by = by.clone().unwrap_or_else(|| "operator".into());
            run.dictations += 1;
            let delivery = format!("dictation-{}", run.dictations);
            let objects = run.runtime.store.objects.clone();
            let call = interpreter::propose(text, &by, "discord", &format!("discord/{delivery}"), &objects)
                .with_context(|| format!("the dictation `{text}`"))?;
            run.runtime.store.log("dictation", "chat", text.clone());
            let defs = run.runtime.defs.clone();
            let outcome = crate::bindings::with_files(&mut run.runtime.store, |store, world| {
                flywheel_surface::catalogue::call(store, world, &defs, &call)
            })?;
            // A dictation is applied, never proposed: it raises no decision and
            // never enters the rail (12).
            run.observations.insert("dictation_bypassed_plan".into(), json!(true));
            run.observations.insert("decisions_created_by_dictation".into(), json!(0));
            if !flywheel_domain::commands::is_operation(&call.tool) {
                // No such tool exists, so the claim was applied by nothing; the
                // response machine reports it under attention (4, 6).
                run.observations
                    .insert("dictation_asserting_done_applied".into(), json!(false));
            }
            run.runtime.store.log("response", &outcome.id, format!("{} by {by}", call.tool));
        }
        Direct::Adapter { command, by } => {
            // The adapter's own binary, run by hand from any machine (217g). A
            // path in the command resolves against `fixtures/`; the material
            // stays where it is and the capture cites it (111).
            let argument = command.split_whitespace().last().unwrap_or_default();
            let bytes = suite.resolve_fixture(argument, "")?;
            run.runtime
                .store
                .world
                .raw
                .insert(argument.to_string(), String::from_utf8_lossy(&bytes).to_string());
            let by = by.clone().unwrap_or_else(|| "operator".into());
            let defs = run.runtime.defs.clone();
            let at = run.runtime.store.now;
            let before = written_under(&run.runtime.store, &flywheel_domain::signals::UNDER);
            let enumerated = crate::bindings::with_files(&mut run.runtime.store, |store, world| {
                flywheel_domain::adapters::run(store, world, &defs, command, &by, at)
            })
            .with_context(|| format!("the adapter `{command}`"))?;
            let after = written_under(&run.runtime.store, &flywheel_domain::signals::UNDER);
            // A repeat import writes nothing, which is what S22 counts (111).
            run.observations.insert(
                "capture_files_written_on_second_import".into(),
                json!(enumerated.captures_written),
            );
            run.observations.insert(
                "signal_files_written_on_second_import".into(),
                json!(after.saturating_sub(before)),
            );
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
            // A projection is written from the state it projects and is never
            // read back as state (77, 132, 142). What drifts is the written
            // view, so the drift is written there: the machinery finds the
            // projection no longer as-of the state it projects, reports it and
            // writes it again from the source.
            let drift: Vec<String> = set
                .iter()
                .map(|(field, value)| format!("{object}\t{field}\t{value}"))
                .collect();
            run.runtime.store.drift_projection(&drift.join("\n"));
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
            // The operator ran this outside every tool, so no response exists
            // to read it as: a session ended by hand is a session gone (4, 66).
            let before = run.runtime.store.responses.len();
            run.runtime.store.log("shell", "by hand", command.clone());
            run.observations.insert(
                "hand_killed_pane_read_as_response".into(),
                json!(run.runtime.store.responses.len() > before),
            );
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

/// Whether an object is the machinery's own — the rail, a sink, a host, a
/// lease — rather than the work a scenario is about. What a scenario counts as
/// a write, closes over as an effect or proves a fresh host can read is the
/// work's; the machinery ticks alongside it either way (D5, 167).
pub fn machinery(store: &crate::store::Store, object: &str) -> bool {
    match flywheel_atoms::Records::get(store, object) {
        Ok(Some(held)) => flywheel_domain::leases::machinery(&held.machine),
        _ => object == flywheel_domain::RAIL,
    }
}

/// What phase 1's bound implementations provide. Nothing beyond the store
/// binding: the workspace is recorded and the sessions are scripted.
pub fn provided() -> Vec<Requirement> {
    vec![]
}


/// How many files the blueprints hold under a path, for the counts an import
/// asserts (111).
fn written_under(store: &crate::store::Store, under: &str) -> usize {
    store
        .world
        .files
        .keys()
        .filter(|path| path.starts_with(under))
        .count()
}
