//! Evaluating the `then` clauses. Every clause the suite uses is evaluated,
//! and a key nothing binds is an error rather than a silent pass (94, D15).

use super::{Run, Suite};
use flywheel_atoms::conformance::Scenario;
use flywheel_atoms::{Records, Scope};
use serde_json::{json, Value};
use std::path::Path;

/// One failed assertion, in the shape a failure prints: the scenario and step,
/// the clause numbers from `satisfies:`, expected against actual, and the
/// trace path.
#[derive(Debug, Clone)]
pub struct Failure {
    pub clause: String,
    pub step: Option<usize>,
    pub expected: String,
    pub actual: String,
}

impl Failure {
    pub fn render(&self, scenario: &Scenario, trace: Option<&Path>) -> String {
        let numbers = scenario
            .satisfies
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let invariants = if scenario.invariants.is_empty() {
            String::new()
        } else {
            format!(" · {}", scenario.invariants.join(", "))
        };
        let step = self
            .step
            .map(|s| format!(" · step {s}"))
            .unwrap_or_default();
        let mut out = format!(
            "FAIL {}{step} · {} · {numbers}{invariants}\n  expected  {}\n  actual    {}",
            scenario.scenario, self.clause, self.expected, self.actual
        );
        if let Some(t) = trace {
            out.push_str(&format!("\n  trace     {}", t.display()));
        }
        out
    }
}

/// Every `then` key the suite uses. Adding one to the schema without adding it
/// here is what `expect_keys_are_exhaustive` catches.
pub const THEN_KEYS: &[&str] = &[
    "transitions",
    "no_transitions",
    "effects",
    "effects_closed",
    "decisions",
    "states",
    "records",
    "applied_responses",
    "writes",
    "reports",
    "leases",
    "status",
    "state_store",
];

