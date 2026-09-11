//! The model's invariants (model.md §15, I1–I16), evaluated over a run's trace
//! for every scenario that names them (audit 4).
//!
//! An invariant is a predicate over what the run recorded — the transitions,
//! the effects, the lease traffic, the decisions standing and the store at the
//! end — and a scenario naming one fails when the trace breaks it, naming the
//! invariant. Six of the sixteen are not properties of a trace: they are held
//! by a gate, a store or a checker named in `HELD_ELSEWHERE`, and a scenario
//! naming one of those is not failed here. A name in neither list is an
//! error, never a silent pass (94, D15).

use super::assertions::Failure;
use crate::runner::{DecisionRecord, LeaseEvent, TickRecord};
use crate::store::Store;
use flywheel_atoms::conformance::GivenObject;
use flywheel_engine::Definitions;
use std::collections::{BTreeMap, BTreeSet};

/// What a predicate reads.
pub struct View<'a> {
    pub given: &'a [GivenObject],
    pub ticks: &'a [TickRecord],
    pub lease_log: &'a [LeaseEvent],
    pub decisions_after: &'a [Vec<DecisionRecord>],
    pub store: &'a Store,
    pub defs: &'a Definitions,
}

/// The invariants a trace can break, each with the statement model.md gives it.
pub const TRACE_CHECKED: &[(&str, &str)] = &[
    ("I1", "no effect before `approved`; `approved` is entered only by a response or a dictation, or by the cadence the operator set"),
    ("I2", "`applied_responses` written with the state change; response id = delivery id"),
    ("I3", "a decision is a state: one creating state per kind, and leaving it retracts"),
    ("I5", "no effect targets a session in `working`; `rebase_place` and `tell_moved` guard on idle"),
    ("I6", "`session.ended` is entered only by the owner's end; a pane killed by hand is `lost`, never a response"),
    ("I8", "`create_bolt` is never performed for a chore; the chore type has no bolt target"),
    ("I10", "`ledger-cell.judged` leaves only on a version move or evidence gone"),
    ("I11", "one lease per object by compare-and-swap, within the host's declaration; the status view shows the holder"),
    ("I12", "every line and place effect, including removal, is a machinery atom"),
    ("I16", "`place.ready` requires `contains_line`; `session.requested` leaves only from a ready place"),
];

/// The invariants no trace can break, and where each is held instead.
pub const HELD_ELSEWHERE: &[(&str, &str)] = &[
    ("I4", "one source per state; the projection is rewritten from the source by the host's drift check (`Host::rewrite_status`) and `commit_status`"),
    ("I7", "the engine holds nothing between ticks: the runner's own tests and S05's rerun from the store"),
    ("I9", "`flywheel asbuilt check` in the built repository — phase 3"),
    ("I13", "atoms are abstract and only `profiles/` name a tool: the model's `check.py`"),
    ("I14", "the host's disk holds fetched state and local commits alone: `flywheel-store-git`'s unit tests"),
    ("I15", "`--force-with-lease` on lease branches and expected-old on `main`: the store's push path, `contract/lease.yaml` and `contract/single-writer.yaml`"),
];

/// The life states a unit stands in before `approved` (unit.yaml life).
const BEFORE_APPROVED: &[&str] = &["in-proposal", "proposed", "deferred", "superseded"];

pub fn statement(name: &str) -> Option<&'static str> {
    TRACE_CHECKED
        .iter()
        .chain(HELD_ELSEWHERE.iter())
        .find(|(n, _)| *n == name)
        .map(|(_, s)| *s)
}

/// Evaluate every invariant the scenario names.
pub fn check(named: &[String], view: &View) -> Vec<Failure> {
    let mut failures = Vec::new();
    for name in named {
        if HELD_ELSEWHERE.iter().any(|(n, _)| n == name) {
            continue;
        }
        let Some((_, statement)) = TRACE_CHECKED.iter().find(|(n, _)| n == name) else {
            failures.push(Failure {
                clause: format!("invariant {name}"),
                step: None,
                expected: "an invariant of model.md §15".into(),
                actual: format!("`{name}` is no invariant the model states"),
            });
            continue;
        };
        let broken = match name.as_str() {
            "I1" => i1(view),
            "I2" => i2(view),
            "I3" => i3(view),
            "I5" => i5(view),
            "I6" => i6(view),
            "I8" => i8(view),
            "I10" => i10(view),
            "I11" => i11(view),
            "I12" => i12(view),
            "I16" => i16(view),
            _ => vec![],
        };
        for actual in broken {
            failures.push(Failure {
                clause: format!("invariant {name}"),
                step: None,
                expected: statement.to_string(),
                actual,
            });
        }
    }
    failures
}

