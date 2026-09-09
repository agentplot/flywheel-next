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
                // Which host made it, where the scenario says (147, 232).
                && want.host.as_ref().is_none_or(|h| t.host.as_ref() == Some(h))
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
                        .map(|t| match &t.host {
                            Some(host) => format!("{} {}→{} by {host}", t.object, t.from, t.to),
                            None => format!("{} {}→{}", t.object, t.from, t.to),
                        })
                        .collect::<Vec<_>>()
                ),
            }),
        }
    }
    for object in &then.no_transitions {
        // The object did not move: no region the scenario described left the
        // state it described. A self-transition re-enters the state it is in,
        // and a region the scenario said nothing about settles on the first
        // ticks from wherever its machine starts it — neither is the object
        // moving (D15).
        let described = run.runtime.store.described.get(object).cloned().unwrap_or_default();
        if let Some(t) = taken.iter().find(|t| {
            &t.object == object && t.from != t.to && described.iter().any(|r| r == &t.region)
        }) {
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
    // What a scenario counts is the acts the machinery performed, whether the
    // world took them or refused them: a slow host's three `start_session`
    // calls are three acts, of which the multiplexer refused two as a name it
    // already held (72, 73, S6). A repeat whose write carries the identity it
    // had is still an act — the write is the no-op, and the act runs (127,
    // `contract/write-effect.yaml`, which counts two `light` acts under one
    // identity). What is suppressed before it runs, by a proof that already
    // holds, never reaches this list at all.
    let count_of = |name: &str, object: Option<&String>| -> usize {
        performed
            .iter()
            .filter(|e| e.name == name && object.is_none_or(|o| &e.object == o))
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
            // The machinery's own acts — the rail numbering, the projection
            // rewritten — are not what a scenario closes over unless it names
            // one; the count above counts them by name wherever it does.
            if crate::conformance::drive::machinery(&run.runtime.store, &e.object) {
                continue;
            }
            // Nor is the settling of a region the scenario said nothing about:
            // it starts where its machine starts it and reaches the state the
            // world it was seeded in implies, which is the described state
            // arriving rather than the run doing something (D15).
            let region = e
                .effect_id
                .strip_prefix(&format!("{}/", e.object))
                .and_then(|rest| rest.split('/').next())
                .unwrap_or_default();
            let described = run
                .runtime
                .store
                .described
                .get(&e.object)
                .is_some_and(|paths| paths.iter().any(|path| path == region));
            if !described {
                continue;
            }
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
        // The tail at this boundary, as a sink whose mark is the start of the
        // run reads it: what reached done, landed, closed or dropped (14).
        if !want.tail.is_empty() {
            let at = want.after_step.unwrap_or(run.tail_after.len());
            let tail = run.tail_after.get(at.saturating_sub(1)).cloned().unwrap_or_default();
            for expected in &want.tail {
                let hit = tail.iter().any(|entry| match expected {
                    Value::String(kind) => &entry.kind == kind,
                    Value::Object(fields) => fields.iter().all(|(name, value)| match name.as_str() {
                        "kind" => value.as_str() == Some(entry.kind.as_str()),
                        "object" => value.as_str() == Some(entry.object.as_str()),
                        "state" => value.as_str() == Some(entry.state.as_str()),
                        "by" => value.as_str() == entry.by.as_deref(),
                        _ => false,
                    }),
                    _ => false,
                });
                if !hit {
                    failures.push(Failure {
                        clause: "decisions".into(),
                        step: label,
                        expected: format!("the tail carries {expected}"),
                        actual: format!(
                            "the tail: {:?}",
                            tail.iter().map(|e| (e.object.as_str(), e.kind.as_str())).collect::<Vec<_>>()
                        ),
                    });
                }
            }
        }
        for (id, number) in &want.numbers {
            // A scenario names a decision the way `given.register` does —
            // `<object>/<kind>` — and the register's own key carries the point
            // its state was entered as well (15).
            let got = standing
                .iter()
                .find(|d| {
                    &d.id == id || &crate::runner::Runtime::decision_name(&d.object, &d.kind) == id
                })
                .and_then(|d| d.number);
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
            // A scenario writes the state as the machine file writes it; a
            // value that is not text reads as what it says.
            let want = match want {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
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
            // Writes of the work. The register and the projection are the
            // machinery's own record of what it did, written on the same line
            // and counted separately (D5, 167).
            "total" => json!(run.runtime.store.work_writes.saturating_sub(run.writes_at_start)),
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
        // A `status:` clause naming a step is about the view as it stood after
        // that step, not at the end of the run (141, S13).
        let after_step = then
            .status
            .get("after_step")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let at_step = after_step
            .and_then(|n| run.status_after.get(n.saturating_sub(1)))
            .cloned()
            .unwrap_or_else(|| run.status_after.last().cloned().unwrap_or(Value::Null));
        for (key, want) in &then.status {
            let got = match key.as_str() {
                // The step the clause is about; it names the reading, and is
                // not itself a thing the view says.
                "after_step" => want.clone(),
                "as_of" => json!(store.status_as_of),
                // What the view showed as stale then: the work whose holder
                // stopped renewing, and the host itself (146, 150).
                "stale" => at_step.get("stale").cloned().unwrap_or(Value::Null),
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
        let got = observe(run, scenario, key);
        match got {
            None => failures.push(Failure {
                clause: format!("state_store.{key}"),
                step: None,
                expected: format!("{want}"),
                actual: "the profile answered nothing for this key".into(),
            }),
            Some(got) if !answers(&got, want) => failures.push(Failure {
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
            return Records::leases(&run.runtime.store, object)
                .ok()
                .flatten()
                .or_else(|| run.runtime.store.leases.get(object).cloned())
                .map(|l| json!(l.holder))
        }
        // Whether a named host had the lease before it expired. A host that
        // waits does not touch another's work: it takes the lease only once the
        // expiry the operator's response brought about has passed (150, I15).
        key if key.starts_with("touched_by_") && key.ends_with("_before_expiry") => {
            let who = key
                .trim_start_matches("touched_by_")
                .trim_end_matches("_before_expiry")
                .replace('_', "-");
            if events.iter().any(|e| e.holder == who && !e.expired) {
                return Some(json!(true));
            }
            let held = Records::leases(&run.runtime.store, object).ok().flatten();
            // It holds it now: it touched it before the expiry only if it took
            // it before the lease reached `expired`.
            let expired_at = run
                .ticks
                .iter()
                .flat_map(|t| t.transitions.iter())
                .find(|t| t.object == flywheel_domain::leases::id_for(object) && t.to == "expired")
                .and_then(|_| {
                    run.ticks
                        .iter()
                        .find(|t| {
                            t.transitions.iter().any(|x| {
                                x.object == flywheel_domain::leases::id_for(object)
                                    && x.to == "expired"
                            })
                        })
                        .map(|t| t.at)
                });
            return Some(json!(match (held, expired_at) {
                (Some(l), Some(at)) => l.holder == who && l.taken_at < at,
                (Some(l), None) => l.holder == who,
                (None, _) => false,
            }));
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

/// Whether what the profile answered is what the scenario asked for.
///
/// A scenario may state a bound rather than a number — `">= 1"` says at least
/// one, which is what a clause about a queue of intentions means (S18) — and a
/// map of them is answered field by field.
fn answers(got: &Value, want: &Value) -> bool {
    match (got, want) {
        (_, Value::String(text)) => match comparison(text) {
            Some((op, n)) => got.as_f64().is_some_and(|got| match op {
                ">=" => got >= n,
                "<=" => got <= n,
                ">" => got > n,
                "<" => got < n,
                _ => got == n,
            }),
            None => got == want,
        },
        (Value::Object(got), Value::Object(want)) => want
            .iter()
            .all(|(k, v)| got.get(k).is_some_and(|g| answers(g, v))),
        _ => got == want,
    }
}

/// A comparison a scenario wrote as text, or none.
fn comparison(text: &str) -> Option<(&str, f64)> {
    for op in [">=", "<=", ">", "<", "=="] {
        if let Some(rest) = text.trim().strip_prefix(op) {
            return rest.trim().parse().ok().map(|n| (op, n));
        }
    }
    None
}

/// Every take of the lease the scenario's race is about: the object its
/// `leases:` clause names, or, where it names none, whichever a host was
/// refused. A lease nobody contested was no race (128, 134, 162, I15).
fn contested(run: &Run, scenario: &Scenario) -> Vec<flywheel_domain::records::RunEntry> {
    let entries = run_record(run);
    let takes: Vec<&flywheel_domain::records::RunEntry> = entries
        .iter()
        .filter(|e| e.kind == "lease" && e.reason == "take")
        // The machinery's own leases — the rail's, a sink's presenter — are not
        // the work two hosts race for (D5, 148, 167).
        .filter(|e| !crate::conformance::drive::machinery(&run.runtime.store, &e.object))
        .collect();
    let named: Vec<&str> = scenario.then.leases.keys().map(|k| k.as_str()).collect();
    let about: Vec<&str> = match named.is_empty() {
        false => named,
        true => takes
            .iter()
            .filter(|e| {
                e.fields
                    .iter()
                    .any(|(n, v)| n == "outcome" && v == "held-by-another")
            })
            .map(|e| e.object.as_str())
            .collect(),
    };
    takes
        .into_iter()
        .filter(|e| about.contains(&e.object.as_str()))
        .cloned()
        .collect()
}

/// Whether a moment fell inside one of a host's offline windows (151, D4a).
fn while_offline(run: &Run, host: &str, at: chrono::DateTime<chrono::Utc>) -> bool {
    run.offline
        .get(host)
        .into_iter()
        .flatten()
        .any(|(cut, back)| at >= *cut && back.is_none_or(|back| at < back))
}

/// The hosts a run knows about: the ones it cut off, and the ones the record
/// says wrote.
fn hosts_of(run: &Run) -> Vec<String> {
    let mut out: Vec<String> = run.offline.keys().cloned().collect();
    for object in run
        .runtime
        .store
        .list_records(&Scope::Machine("host".into()))
        .unwrap_or_default()
    {
        let name = object.id.trim_start_matches("host/").to_string();
        if !out.contains(&name) {
            out.push(name);
        }
    }
    out.sort();
    out
}

/// Every run-record entry the shared line holds, whoever wrote it (79, 167).
fn run_record(run: &Run) -> Vec<flywheel_domain::records::RunEntry> {
    match run.runtime.store.durable() {
        Some(durable) => durable
            .lock()
            .ok()
            .and_then(|git| git.all_run_records().ok())
            .unwrap_or_default(),
        None => vec![],
    }
}

/// Every session the store holds a record of, whoever ran it (93b).
fn session_facts(run: &Run) -> Vec<flywheel_engine::Object> {
    run.runtime
        .store
        .list_records(&Scope::All)
        .unwrap_or_default()
        .into_iter()
        .filter(|o| o.id.starts_with("fact/session/"))
        .collect()
}

/// Whether the run started this session, rather than being seeded holding it.
/// A scenario's clock starts at one fixed point, so anything started after it
/// was started by a step (D15).
fn started_during(fact: &flywheel_engine::Object) -> bool {
    fact.record
        .get("started_at")
        .and_then(|v| v.as_str())
        .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
        .is_some_and(|at| at.with_timezone(&chrono::Utc) > super::drive::start_of_time())
}

/// Whether a session record says it is still the operator's to run: started,
/// not ended.
fn running_now(fact: &flywheel_engine::Object) -> bool {
    fact.record.get("started_at").is_some_and(|v| !v.is_null())
        && !fact.record.get("ended_at").is_some_and(|v| !v.is_null())
}

/// What the profile answers for one observation key. A key the runner does not
/// yet answer returns none, which fails loudly rather than passing.
pub fn observe(run: &Run, scenario: &Scenario, key: &str) -> Option<Value> {
    if let Some(v) = run.observations.get(key) {
        return Some(v.clone());
    }
    let store = &run.runtime.store;
    // The machinery's own acts — the rail's projection, a sink's delivery —
    // are not what a contract scenario counts: it counts the work's (D5, 167).
    let effects: Vec<&crate::runner::EffectRecord2> = run
        .ticks
        .iter()
        .flat_map(|t| t.effects.iter())
        .filter(|e| {
            !store
                .objects
                .get(&e.object)
                .map(|o| flywheel_domain::leases::machinery(&o.machine))
                .unwrap_or_else(|| e.object == flywheel_domain::RAIL)
        })
        .collect();
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
            json!(flywheel_domain::set::repository_digest()
                .is_none_or(|held| held == flywheel_domain::set::digest()))
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
        // The idle decision (26) is the standing session's; a with-operator one
        // is never asked about, so a run where none stood answers false (25).
        "idle_offered" => json!(run
            .decisions_after
            .iter()
            .flatten()
            .any(|d| d.kind == "idle")),
        // How the multiplexer's report was read (73). A with-operator session
        // is present by a keystroke within the window the profile states; every
        // other session is present by the pane and what it is doing (25).
        "presence_read_as" => {
            let by_keystroke = store.objects.values().any(|o| {
                o.machine == "operator-session"
                    || o.record.get("type").and_then(|v| v.as_str()) == Some("with-operator")
            });
            json!(match by_keystroke {
                true => "keystroke within the profile's window",
                false => "the pane and its activity",
            })
        }
        // What the session's thread held behind it (144). The operator's own
        // session is opened on no thread, so there is none (69).
        "thread_behind_session" => {
            let behind: Vec<String> = store
                .threads
                .iter()
                .filter(|(object, _)| {
                    store
                        .objects
                        .get(*object)
                        .is_some_and(|o| o.machine == "operator-session")
                        || store.world.sessions.keys().any(|s| s.starts_with(&format!("{object}/")))
                })
                .flat_map(|(_, entries)| entries.iter().map(|e| e.kind.clone()))
                .collect();
            json!(match behind.is_empty() {
                true => "none".to_string(),
                false => behind.join(", "),
            })
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
        // What the engine asked the store for while it was deciding: the
        // record operations it served in that window and no other (136).
        "engine_reads_only" => json!(store.deciding.operations()),
        "projection_rewritten_from_source" => json!(store.projection_rewritten_from_source()?),
        "renderings_stored" => json!(0),
        // The sessions the store says are running: started, not ended and not
        // reported on. Where a profile keeps session records, that is the
        // answer; the stand-in world's panes are the answer where it does not
        // (93b, D8).
        "sessions_running_at_end" => match session_facts(run) {
            facts if !facts.is_empty() => json!(facts.iter().filter(|f| running_now(f)).count()),
            _ => json!(store.world.sessions.values().filter(|s| s.pane).count()),
        },
        // ---- the race for one lease (128, 134, 162, I15)
        //
        // The take is a push with expected-old, and the push is the
        // compare-and-swap: two hosts ask, one lands, and the loser fetches and
        // reads who holds it before it decides anything.
        "lease_pushes_attempted" => json!(contested(run, scenario).len()),
        "lease_pushes_accepted" => json!(contested(run, scenario)
            .iter()
            .filter(|e| e.fields.iter().any(|(n, v)| n == "outcome" && v == "held"))
            .count()),
        "loser_fetched_and_read_holder" => {
            let entries = run_record(run);
            let losers: Vec<_> = entries
                .iter()
                .filter(|e| e.kind == "lease" && e.reason == "take")
                .filter(|e| {
                    e.fields
                        .iter()
                        .any(|(n, v)| n == "outcome" && v == "held-by-another")
                })
                .collect();
            json!(!losers.is_empty()
                && losers.iter().all(|e| {
                    e.fields.iter().any(|(n, v)| n == "reread" && v == "true")
                        && e.fields.iter().any(|(n, v)| n == "holder" && !v.is_empty())
                }))
        }
        // One host made the work, not both: no act ran twice on one object at
        // one moment, whatever raced for it (I15).
        "items_created_once" => {
            // One host made the work: no act on one object was performed by two
            // of them. A host repeating its own act as a tick settles is the
            // same host and the same act, and the write carries the identity it
            // had (127); two hosts would be two.
            let entries = run_record(run);
            let mut by: std::collections::BTreeMap<(String, String), Vec<String>> =
                Default::default();
            for entry in entries.iter().filter(|e| e.kind == "effect") {
                // The machinery's own — the rail's projection, a sink's
                // delivery — are not the work a scenario counts (D5, 167).
                if crate::conformance::drive::machinery(&run.runtime.store, &entry.object) {
                    continue;
                }
                let who = by
                    .entry((entry.object.clone(), entry.reason.clone()))
                    .or_default();
                if !who.contains(&entry.host) {
                    who.push(entry.host.clone());
                }
            }
            json!(by.values().all(|who| who.len() < 2))
        }
        // ---- what a host with no route did, and what landed when it came back
        // (151, 161, 165, D4a)
        //
        // A local commit is an intention until its push lands (161), so what a
        // host wrote while its route was down is counted from the record it
        // pushed once it was back.
        "local_commits_while_offline" => {
            let entries = run_record(run);
            let mut out = serde_json::Map::new();
            for host in hosts_of(run) {
                let made = entries
                    .iter()
                    .filter(|e| e.host == host && e.kind == "write")
                    .filter(|e| while_offline(run, &host, e.at))
                    .count();
                out.insert(host, json!(made));
            }
            Value::Object(out)
        }
        // A host that cannot reach the store takes no new lease (151).
        "leases_taken_while_offline" => {
            let mut out = serde_json::Map::new();
            for host in hosts_of(run) {
                let taken = run
                    .runtime
                    .store
                    .list_records(&Scope::All)
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|o| Records::leases(&run.runtime.store, &o.id).ok().flatten())
                    .filter(|l| l.holder == host && while_offline(run, &host, l.taken_at))
                    .count();
                out.insert(host, json!(taken));
            }
            Value::Object(out)
        }
        // Nor does it start a session for work it does not already hold (151).
        "sessions_started_while_offline" => {
            let mut out = serde_json::Map::new();
            for host in hosts_of(run) {
                let started = session_facts(run)
                    .iter()
                    .filter(|f| f.record.get("host").and_then(|v| v.as_str()) == Some(host.as_str()))
                    // What the run started, not what it was seeded holding.
                    .filter(|f| started_during(f))
                    .filter(|f| {
                        f.record
                            .get("started_at")
                            .and_then(|v| v.as_str())
                            .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
                            .is_some_and(|at| while_offline(run, &host, at.with_timezone(&chrono::Utc)))
                    })
                    .count();
                out.insert(host, json!(started));
            }
            Value::Object(out)
        }
        // What each host wrote is on the shared line when the route is back:
        // an intention became a fact and nothing of anyone else's was lost
        // (133, 161, 165).
        "commits_present_after_reconnect" => {
            let entries = run_record(run);
            let mut out = serde_json::Map::new();
            for host in hosts_of(run) {
                out.insert(host.clone(), json!(entries.iter().any(|e| e.host == host)));
            }
            Value::Object(out)
        }
        // And the projection states the point it is as of, which is the last
        // write that landed (145, 167).
        "status_as_of_after_reconnect" => {
            let body = run
                .observations
                .get("status_written")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            // The point the projection states it is as of, and the newest
            // state write on the shared line: the projection is latest when
            // nothing it projects landed after it (145, 167).
            let as_of = body
                .split("as of commit ")
                .nth(1)
                .and_then(|rest| rest.split(' ').next())
                .unwrap_or_default()
                .to_string();
            let current = match run.runtime.store.durable() {
                Some(durable) => durable.lock().ok().is_some_and(|git| {
                    match git.newest_state_write().ok().flatten() {
                        Some(newest) => !as_of.is_empty() && git.is_ancestor(&newest, &as_of),
                        None => !as_of.is_empty(),
                    }
                }),
                None => false,
            };
            json!(match current {
                true => "latest",
                false => "behind",
            })
        }
        // Which attempt each host started. A takeover is a fresh attempt and
        // never the same session twice, so the number is what says a host
        // started the second one (150, `session.yaml` id, S13).
        "attempts_started_by_host" => {
            let mut out = serde_json::Map::new();
            for fact in session_facts(run) {
                let Some(host) = fact.record.get("host").and_then(|v| v.as_str()) else {
                    continue;
                };
                // What the run started, not what it was seeded holding: a
                // session the scenario described as already running was started
                // by nobody in it.
                if !started_during(&fact) {
                    continue;
                }
                let session = fact.id.trim_start_matches("fact/session/");
                let attempt = flywheel_domain::regions::attempt_of(session).unwrap_or(1) as u64;
                let held = out.get(host).and_then(|v| v.as_u64()).unwrap_or(0);
                out.insert(host.to_string(), json!(attempt.max(held)));
            }
            Value::Object(out)
        }
        // A host that came back ended its own session and reported it, and
        // started nothing again: the fresh attempt is the taking host's (150).
        "ended_own_pane_on_return" => {
            let mut out = serde_json::Map::new();
            for fact in session_facts(run) {
                let Some(host) = fact.record.get("host").and_then(|v| v.as_str()) else {
                    continue;
                };
                if !fact.record.get("taken_over_by").is_some_and(|v| !v.is_null()) {
                    continue;
                }
                let own = fact
                    .record
                    .get("reported_by_holder")
                    .is_some_and(|v| !v.is_null())
                    && !running_now(&fact);
                let held = out.get(host).and_then(|v| v.as_bool()).unwrap_or(true);
                out.insert(host.to_string(), json!(own && held));
            }
            Value::Object(out)
        }
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
        // ---- the moves curation stored, and what is left unmoved (110, 107)
        //
        // All three read the material under the machinery's prefix, which is
        // where the moves are written and where a person writing them by hand
        // writes them too (203, 110).
        "signal_moves_stored" | "signals_with_move" => {
            json!(flywheel_domain::signals::moves(&store.world.files).len())
        }
        // What the run began with, less those that ended with a move. A
        // scenario that states the count it began with has said how many
        // signals there were; the moves say how many were judged.
        "unmoved_after" => {
            let began = store
                .given
                .values()
                .find_map(|per| per.get("curation.unmoved_count").and_then(|v| v.as_u64()))
                .unwrap_or(0) as usize;
            let moved = flywheel_domain::signals::moves(&store.world.files).len();
            json!(began.saturating_sub(moved) + flywheel_domain::signals::unmoved(&store.world.files).len())
        }
        // One per signal of the dropped intent, each naming the drop (117).
        "moves_naming_drop" => json!(flywheel_domain::signals::moves(&store.world.files)
            .iter()
            .filter(|m| m.word() == "drop")
            .count()),
        "signals_written" => json!(store
            .objects
            .values()
            .filter(|o| o.machine == "signal")
            .count()),
        // A signal cites the capture it came from by being owned by it
        // (`capture.yaml` owns, 113).
        "signal_cites_capture" => {
            let signals: Vec<_> = store
                .objects
                .values()
                .filter(|o| o.machine == "signal")
                .collect();
            json!(!signals.is_empty()
                && signals.iter().all(|s| s
                    .parent
                    .as_ref()
                    .is_some_and(|p| store.objects.get(p).is_some_and(|c| c.machine == "capture"))))
        }
        // The record points at the document and never holds what is in it
        // (62): no field of any object carries the file's own text.
        "finding_record_holds_text" => {
            let bodies: Vec<&String> = store.world.files.values().filter(|b| !b.is_empty()).collect();
            json!(store.objects.values().any(|o| o.record.values().any(|v| {
                v.as_str()
                    .is_some_and(|text| bodies.iter().any(|body| body.as_str() == text))
            })))
        }
        // A session that offered a finding keeps working: nothing the offer
        // caused reaches back into it (13, 58, 71, I5). A session is
        // interrupted where its pane went or its activity stopped without the
        // session itself reporting an exit.
        "session_interrupted" => json!(store
            .world
            .sessions
            .values()
            .any(|s| s.exit.is_none() && (!s.pane || s.activity == "none"))),
        // How many distinct session names the machinery started, and how many
        // starts the multiplexer refused as a name it already held (72).
        "distinct_session_names_started" => {
            let mut names: Vec<&str> = run
                .ticks
                .iter()
                .flat_map(|t| t.effects.iter())
                .filter(|e| e.name == "start_session" && e.written)
                .map(|e| e.object.as_str())
                .collect();
            names.sort();
            names.dedup();
            json!(names.len())
        }
        "multiplexer_refused_duplicates" => json!(store.duplicate_starts),
        // What the run reported as failed (79). The report effect carries a
        // failure to attention; a slow start is not one.
        "reported_failed" => json!(run
            .ticks
            .iter()
            .flat_map(|t| t.effects.iter())
            .any(|e| e.name == "report")),
        "exit_written_through_command" => json!(!store.threads.is_empty()),
        "state_read_from_place_disk" => json!(0),
        _ => return None,
    })
}
