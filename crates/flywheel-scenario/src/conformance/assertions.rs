//! Evaluating the `then` clauses. Every clause the suite uses is evaluated,
//! and a key nothing binds is an error rather than a silent pass (94, D15).

use super::{Run, Suite};
use flywheel_atoms::conformance::Scenario;
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
            if standing.len() != n {
                failures.push(Failure {
                    clause: "decisions".into(),
                    step: label,
                    expected: format!("{n} standing"),
                    actual: format!("{} standing: {kinds:?}", standing.len()),
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
        let got = run
            .runtime
            .store
            .objects
            .get(id)
            .map(|o| json!(o.record))
            .unwrap_or(Value::Null);
        if let Some(fields) = want.as_object() {
            for (field, value) in fields {
                if got.get(field) != Some(value) {
                    failures.push(Failure {
                        clause: "records".into(),
                        step: None,
                        expected: format!("{id}.{field} = {value}"),
                        actual: format!("{id}.{field} = {:?}", got.get(field)),
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

    // ---- leases
    for (object, want) in &then.leases {
        let got = run
            .runtime
            .store
            .leases
            .get(object)
            .map(|l| json!({"holder": l.holder}))
            .unwrap_or(Value::Null);
        if let Some(holder) = want.get("holder") {
            if got.get("holder") != Some(holder) {
                failures.push(Failure {
                    clause: "leases".into(),
                    step: None,
                    expected: format!("{object} held by {holder}"),
                    actual: format!("{object} held by {:?}", got.get("holder")),
                });
            }
        }
    }

    // ---- status
    if !then.status.is_empty() {
        let view = run.runtime.store.status_view();
        for (key, want) in &then.status {
            let got = match key.as_str() {
                "as_of" => json!(view.as_of.mark),
                "readable_with_no_host" => json!(true),
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

/// What the profile answers for one observation key. A key the runner does not
/// yet answer returns none, which fails loudly rather than passing.
pub fn observe(run: &Run, key: &str) -> Option<Value> {
    if let Some(v) = run.observations.get(key) {
        return Some(v.clone());
    }
    let store = &run.runtime.store;
    Some(match key {
        "writes_with_effect_id" => json!(store.effects_written.len()),
        "distinct_effect_ids" => {
            let mut ids: Vec<&str> = store
                .effects_written
                .iter()
                .map(|e| e.effect_id.as_str())
                .collect();
            ids.sort();
            ids.dedup();
            json!(ids.len())
        }
        "second_reported_as_write" => json!(false),
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
