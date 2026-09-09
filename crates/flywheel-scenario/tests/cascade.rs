//! The rail mockup scenario driven through the stand-in: one approval cascades
//! into items, places, sessions and a merge; one dictation retires work.

use flywheel_scenario::{scenario, Runtime};
use std::path::{Path, PathBuf};

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("workspace root")
}

fn seeded() -> Runtime {
    let defs = flywheel_engine::load::load_dir(&workspace().join("definitions")).expect("definitions load");
    let sc = scenario::load(&workspace().join("scenarios/rail-mockup.yaml")).expect("scenario loads");
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
    let build = &rt.store.world.sessions["unit/atlas/status-writer/wi-2/build/1"];
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
    let unit = "unit/atlas/rail-tail";
    let items = ["unit/atlas/rail-tail/wi-1", "unit/atlas/rail-tail/wi-2"];
    let sessions = ["unit/atlas/rail-tail/wi-1/build/1", "unit/atlas/rail-tail/wi-2/build/1"];
    assert_eq!(state(&rt, unit).as_deref(), Some("in-flight"));
    for s in sessions {
        assert!(rt.store.world.sessions[s].pane, "{s} runs before the drop");
    }
    let other_before = rt.store.world.sessions["elaboration/rail-derivation/prototype/work/1"].pane;

    rt.dictate(unit, "drop", "test");
    run_until_quiet(&mut rt, 100, 5);

    assert_eq!(state(&rt, unit).as_deref(), Some("dropped"));
    for id in items {
        let o = &rt.store.objects[id];
        assert_eq!(o.top_state(), Some("dropped"), "{id}");
        assert_eq!(o.config.get("place.place.life").map(String::as_str), Some("removed"), "{id}'s place is released");
        // Leaving `in-type` neither ends nor clears the stages it ran: they
        // stay readable at their dotted paths, holding what they reached when
        // the item was dropped (model.md §1).
        assert!(
            o.config.keys().any(|k| k.starts_with("life.in-type")),
            "{id}'s stages are no longer readable"
        );
    }
    for s in sessions {
        assert!(!rt.store.world.sessions[s].pane, "{s} was ended");
    }
    assert_eq!(rt.store.world.sessions["elaboration/rail-derivation/prototype/work/1"].pane, other_before, "an unrelated session is untouched");

    let dropped: Vec<_> = rt.store.tail.iter().filter(|t| t.kind == "dropped" && t.object.starts_with(unit)).map(|t| t.object.clone()).collect();
    assert!(dropped.contains(&unit.to_string()));
    for id in items { assert!(dropped.contains(&id.to_string()), "{id} in the tail"); }
    assert!(rt.decisions().iter().all(|d| !d.object.starts_with(unit)), "no decision stands on dropped work");
}

#[test]
fn services_start_stop_and_fail_by_dictation() {
    let mut rt = seeded();
    // The atlas repository declares two services; the open bolt's ready place declares them.
    rt.settle(50);
    let bolt = "bolt/atlas/plan-rows";
    let web = "service/atlas/plan-rows/web";
    let worker = "service/atlas/plan-rows/worker";
    assert_eq!(rt.store.objects[bolt].config.get("services").map(String::as_str), Some("declared"));
    for s in [web, worker] {
        assert_eq!(state(&rt, s).as_deref(), Some("stopped"), "{s} is declared in stopped");
        assert_eq!(rt.store.objects[s].parent.as_deref(), Some(bolt));
    }
    assert!(!rt.store.objects.values().any(|o| o.machine == "service" && o.id.contains("switchboard")), "a repository with no declarations gets no services");
    assert!(rt.decisions().iter().all(|d| !d.object.starts_with("service/")), "nothing is started until asked");

    // start: present after the effect, serving one tick later, endpoint recorded, no decision
    rt.dictate(web, "start", "test");
    run_until_quiet(&mut rt, 20, 3);
    assert_eq!(state(&rt, web).as_deref(), Some("running"));
    assert_eq!(rt.store.objects[web].record.get("endpoint").and_then(|v| v.as_str()), Some("http://atlas.plan-rows.localhost:41231"));
    assert_eq!(rt.store.objects[web].record.get("moved_by").and_then(|v| v.as_str()), Some(rt.store.responses[0].id.as_str()), "the dictation that started it is recorded");
    assert_eq!(state(&rt, worker).as_deref(), Some("stopped"), "the other service is untouched");
    assert!(rt.decisions().iter().all(|d| d.object != web));

    // stop: back to stopped, the endpoint gone with the process
    rt.dictate(web, "stop", "test");
    run_until_quiet(&mut rt, 20, 3);
    assert_eq!(state(&rt, web).as_deref(), Some("stopped"));
    assert!(rt.store.objects[web].record.get("endpoint").is_none());
    assert_eq!(rt.store.world.services[web].process, "absent");

    // a scripted failure: the worker's declaration says it exits instead of serving
    rt.store.world.declarations.get_mut("atlas").unwrap().iter_mut().find(|d| d.name == "worker").unwrap().fails = true;
    rt.dictate(worker, "start", "test");
    run_until_quiet(&mut rt, 20, 3);
    assert_eq!(state(&rt, worker).as_deref(), Some("failed"));
    let failed: Vec<_> = rt.decisions().into_iter().filter(|d| d.object == worker).collect();
    assert_eq!(failed.len(), 1, "one standing decision on the failed service");
    assert_eq!((failed[0].kind.as_str(), failed[0].group.as_str()), ("service-failed", "attention"));
    assert_eq!(failed[0].answers, vec!["start", "stop"]);
    assert!(rt.store.world.services[worker].failure.as_deref().unwrap_or("").starts_with("exit 1"));

    // stop clears the failure; the decision goes with the state
    rt.dictate(worker, "stop", "test");
    run_until_quiet(&mut rt, 20, 3);
    assert_eq!(state(&rt, worker).as_deref(), Some("stopped"));
    assert!(rt.decisions().iter().all(|d| d.object != worker));
}

