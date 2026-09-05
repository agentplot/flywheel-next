//! The plan mockup scenario driven through the stand-in: one approval cascades
//! into items, places, sessions and a merge; one dictation retires work.

use flywheel_scenario::{scenario, Runtime};
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("workspace root")
}

fn seeded() -> Runtime {
    let defs = flywheel_engine::load::load_dir(&workspace().join("definitions")).expect("definitions load");
    let sc = scenario::load(&workspace().join("scenarios/plan-mockup.yaml")).expect("scenario loads");
    scenario::seed(defs, &sc)
}

fn state(rt: &Runtime, id: &str) -> Option<String> {
    rt.store.objects.get(id).and_then(|o| o.top_state()).map(String::from)
}

/// Tick until `quiet` consecutive ticks fire nothing, or `max` ticks in all. Scripted sessions
/// act on their own clock, so one empty tick is not quiescence.
fn run_until_quiet(rt: &mut Runtime, max: usize, quiet: usize) -> usize {
    let mut idle = 0;
    for n in 0..max {
        if rt.tick() == 0 { idle += 1; } else { idle = 0; }
        if idle >= quiet { return n + 1; }
    }
    panic!("not quiescent after {max} ticks");
}

fn number_of(rt: &mut Runtime, object: &str, kind: &str) -> Option<u32> {
    rt.decisions().iter().find(|d| d.object == object && d.kind == kind).and_then(|d| d.number)
}

#[test]
fn approving_a_unit_cascades_to_a_merge_and_a_question() {
    let mut rt = seeded();
    rt.settle(50);

    let unit = "unit/atlas/status-writer";
    let wi1 = "unit/atlas/status-writer/wi-1";
    let wi2 = "unit/atlas/status-writer/wi-2";
    assert_eq!(state(&rt, unit).as_deref(), Some("proposed"));
    assert!(!rt.store.objects.contains_key(wi1), "no items before approval");

    let number = number_of(&mut rt, unit, "unit-proposed").expect("the unit's proposal is a numbered decision");
    let given_before = rt.store.register.numbers.clone();
    let standing_before: Vec<(String, u32)> = rt.decisions().iter().filter(|d| d.object != unit).map(|d| (d.id.clone(), d.number.unwrap())).collect();

    rt.respond(number, "yes", "test");
    let ticks = run_until_quiet(&mut rt, 200, 5);
    assert!(ticks < 200);

    assert_eq!(state(&rt, unit).as_deref(), Some("in-flight"), "the unit waits on its second item");
    assert_eq!(state(&rt, wi1).as_deref(), Some("merged"));
    assert_eq!(rt.store.objects[wi1].config.get("place.place.life").map(String::as_str), Some("removed"), "the merged item's place is gone");
    assert_eq!(state(&rt, wi2).as_deref(), Some("in-type"));
    assert_eq!(rt.store.objects[wi2].config.get("life.in-type.stages").map(String::as_str), Some("build"));

    let decisions = rt.decisions();
    let question: Vec<_> = decisions.iter().filter(|d| d.object == wi2).collect();
    assert_eq!(question.len(), 1, "wi-2 stands at exactly one decision");
    assert_eq!((question[0].kind.as_str(), question[0].group.as_str()), ("question", "answer"));
    let build = &rt.store.world.sessions["unit/atlas/status-writer/wi-2/build"];
    assert!(build.pane, "the blocked session stays alive for its answer");
    assert_eq!(build.question.as_deref(), Some("two readings of one-writer sc.2 — which?"));
    assert!(!decisions.iter().any(|d| d.object == unit), "the approved proposal is no longer a decision");
    assert!(!decisions.iter().any(|d| d.object == wi1));

    let merged: Vec<_> = rt.store.tail.iter().filter(|t| t.object == wi1 && t.kind == "merged").collect();
    assert_eq!(merged.len(), 1, "the tail records wi-1's merge once");
    assert_eq!(merged[0].state, "merged");

    // Numbers already given never move; decisions still standing keep theirs.
    for (id, n) in &given_before {
        assert_eq!(rt.store.register.numbers.get(id), Some(n), "number {n} for {id} changed");
    }
    for (id, n) in &standing_before {
        let now = decisions.iter().find(|d| &d.id == id).unwrap_or_else(|| panic!("{id} no longer stands"));
        assert_eq!(now.number, Some(*n));
    }
    assert!(question[0].number.unwrap() > given_before.values().copied().max().unwrap(), "the new question gets a fresh number");
    assert_eq!(rt.store.responses.len(), 1);
    assert!(rt.store.objects[unit].applied_responses.contains(&rt.store.responses[0].id));
}

#[test]
fn dropping_a_unit_retires_its_items_and_ends_their_sessions() {
    let mut rt = seeded();
    let unit = "unit/atlas/plan-tail";
    let items = ["unit/atlas/plan-tail/wi-1", "unit/atlas/plan-tail/wi-2"];
    let sessions = ["unit/atlas/plan-tail/wi-1/build", "unit/atlas/plan-tail/wi-2/build"];
    assert_eq!(state(&rt, unit).as_deref(), Some("in-flight"));
    for s in sessions {
        assert!(rt.store.world.sessions[s].pane, "{s} runs before the drop");
    }
    let other_before = rt.store.world.sessions["elaboration/plan-derivation/prototype/work"].pane;

    rt.dictate(unit, "drop", "test");
    run_until_quiet(&mut rt, 100, 5);

    assert_eq!(state(&rt, unit).as_deref(), Some("dropped"));
    for id in items {
        let o = &rt.store.objects[id];
        assert_eq!(o.top_state(), Some("dropped"), "{id}");
        assert_eq!(o.config.get("place.place.life").map(String::as_str), Some("removed"), "{id}'s place is released");
        assert!(!o.config.keys().any(|k| k.starts_with("life.in-type")), "{id} left its stages");
    }
    for s in sessions {
        assert!(!rt.store.world.sessions[s].pane, "{s} was ended");
    }
    assert_eq!(rt.store.world.sessions["elaboration/plan-derivation/prototype/work"].pane, other_before, "an unrelated session is untouched");

    let dropped: Vec<_> = rt.store.tail.iter().filter(|t| t.kind == "dropped" && t.object.starts_with(unit)).map(|t| t.object.clone()).collect();
    assert!(dropped.contains(&unit.to_string()));
    for id in items { assert!(dropped.contains(&id.to_string()), "{id} in the tail"); }
    assert!(rt.decisions().iter().all(|d| !d.object.starts_with(unit)), "no decision stands on dropped work");
}