// ------------------------------------------------------------- what is read

/// The states each object's regions stand in, as the trace moves them: the
/// given state first, then every transition in order.
struct Tracked {
    states: BTreeMap<(String, String), String>,
}

impl Tracked {
    fn from_given(given: &[GivenObject]) -> Tracked {
        let mut states = BTreeMap::new();
        for object in given {
            for (region, state) in &object.state {
                states.insert((object.id.clone(), region.clone()), state.clone());
            }
        }
        Tracked { states }
    }

    fn apply(&mut self, tick: &TickRecord) {
        for t in &tick.transitions {
            self.states
                .insert((t.object.clone(), t.region.clone()), t.to.clone());
        }
    }

    /// The state of a region, matched by its last path segment so `life` and
    /// `alive.activity` are found however the trace spells the path.
    fn state_of(&self, object: &str, region: &str) -> Option<&str> {
        self.states
            .iter()
            .find(|((o, r), _)| o == object && (r == region || r.ends_with(&format!(".{region}"))))
            .map(|(_, s)| s.as_str())
    }
}

impl View<'_> {
    fn machine_of(&self, object: &str) -> String {
        if let Some(held) = self.store.objects.get(object) {
            return held.machine.clone();
        }
        if let Some(given) = self.given.iter().find(|g| g.id == object) {
            return given.machine.clone();
        }
        object.split('/').next().unwrap_or_default().to_string()
    }

    fn record_of(&self, object: &str, field: &str) -> Option<serde_json::Value> {
        if let Some(held) = self.store.objects.get(object) {
            return held.record.get(field).cloned();
        }
        self.given
            .iter()
            .find(|g| g.id == object)
            .and_then(|g| g.record.get(field).cloned())
    }

    /// The sessions of a place: those whose record names it.
    fn sessions_of_place(&self, place: &str) -> Vec<String> {
        self.store
            .objects
            .values()
            .filter(|o| o.machine == "session")
            .filter(|o| o.record.get("place").and_then(|v| v.as_str()) == Some(place))
            .map(|o| o.id.clone())
            .chain(
                self.given
                    .iter()
                    .filter(|g| g.machine == "session")
                    .filter(|g| g.record.get("place").and_then(|v| v.as_str()) == Some(place))
                    .map(|g| g.id.clone()),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}

// --------------------------------------------------------------- predicates

fn i1(view: &View) -> Vec<String> {
    let mut broken = vec![];
    let mut tracked = Tracked::from_given(view.given);
    for tick in view.ticks {
        for t in &tick.transitions {
            let machine = view.machine_of(&t.object);
            if t.to == "approved"
                && t.region.ends_with("life")
                && matches!(machine.as_str(), "unit" | "elaboration")
                && t.response.is_none()
                && !t
                    .reason
                    .as_deref()
                    .is_some_and(|r| r.contains("cadence") || r.contains("dictat"))
            {
                broken.push(format!(
                    "tick {}: {} entered approved by no response, dictation or cadence ({})",
                    tick.tick,
                    t.object,
                    t.reason.clone().unwrap_or_else(|| "no reason".into())
                ));
            }
        }
        tracked.apply(tick);
        for e in &tick.effects {
            if view.machine_of(&e.object) != "unit" {
                continue;
            }
            if let Some(life) = tracked.state_of(&e.object, "life") {
                if BEFORE_APPROVED.contains(&life) {
                    broken.push(format!(
                        "tick {}: effect `{}` on {} while its life is `{life}`, before approved",
                        tick.tick, e.name, e.object
                    ));
                }
            }
        }
    }
    broken
}

fn i2(view: &View) -> Vec<String> {
    let mut broken = vec![];
    for tick in view.ticks {
        for t in &tick.transitions {
            let Some(response) = &t.response else {
                continue;
            };
            if let Some(held) = view.store.objects.get(&t.object) {
                if !held.applied_responses.iter().any(|r| r == response) {
                    broken.push(format!(
                        "tick {}: {} moved by response `{response}` and its applied_responses does not carry it",
                        tick.tick, t.object
                    ));
                }
            }
            let recorded = view.store.objects.contains_key(&format!("response/{response}"))
                || view.store.responses.iter().any(|r| &r.id == response)
                || view.store.objects.values().any(|o| {
                    o.machine == "response"
                        && o.record.get("delivery_id").and_then(|v| v.as_str()) == Some(response)
                });
            if !recorded {
                broken.push(format!(
                    "tick {}: response `{response}` moved {} and no response record carries that id as its delivery",
                    tick.tick, t.object
                ));
            }
        }
    }
    broken
}

fn i3(view: &View) -> Vec<String> {
    let mut broken = vec![];
    let Some(standing) = view.decisions_after.last() else {
        return broken;
    };
    for decision in standing {
        let Some(object) = view.store.objects.get(&decision.object) else {
            broken.push(format!(
                "decision {} of kind `{}` stands for {}, which is no object of the store",
                decision.id, decision.kind, decision.object
            ));
            continue;
        };
        let creating = creating_states(view.defs, &object.machine);
        let in_one = object
            .config
            .values()
            .any(|state| creating.contains(&(state.clone(), decision.kind.clone())));
        if !in_one {
            broken.push(format!(
                "decision {} of kind `{}` stands and {} is in no state that creates one: {:?}",
                decision.id, decision.kind, decision.object, object.config
            ));
        }
    }
    broken
}

/// Every (state, decision kind) a machine's states create, nested regions
/// included.
fn creating_states(defs: &Definitions, machine: &str) -> BTreeSet<(String, String)> {
    fn walk(regions: &BTreeMap<String, flywheel_engine::defs::Region>, out: &mut BTreeSet<(String, String)>) {
        for region in regions.values() {
            for (name, state) in &region.states {
                if let Some(decision) = &state.decision {
                    out.insert((name.clone(), decision.kind.clone()));
                }
                walk(&state.regions, out);
            }
        }
    }
    let mut out = BTreeSet::new();
    if let Some(m) = defs.machines.get(machine) {
        walk(&m.regions, &mut out);
    }
    out
}

fn i5(view: &View) -> Vec<String> {
    let mut broken = vec![];
    let mut tracked = Tracked::from_given(view.given);
    for tick in view.ticks {
        tracked.apply(tick);
        for e in &tick.effects {
            if !matches!(e.name.as_str(), "rebase_place" | "tell_moved") {
                continue;
            }
            for session in view.sessions_of_place(&e.object) {
                if tracked.state_of(&session, "activity") == Some("working") {
                    broken.push(format!(
                        "tick {}: `{}` on {} while its session {session} is working",
                        tick.tick, e.name, e.object
                    ));
                }
            }
        }
    }
    broken
}

fn i6(view: &View) -> Vec<String> {
    let mut broken = vec![];
    for tick in view.ticks {
        for t in &tick.transitions {
            if view.machine_of(&t.object) != "session" || !t.region.ends_with("life") {
                continue;
            }
            match t.to.as_str() {
                "ended" | "lost" if t.response.is_some() => broken.push(format!(
                    "tick {}: {} entered `{}` by response `{}`; a response retires the work and the owner ends the session, and a pane gone is never a response",
                    tick.tick,
                    t.object,
                    t.to,
                    t.response.clone().unwrap_or_default()
                )),
                "ended" if t.from == "lost" => broken.push(format!(
                    "tick {}: {} went from `lost` to `ended`; a lost pane is lost",
                    tick.tick, t.object
                )),
                _ => {}
            }
        }
    }
    broken
}

fn i8(view: &View) -> Vec<String> {
    let mut broken = vec![];
    for tick in view.ticks {
        for e in &tick.effects {
            if e.name != "create_bolt" {
                continue;
            }
            if view.record_of(&e.object, "type").and_then(|v| v.as_str().map(String::from))
                == Some("chore".into())
            {
                broken.push(format!(
                    "tick {}: `create_bolt` performed for the chore {}",
                    tick.tick, e.object
                ));
            }
        }
    }
    broken
}

fn i10(view: &View) -> Vec<String> {
    let mut broken = vec![];
    for tick in view.ticks {
        for t in &tick.transitions {
            if view.machine_of(&t.object) != "ledger-cell" || t.from != "judged" || t.to == "judged" {
                continue;
            }
            let why = t.reason.clone().unwrap_or_default();
            if !(why.contains("version") || why.contains("evidence")) {
                broken.push(format!(
                    "tick {}: {} left `judged` for `{}` on `{why}`, neither a version move nor evidence gone",
                    tick.tick, t.object, t.to
                ));
            }
        }
    }
    broken
}

fn i11(view: &View) -> Vec<String> {
    let mut broken = vec![];
    for event in view.lease_log {
        if event.held && !event.coverable {
            broken.push(format!(
                "{} took a lease on {} outside its declaration",
                event.holder, event.object
            ));
        }
        if event.doubled {
            broken.push(format!(
                "{} overwrote another host's live lease on {}",
                event.holder, event.object
            ));
        }
    }
    // The status view shows the holder of every lease that stands (143).
    let store = view.store;
    let point = flywheel_atoms::ReadPoint {
        mark: format!("write {}", store.writes),
        seq: store.writes,
        at: store.now,
    };
    match flywheel_domain::status::read(
        store,
        view.defs,
        &point,
        store.now,
        chrono::Duration::minutes(5),
        chrono::Duration::minutes(30),
    ) {
        Ok(status) => {
            for (object, lease) in &store.leases {
                let object = flywheel_domain::leases::object_of(object).unwrap_or(object);
                let machine = view.machine_of(object);
                if flywheel_domain::leases::machinery(&machine) {
                    continue;
                }
                let shown = status
                    .rows
                    .iter()
                    .find(|row| row.object == object)
                    .and_then(|row| row.holder.clone());
                if shown.as_deref() != Some(lease.holder.as_str()) {
                    broken.push(format!(
                        "{} holds {object} and the status view shows {}",
                        lease.holder,
                        shown.unwrap_or_else(|| "no holder".into())
                    ));
                }
            }
        }
        Err(e) => broken.push(format!("the status view could not be read: {e}")),
    }
    broken
}

fn i12(view: &View) -> Vec<String> {
    let mut broken = vec![];
    for tick in view.ticks {
        for e in &tick.effects {
            if !view.defs.atoms.effects.contains_key(&e.name) {
                broken.push(format!(
                    "tick {}: `{}` on {} is no atom of `atoms.yaml`",
                    tick.tick, e.name, e.object
                ));
            }
        }
    }
    broken
}

fn i16(view: &View) -> Vec<String> {
    let mut broken = vec![];
    let mut tracked = Tracked::from_given(view.given);
    for tick in view.ticks {
        for t in &tick.transitions {
            let machine = view.machine_of(&t.object);
            if machine == "place" && t.to == "ready" && t.region.ends_with("life") {
                let guards: Vec<&crate::runner::GuardRecord> = tick
                    .guards
                    .iter()
                    .filter(|g| g.object == t.object && g.region == t.region)
                    .collect();
                if !guards.is_empty() && !guards.iter().any(|g| g.matched.contains("contains_line")) {
                    broken.push(format!(
                        "tick {}: {} reached ready on a guard that did not read contains_line: {:?}",
                        tick.tick,
                        t.object,
                        guards.iter().map(|g| g.matched.as_str()).collect::<Vec<_>>()
                    ));
                }
            }
            if machine == "session" && t.from == "requested" && t.to != "requested" && t.region.ends_with("life") {
                if let Some(place) = view.record_of(&t.object, "place").and_then(|v| v.as_str().map(String::from)) {
                    if let Some(life) = tracked.state_of(&place, "life") {
                        if life != "ready" {
                            broken.push(format!(
                                "tick {}: {} left requested while its place {place} is `{life}`, not ready",
                                tick.tick, t.object
                            ));
                        }
                    }
                }
            }
        }
        tracked.apply(tick);
    }
    broken
}