#[test]
fn the_capture_box_makes_captures_and_marks_intents_with_a_control() {
    let mut rt = seeded();
    rt.settle(50);
    let before = rt.decisions().len();

    // typed text: a capture read into one signal of kind ask, no session, no decision
    let r = rt.capture("retries hammer the provider on 429", false, "test");
    run_until_quiet(&mut rt, 20, 3);
    let cap = r["id"].as_str().unwrap().to_string();
    assert_eq!(r["kind"], "capture");
    assert_eq!(state(&rt, &cap).as_deref(), Some("read"));
    let sig = rt.store.objects.values().find(|o| o.parent.as_deref() == Some(&cap)).expect("one signal under the capture");
    assert_eq!(sig.machine, "signal");
    assert_eq!(sig.record.get("kind").and_then(|v| v.as_str()), Some("ask"));
    assert_eq!(sig.top_state(), Some("unmoved"));
    assert_eq!(rt.store.objects[&cap].record["intent"], false);
    assert!(!rt.store.world.sessions.keys().any(|k| k.starts_with(&cap)), "a page capture needs no reader session");
    assert_eq!(rt.decisions().len(), before, "a capture is not a decision");

    // the control marks a capture as an intent: still a capture with one ask signal, flagged for curation
    let r = rt.capture("host liveness on the mac mini", true, "test");
    run_until_quiet(&mut rt, 20, 3);
    let marked = r["id"].as_str().unwrap().to_string();
    assert_eq!(r["kind"], "capture · intent");
    let o = &rt.store.objects[&marked];
    assert_eq!((o.machine.as_str(), o.top_state()), ("capture", Some("read")));
    assert_eq!(o.record["intent"], true);
    assert_eq!(rt.store.objects.values().filter(|s| s.parent.as_deref() == Some(&marked) && s.machine == "signal").count(), 1);
    assert!(rt.captured().iter().any(|c| c["id"] == marked && c["intent"] == true));

    // nothing in the text is parsed: prefixes are just words in a capture
    let before_objects = rt.store.objects.len();
    for text in ["intent: host liveness on the mac mini", "chore atlas: stale AGENTS.md", "bolt plan-rows: tail grouping by day"] {
        let r = rt.capture(text, false, "test");
        run_until_quiet(&mut rt, 20, 3);
        let id = r["id"].as_str().unwrap().to_string();
        assert_eq!((rt.store.objects[&id].machine.as_str(), rt.store.objects[&id].record["raw"].as_str()), ("capture", Some(text)));
    }
    assert!(!rt.store.objects.values().any(|o| o.id == "intent/host-liveness-on-the-mac-mini" || o.id.starts_with("unit/atlas/chore-")), "no prefix opens an intent or a unit");
    assert_eq!(rt.store.objects.len() - before_objects, 3 * 3, "each submission: one capture, one signal, one response");

    // each submission is one response, already applied; none stands as unapplicable; none is a decision
    assert_eq!(rt.store.responses.len(), 5);
    assert!(rt.store.objects.values().filter(|o| o.machine == "response" && o.id.starts_with("response/page-")).all(|o| o.top_state() == Some("applied")));
    assert_eq!(rt.decisions().len(), before);
}


#[test]
fn a_bolt_close_yes_lands_the_bolt() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let defs = flywheel_engine::load::load_dir(&root.join("definitions")).unwrap();
    let sc = flywheel_scenario::scenario::load(&root.join("scenarios/rail-mockup.yaml")).unwrap();
    let mut rt = flywheel_scenario::scenario::seed(defs, &sc);
    rt.settle(50);
    let d = rt.decisions().into_iter().find(|d| d.kind == "bolt-close" && d.object == "bolt/switchboard/plan-rows").expect("close offered");
    rt.respond(d.number.unwrap(), "yes", "test");
    let mut quiet = 0;
    for _ in 0..200 { if rt.tick() == 0 { quiet += 1; if quiet >= 5 { break; } } else { quiet = 0; } }
    let bolt = &rt.store.objects["bolt/switchboard/plan-rows"];
    assert_eq!(bolt.top_state(), Some("landed"), "config: {:?}", bolt.config);
    assert!(rt.store.tail.iter().any(|t| t.object == "bolt/switchboard/plan-rows" && t.kind == "landed"));
    assert!(!rt.decisions().iter().any(|d| d.kind == "bolt-close"), "the close decision retracts");
    assert!(rt.store.objects.values().filter(|o| o.machine == "response" && o.id.starts_with("response/page-")).all(|o| o.top_state() != Some("unapplicable")));
}
