//! Seeding a real host's state store from a scenario's `given:` (94, D15).
//!
//! A scenario file is the described state of an instance at one moment, and
//! until now the only thing that could stand on one was the scenario runner's
//! in-process store. So there was no way to open a served page on a described
//! state: a fresh instance has nothing on it, and the operator could not see
//! the product work. This puts the described objects into the state repository
//! the host already has, through the store's own write path, and a host serving
//! over that store renders that scenario's rail (125, 193, 310).
//!
//! It is a seed and not a second way to make an object: every object goes in
//! through the state store, in one commit, with the sequence the description
//! gave it, exactly as `flywheel scenario run` puts one there under `--hosts
//! real` (D15).

use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use flywheel_atoms::scenario::Scenario;
use flywheel_engine::Object;
use flywheel_store_git::GitStore;
use serde_json::Value;

/// What a seed put in place.
pub struct Seeded {
    /// The objects written, in the order they were created.
    pub objects: usize,
    /// The moment the description is now anchored at.
    pub now: DateTime<Utc>,
    /// The first number the register will give, where the scenario named one.
    pub register_start: Option<u32>,
}

/// Read a scenario and put its `given:` state into a state store.
///
/// The description is moved to `at` first: a scenario states its own `now` and
/// every age in it — `entered: 12d`, a host's `last_seen`, the tail — is
/// relative to that moment. A host's clock is the real one, so a description
/// seeded at its own written date would read as a week stale and its `older:`
/// guards would fire on ages nobody described. Shifting the whole description
/// by one delta keeps every age exactly as written (D15).
pub fn from_scenario(store: &mut GitStore, path: &std::path::Path, at: DateTime<Utc>) -> Result<Seeded> {
    let mut scenario = flywheel_atoms::scenario::load(path)
        .with_context(|| format!("reading the scenario at {}", path.display()))?;
    into_store(store, &mut scenario, at)
}

/// The same over a scenario already read.
pub fn into_store(store: &mut GitStore, scenario: &mut Scenario, at: DateTime<Utc>) -> Result<Seeded> {
    move_to(scenario, at);
    let defs = flywheel_domain::set::load()?;
    // The described state, built the one way it is built anywhere: the
    // scenario's own seeding, which is what every conformance run stands on.
    let runtime = flywheel_scenario::scenario::seed(defs, scenario);
    let mut objects: Vec<Object> = runtime.store.objects.values().cloned().collect();
    objects.sort_by_key(|o| o.created);
    store
        .seed_objects(&objects)
        .context("putting the described objects on the shared line")?;
    if let Some(next) = scenario.given.register_start {
        let mut register = flywheel_domain::commands::register(store)?;
        register.next_number = next;
        flywheel_domain::commands::set_register(store, &register, &[])?;
    }
    Ok(Seeded {
        objects: objects.len(),
        now: at,
        register_start: scenario.given.register_start,
    })
}

/// Move a description's own moment to `at`, carrying every absolute time in it
/// along by the same amount.
fn move_to(scenario: &mut Scenario, at: DateTime<Utc>) {
    let Some(from) = scenario.given.now else {
        scenario.given.now = Some(at);
        return;
    };
    let delta = at - from;
    scenario.given.now = Some(at);
    if delta.is_zero() {
        return;
    }
    for host in &mut scenario.given.hosts {
        for value in host.values_mut() {
            shift(value, delta);
        }
    }
    for per_object in scenario.given.evidence.values_mut() {
        for value in per_object.values_mut() {
            shift(value, delta);
        }
    }
    for entry in &mut scenario.given.tail {
        entry.at += delta;
    }
    for fact in scenario.given.sessions.values_mut() {
        if let Some(idle) = fact.idle_since {
            fact.idle_since = Some(idle + delta);
        }
    }
    for object in &mut scenario.given.objects {
        for value in object.record.values_mut() {
            shift(value, delta);
        }
    }
}

/// Any moment written in a value moves with the description; everything else is
/// left as it stands.
fn shift(value: &mut Value, delta: Duration) {
    match value {
        Value::String(text) => {
            if let Ok(at) = DateTime::parse_from_rfc3339(text) {
                *text = (at.with_timezone(&Utc) + delta).to_rfc3339();
            }
        }
        Value::Array(values) => values.iter_mut().for_each(|v| shift(v, delta)),
        Value::Object(fields) => fields.values_mut().for_each(|v| shift(v, delta)),
        _ => {}
    }
}