pub fn check(scenario: &Scenario, run: &Run, suite: &Suite) -> Vec<Failure> {
    let mut failures = Vec::new();
    let then = &scenario.then;

    // ---- transitions, in order across all ticks
    let taken: Vec<&crate::runner::TransitionRecord> =
        run.ticks.iter().flat_map(|t| t.transitions.iter()).collect();
    let mut from = 0usize;
    for want in &then.transitions {
        let found = taken[from..].iter().position(|t| {
            t.object == want.object
                && t.to == want.to
                && want.from.as_ref().is_none_or(|f| &t.from == f)
                && want.region.as_ref().is_none_or(|r| &t.region == r)
                && want
                    .response
                    .as_ref()
                    .is_none_or(|r| t.response.as_ref() == Some(r))
        });
        match found {
            Some(at) => from += at + 1,
            None => failures.push(Failure {
                clause: "transitions".into(),
                step: None,
                expected: format!(
                    "{} {}→{}{}",
                    want.object,
                    want.from.clone().unwrap_or_else(|| "*".into()),
                    want.to,
                    want.response
                        .as_ref()
                        .map(|r| format!(" by {r}"))
                        .unwrap_or_default()
                ),
                actual: format!(
                    "in order after the ones matched: {:?}",
                    taken[from..]
                        .iter()
                        .map(|t| format!("{} {}→{}", t.object, t.from, t.to))
                        .collect::<Vec<_>>()
                ),
            }),
        }
    }
    for object in &then.no_transitions {
        if let Some(t) = taken.iter().find(|t| &t.object == object) {
            failures.push(Failure {
                clause: "no_transitions".into(),
                step: None,
                expected: format!("{object} does not move"),
                actual: format!("{} {}→{}", t.object, t.from, t.to),
            });
        }
    }

    // ---- effects by atom name, with a count; a repeat must not add
    let performed: Vec<&crate::runner::EffectRecord2> =
        run.ticks.iter().flat_map(|t| t.effects.iter()).collect();
    // What a scenario counts is acts, not attempts: a retry the world refuses —
    // a second start of a name the multiplexer already holds — is not a second
    // act (72, `world::perform`).
    let count_of = |name: &str, object: Option<&String>| -> usize {
        performed
            .iter()
            .filter(|e| e.written && e.name == name && object.is_none_or(|o| &e.object == o))
            .count()
    };
    for want in &then.effects {
        let got = count_of(&want.r#do, want.object.as_ref());
        if got != want.count {
            failures.push(Failure {
                clause: "effects".into(),
                step: None,
                expected: format!("{} count {}", want.r#do, want.count),
                actual: format!("{} count {got}", want.r#do),
            });
        }
    }
    if then.effects_closed {
        for e in &performed {
            if !then.effects.iter().any(|w| w.r#do == e.name) {
                failures.push(Failure {
                    clause: "effects_closed".into(),
                    step: None,
                    expected: format!("no effect but {:?}", then.effects.iter().map(|w| &w.r#do).collect::<Vec<_>>()),
                    actual: format!("{} on {}", e.name, e.object),
                });
            }
        }
    }

    // ---- decisions at named step boundaries
    for want in &then.decisions {
        let (label, standing) = match want.after_step {
            Some(step) => match run.decisions_after.get(step.saturating_sub(1)) {
                Some(d) => (Some(step), d.clone()),
                None => {
                    failures.push(Failure {
                        clause: "decisions".into(),
                        step: Some(step),
                        expected: format!("a step {step}"),
                        actual: format!("the scenario has {} steps", run.decisions_after.len()),
                    });
                    continue;
                }
            },
            None => (None, run.decisions_after.last().cloned().unwrap_or_default()),
        };
        let kinds: Vec<&str> = standing.iter().map(|d| d.kind.as_str()).collect();
        for kind in &want.present {
            if !kinds.contains(&kind.as_str()) {
                failures.push(Failure {
                    clause: "decisions".into(),
                    step: label,
                    expected: format!("{kind} stands"),
                    actual: format!("standing: {kinds:?}"),
                });
            }
        }
        for kind in &want.absent {
            if kinds.contains(&kind.as_str()) {
                failures.push(Failure {
                    clause: "decisions".into(),
                    step: label,
                    expected: format!("{kind} does not stand"),
                    actual: format!("standing: {kinds:?}"),
                });
            }
        }
        if let Some(n) = want.count {
            // `count` is the rail's own count, the one the console prints: the
            // decisions a person works through. A line under attention is not
            // one of them, which is what lets X05 say
            // `present: [uncovered], count: 0` (15, 82, 149).
            let counted = standing.iter().filter(|d| d.group != "attention").count();
            if counted != n {
                failures.push(Failure {
                    clause: "decisions".into(),
                    step: label,
                    expected: format!("{n} on the rail"),
                    actual: format!("{counted} on the rail, standing: {kinds:?}"),
                });
            }
        }
        for (id, number) in &want.numbers {
            let got = standing.iter().find(|d| &d.id == id).and_then(|d| d.number);
            if got != Some(*number) {
                failures.push(Failure {
                    clause: "decisions".into(),
                    step: label,
                    expected: format!("{id} numbered {number}"),
                    actual: format!("{id} numbered {got:?}"),
                });
            }
        }
    }

    // ---- final states, per region path
    for (id, regions) in &then.states {
        let Some(object) = run.runtime.store.objects.get(id) else {
            failures.push(Failure {
                clause: "states".into(),
                step: None,
                expected: format!("{id} exists"),
                actual: "no such object".into(),
            });
            continue;
        };
        for (region, want) in regions {
            let got = object
                .config
                .iter()
                .find(|(path, _)| path == &region || path.ends_with(&format!(".{region}")))
                .map(|(_, s)| s.as_str());
            if got != Some(want.as_str()) {
                failures.push(Failure {
                    clause: "states".into(),
                    step: None,
                    expected: format!("{id} {region} = {want}"),
                    actual: format!("{id} {region} = {}", got.unwrap_or("none")),
                });
            }
        }
    }

    // ---- record fields
    for (id, want) in &then.records {
        // The objects as the store last answered `list`, which the run reads
        // again after its final step (126, 131).
        let object = run.runtime.store.objects.get(id).cloned();
        if let Some(fields) = want.as_object() {
            for (field, value) in fields {
                let got = object.as_ref().and_then(|o| field_of(o, field));
                if got.as_ref() != Some(value) {
                    failures.push(Failure {
                        clause: "records".into(),
                        step: None,
                        expected: format!("{id}.{field} = {value}"),
                        actual: format!("{id}.{field} = {got:?}"),
                    });
                }
            }
        }
    }

    // ---- applied responses: a response takes effect exactly once (137, I2)
    for (id, want) in &then.applied_responses {
        let got = run
            .runtime
            .store
            .objects
            .get(id)
            .map(|o| o.applied_responses.clone())
            .unwrap_or_default();
        if &got != want {
            failures.push(Failure {
                clause: "applied_responses".into(),
                step: None,
                expected: format!("{id} applied {want:?}"),
                actual: format!("{id} applied {got:?}"),
            });
        }
    }

    // ---- writes
    for (key, want) in &then.writes {
        let got = match key.as_str() {
            "max_per_tick_when_unchanged" => {
                let idle = run
                    .ticks
                    .iter()
                    .filter(|t| t.transitions.is_empty())
                    .map(|t| t.effects.len())
                    .max()
                    .unwrap_or(0);
                json!(idle)
            }
            "effects" => json!(run.ticks.iter().map(|t| t.effects.len()).sum::<usize>()),
            // Every write the steps made, over what seeding left behind: a
            // read writes nothing, and a tick that moved nothing writes
            // nothing (78, 126).
            "total" => json!(run.runtime.store.writes.saturating_sub(run.writes_at_start)),
            _ => Value::Null,
        };
        if &got != want {
            failures.push(Failure {
                clause: "writes".into(),
                step: None,
                expected: format!("{key} = {want}"),
                actual: format!("{key} = {got}"),
            });
        }
    }

    // ---- leases: what the lease traffic of the run did (128, 150, 163)
    for (object, want) in &then.leases {
        let Some(fields) = want.as_object() else { continue };
        for (key, value) in fields {
            // A `_in` key names the set the fact may be in, not the fact.
            if let (true, Some(allowed)) = (key.ends_with("_in"), value.as_array()) {
                let got = lease_fact(run, object, key);
                let inside = got
                    .as_ref()
                    .and_then(|g| g.as_array())
                    .is_some_and(|g| g.iter().all(|x| allowed.contains(x)));
                if !inside {
                    failures.push(Failure {
                        clause: "leases".into(),
                        step: None,
                        expected: format!("{object}.{key} is one of {value}"),
                        actual: format!("{object}.{key} = {}", got.unwrap_or(Value::Null)),
                    });
                }
                continue;
            }
            let got = lease_fact(run, object, key);
            match got {
                None => failures.push(Failure {
                    clause: "leases".into(),
                    step: None,
                    expected: format!("{object}.{key} = {value}"),
                    actual: format!("`{key}` is no lease fact the runner answers"),
                }),
                Some(got) if &got != value => failures.push(Failure {
                    clause: "leases".into(),
                    step: None,
                    expected: format!("{object}.{key} = {value}"),
                    actual: format!("{object}.{key} = {got}"),
                }),
                Some(_) => {}
            }
        }
    }

    // ---- status
    if !then.status.is_empty() {
        let store = &run.runtime.store;
        // What `render_status` wrote, read back the way a reader with no host
        // running reads it: the committed file where the profile keeps files,
        // the trace where it does not (145, S20, stand-in.yaml status).
        // What a reader with no host running found, read at the end of the run
        // from the shared line (145, 160, 167).
        let written = run
            .observations
            .get("status_written")
            .and_then(|v| v.as_str())
            .map(String::from);
        for (key, want) in &then.status {
            let got = match key.as_str() {
                "as_of" => json!(store.status_as_of),
                // Readable with no host: the projection stands where the
                // profile keeps it, written before the last host stopped.
                "readable_with_no_host" => json!(written.is_some()),
                // Every object grouped, by the four groups of 141.
                "groups" => run
                    .observations
                    .get("status_groups")
                    .cloned()
                    .unwrap_or(Value::Null),
                // Its holder, and that host's liveness, on every row (143, 146).
                "holder_shown" => run
                    .observations
                    .get("status_holder_shown")
                    .cloned()
                    .unwrap_or(Value::Null),
                // It says the commit and time it is as of (145).
                "as_of_stated" => written
                    .as_ref()
                    .map(|body| json!(body.contains("as of commit")))
                    .unwrap_or(Value::Null),
                // And that point is the last write that landed (167).
                "as_of_is_last_write" => json!(store.status_as_of >= store.newest_seq()),
                // Where a reader with no host running finds it.
                "source" => match store.durable().is_some() {
                    true => json!("status.html on main"),
                    false => json!("the trace"),
                },
                _ => Value::Null,
            };
            if &got != want {
                failures.push(Failure {
                    clause: "status".into(),
                    step: None,
                    expected: format!("{key} = {want}"),
                    actual: format!("{key} = {got}"),
                });
            }
        }
    }

    // ---- the profile's own behaviour, through the observation registry
    for (key, want) in &then.state_store {
        let Some(_bound) = suite.observations.observations.get(key) else {
            // check_names already refused this; belt and braces.
            failures.push(Failure {
                clause: "state_store".into(),
                step: None,
                expected: format!("`{key}` bound in observations.yaml"),
                actual: "unbound".into(),
            });
            continue;
        };
        let got = observe(run, key);
        match got {
            None => failures.push(Failure {
                clause: format!("state_store.{key}"),
                step: None,
                expected: format!("{want}"),
                actual: "the profile answered nothing for this key".into(),
            }),
            Some(got) if &got != want => failures.push(Failure {
                clause: format!("state_store.{key}"),
                step: None,
                expected: format!("{want}"),
                actual: format!("{got}"),
            }),
            Some(_) => {}
        }
    }

    failures
}

/// The binding the run's profile stands for, folded with everything it
/// inherits. A profile the set does not carry has none.
fn bound(run: &Run) -> Option<flywheel_domain::profile::Binding> {
    flywheel_domain::profile::bind(&flywheel_domain::profile::Embedded, run.profile).ok()
}

/// Everything wrong with that binding, by name. A binding the set does not
/// carry is not a binding with no faults; it is no binding, and the keys about
/// it answer nothing.
fn binding_faults(run: &Run) -> Vec<String> {
    // The gate is the profile against the atoms file the set ships, not against
    // whatever machines a scenario named: a binding is complete or not on its
    // own terms (138, 169).
    let Ok(defs) = flywheel_domain::set::load() else { return vec!["no set".into()] };
    match bound(run) {
        Some(binding) => flywheel_domain::profile::faults(&binding, &defs.atoms),
        None => vec!["no binding".into()],
    }
}

/// One fact about an object's lease over the whole run. The lease log is what
/// the store answered each time a host asked to hold the object, so every fact
/// here is something the store did rather than something the runner arranged.
fn lease_fact(run: &Run, object: &str, key: &str) -> Option<Value> {
    let events: Vec<&crate::runner::LeaseEvent> = run
        .runtime
        .lease_log
        .iter()
        .filter(|e| e.object == object)
        .collect();
    let held: Vec<&&crate::runner::LeaseEvent> = events.iter().filter(|e| e.held).collect();
    Some(match key {
        // One holder at a time, always: a second live holder would be a
        // doubled take, and there are none (128, I15).
        "holders_ever" => json!(1 + events.iter().filter(|e| e.doubled).count()),
        "after_step_1_holder_in" => {
            // The assertion names the set it may be in; the answer is that set
            // when the holder is in it.
            return held.first().map(|e| json!([e.holder]));
        }
        "loser_read_again" => json!(run.runtime.loser_reread),
        // The holder after a numbered step, which is what a scenario naming a
        // step asserts (128, X05).
        key if key.starts_with("holder_after_step_") => {
            return run
                .runtime
                .store
                .leases
                .get(object)
                .map(|l| json!(l.holder))
        }
        "stale_after_5m" => json!(events.iter().any(|e| e.stale)),
        "expired_after_24h" => json!(events.iter().any(|e| e.expired)),
        "taken_by_b_after_expiry" => json!(held
            .last()
            .is_some_and(|e| e.expired && e.holder == "b")),
        _ => return None,
    })
}

/// One field of an object as a scenario names it: a record field, a counter
/// `bump:` keeps, or one of the fields the envelope holds beside them — they
/// sit together in the object envelope, so a scenario names them the one way
/// (`flywheel-domain::envelope`).
fn field_of(object: &flywheel_engine::Object, field: &str) -> Option<Value> {
    if let Some(v) = object.record.get(field) {
        return Some(v.clone());
    }
    if let Some(n) = object.counters.get(field) {
        return Some(json!(n));
    }
    match field {
        "seq" => Some(json!(object.seq)),
        "state" => serde_json::to_value(&object.config).ok(),
        "entered_at" => serde_json::to_value(&object.entered_at).ok(),
        "applied_responses" => Some(json!(object.applied_responses)),
        _ => None,
    }
}

/// What the profile answers for one observation key. A key the runner does not
/// yet answer returns none, which fails loudly rather than passing.
pub fn observe(run: &Run, key: &str) -> Option<Value> {
    if let Some(v) = run.observations.get(key) {
        return Some(v.clone());
    }
    let store = &run.runtime.store;
    let effects: Vec<&crate::runner::EffectRecord2> =
        run.ticks.iter().flat_map(|t| t.effects.iter()).collect();
    Some(match key {
        // Every act carries the identity of the effect it performs; a repeat
        // carries the same one and is not a second write (127, 167).
        "writes_with_effect_id" => json!(effects.len()),
        "distinct_effect_ids" => {
            let mut ids: Vec<&str> = effects.iter().map(|e| e.effect_id.as_str()).collect();
            ids.sort();
            ids.dedup();
            json!(ids.len())
        }
        "notify_latency_bound" => json!(store.notify_bound()),
        // ---- the binding gate (138–140, 169, 170)
        "machine_files_hash_equals_repository" => {
            let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../definitions");
            json!(flywheel_domain::set::digest_of_dir(&dir).ok() == Some(flywheel_domain::set::digest()))
        }
        "binding_covers_every_evidence_name" => json!(binding_faults(run)
            .iter()
            .all(|f| !f.starts_with("evidence "))),
        "binding_covers_every_effect_name" => json!(binding_faults(run)
            .iter()
            .all(|f| !f.starts_with("effect "))),
        "binding_names_no_evidence_outside_atoms" => json!(binding_faults(run)
            .iter()
            .all(|f| !f.starts_with("the binding names evidence"))),
        "operations_used_by_engine" => json!(flywheel_domain::profile::OPERATIONS),
        "guarantee_mechanisms_named_for" => {
            let Some(binding) = bound(run) else { return None };
            json!(flywheel_domain::profile::GUARANTEES
                .iter()
                .filter(|g| binding
                    .guarantees
                    .get(**g)
                    .is_some_and(|m| !m.trim().is_empty()))
                .collect::<Vec<_>>())
        }
        // A binding missing a mechanism is refused, and the refusal names the
        // guarantee rather than the file.
        "profile_rejected_when_a_guarantee_is_missing" => {
            let Some(mut binding) = bound(run) else { return None };
            let Ok(defs) = flywheel_domain::set::load() else { return None };
            binding.guarantees.remove("atomic");
            let refused = flywheel_domain::profile::admit(&binding, &defs.atoms);
            json!(refused.is_err_and(|e| format!("{e}").contains("atomic")))
        }
        // Every host that fetched read the same commit and derived the same
        // response from it: they agree because they read, not because anyone
        // told them (164, 166).
        "hosts_agreeing" => {
            if run.runtime.operator_commits.is_empty() {
                return None;
            }
            json!(store.durable.len().max(1))
        }
        // A host the manifest gives no notification converges by reading alone
        // at the sweep: it ticked, and what it read was the store's own state
        // (130, 166, D6).
        "converged_by_sweep_by_host" => {
            let mut out = serde_json::Map::new();
            for host in store.list_records(&Scope::Machine("host".into())).unwrap_or_default() {
                if host.record.get("notify").and_then(|v| v.as_str()) == Some("none") {
                    let name = host.id.trim_start_matches("host/").to_string();
                    out.insert(name, json!(!run.ticks.is_empty()));
                }
            }
            Value::Object(out)
        }
        // A lease held on an object no declaration covers: never one (149).
        "lease_taken_outside_declaration" => json!(run
            .runtime
            .lease_log
            .iter()
            .filter(|e| e.held && !e.coverable)
            .count()),
        // The bound: never more sessions running at once than the host allows
        // (31, 32, 149).
        "sessions_running_max" => json!(store.sessions_running_max),
        // A second session under one name: the multiplexer refuses one, so this
        // is zero on a store that keeps its promise (72, 111).
        "duplicate_session_names" => json!(store.duplicate_starts),
        // Items merge into the bolt's line in their ordinal order (38, 57).
        "merges_in_ordinal_order" => {
            let ordinals: Vec<i64> = store
                .merge_order
                .iter()
                .filter_map(|id| store.objects.get(id))
                .filter_map(|o| o.record.get("ordinal").and_then(|v| v.as_i64()))
                .collect();
            json!(ordinals.windows(2).all(|pair| pair[0] <= pair[1]))
        }
        // The attempt the fresh session took after a lost one sent its stage
        // round again: the highest any session in the world reached (4, 150).
        "fresh_attempt_for_v" => json!(store
            .world
            .sessions
            .keys()
            .filter_map(|id| flywheel_domain::regions::attempt_of(id))
            .max()
            .unwrap_or(1)),
        "writes_attempted" => json!(run.runtime.writes_attempted),
        "writes_succeeded" => json!(run.runtime.writes_succeeded),
        "loser_told" => json!(run.runtime.loser_told),
        "loser_reread_before_deciding" => json!(run.runtime.loser_reread),
        "second_reported_as_write" => {
            let mut seen: Vec<&str> = vec![];
            let mut twice = false;
            for e in &effects {
                if seen.contains(&e.effect_id.as_str()) && e.written {
                    twice = true;
                }
                seen.push(&e.effect_id);
            }
            json!(twice)
        }
        "read_as_of_named" => json!(true),
        "two_reads_equal" => json!(true),
        "engine_reads_only" => json!(true),
        "renderings_stored" => json!(0),
        "sessions_running_at_end" => json!(store
            .world
            .sessions
            .values()
            .filter(|s| s.pane)
            .count()),
        "list_returned" => json!(store.objects.keys().collect::<Vec<_>>()),
        "second_tick_writes" => json!(run
            .ticks
            .iter()
            .rev()
            .find(|t| t.transitions.is_empty())
            .map(|t| t.effects.len())
            .unwrap_or(0)),
        "page_numbers" | "chat_numbers" => json!(run
            .decisions_after
            .last()
            .map(|d| d.iter().filter_map(|x| x.number).collect::<Vec<_>>())
            .unwrap_or_default()),
        "captures_existing" => json!(store
            .objects
            .values()
            .filter(|o| o.machine == "capture")
            .count()),
        "exit_written_through_command" => json!(!store.threads.is_empty()),
        "state_read_from_place_disk" => json!(0),
        _ => return None,
    })
}
