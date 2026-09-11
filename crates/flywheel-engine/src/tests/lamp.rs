//! A toy machine driven end to end through the engine: definitions loaded from
//! inline YAML, evidence from a hand-written source, responses through the
//! register, decisions derived by `plan`.

use chrono::{DateTime, Duration, TimeZone, Utc};
use crate::defs::{Atoms, Definitions, Guard, Machine, State};
use crate::eval::{self, Ctx};
use crate::runtime::{DecisionInstance, EvidenceSource, Object, Register, Response, ResponseKind, Snapshot};
use crate::{apply, initialise, rail, plan_tick, Fired};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const LAMP: &str = r#"
machine: lamp
version: 1
kind: object
object: lamp
regions:
  power:
    initial: "off"
    states:
      "off":
        decision: {kind: switch, group: decide, answers: ["on", "mode <name>"], batch: fixture}
        transitions:
          - when: {response: "mode <name>"}
            to: "off"
            effects: [{do: set_mode, args: {mode: $response}}]
          - when: {response: "on"}
            to: "on"
          - when: {ev: lamp.forced, is: true}
            to: "on"
      "on":
        tail: done
        entry: [{do: light}]
        regions:
          level:
            initial: dim
            states:
              dim:
                transitions:
                  - when: {ev: lamp.level, gte: 5}
                    to: bright
              bright: {}
        transitions:
          - when: {any: [{ev: lamp.faulty, is: true}, {response: "off"}]}
            to: "off"
"#;

const ATOMS: &str = r#"
evidence:
  lamp.lit: {of: lamp, type: bool, proof_of: light}
  lamp.level: {of: lamp, type: int}
  lamp.faulty: {of: lamp, type: bool}
  lamp.forced: {of: lamp, type: bool}
effects:
  light: {of: lamp, args: [], proof: lamp.lit}
  set_mode: {of: lamp, args: [mode]}
"#;

/// Evidence by `<object>/<name>`.
#[derive(Default)]
struct Facts(BTreeMap<String, Value>);

impl Facts {
    fn set(&mut self, object: &str, name: &str, v: Value) {
        self.0.insert(format!("{object}/{name}"), v);
    }
}

impl EvidenceSource for Facts {
    fn evidence(&self, object: &str, _region: &str, name: &str) -> Option<Value> {
        self.0.get(&format!("{object}/{name}")).cloned()
    }
}

fn defs() -> Definitions {
    let m: Machine = serde_yaml::from_str(LAMP).expect("lamp machine parses");
    let atoms: Atoms = serde_yaml::from_str(ATOMS).expect("atoms parse");
    let mut d = Definitions { atoms, ..Default::default() };
    d.machines.insert(m.machine.clone(), m);
    d
}

fn t0() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 4, 12, 0, 0).unwrap()
}

fn lamp(defs: &Definitions, now: DateTime<Utc>) -> Object {
    named_lamp(defs, "lamp/1", None, 1, now)
}

/// One lamp, with the fixture its decision folds by where it has one (11).
fn named_lamp(
    defs: &Definitions,
    id: &str,
    fixture: Option<&str>,
    created: u64,
    now: DateTime<Utc>,
) -> Object {
    let mut record = BTreeMap::new();
    if let Some(fixture) = fixture {
        record.insert("fixture".to_string(), json!(fixture));
    }
    let mut o = Object {
        id: id.into(),
        machine: "lamp".into(),
        parent: None,
        config: BTreeMap::new(),
        entered_at: BTreeMap::new(),
        record,
        counters: BTreeMap::new(),
        applied_responses: vec![],
        seq: 0,
        created,
    };
    initialise(defs, &mut o, now);
    o
}

fn answer(id: &str, number: u32, text: &str, at: DateTime<Utc>) -> Response {
    Response { id: id.into(), kind: ResponseKind::Answer, decision: Some(number), object: None, answer: text.into(), given_by: "test".into(), given_at: at, delivery: "test".into() }
}

fn dictation(id: &str, object: &str, text: &str, at: DateTime<Utc>) -> Response {
    Response { id: id.into(), kind: ResponseKind::Dictation, decision: None, object: Some(object.into()), answer: text.into(), given_by: "test".into(), given_at: at, delivery: "test".into() }
}

/// The rail as a tick leaves it: derive, number what stands with no entry,
/// retract every entry whose decision is gone, and derive again over the
/// register the numbering wrote. This is what the rail machine's own numbering
/// act does, and what the tick does when a decision state is left (9, 15, I3).
fn rail_of(
    defs: &Definitions,
    objects: &BTreeMap<String, Object>,
    register: &mut Register,
    now: DateTime<Utc>,
) -> Vec<DecisionInstance> {
    let standing = rail::derive(defs, objects, register);
    register.number_all(&standing);
    let ids: Vec<String> = standing.iter().map(|d| d.id.clone()).collect();
    register.retract_gone(&ids, now);
    rail::derive(defs, objects, register)
}

/// One tick: plan against the snapshot, apply every fired transition. Returns what fired.
fn tick(defs: &Definitions, objects: &mut BTreeMap<String, Object>, responses: &[Response], register: &Register, facts: &Facts, now: DateTime<Utc>) -> Vec<Fired> {
    let fired = {
        let snap = Snapshot { objects, responses, register, evidence: facts, now };
        plan_tick(defs, &snap)
    };
    for f in &fired {
        let o = objects.get_mut(&f.object).expect("fired object exists");
        apply(defs, o, f, now);
    }
    fired
}

#[test]
fn initial_configuration_and_nested_region() {
    let d = defs();
    let o = lamp(&d, t0());
    assert_eq!(o.config.get("power").map(String::as_str), Some("off"));
    assert!(!o.config.contains_key("power.on.level"), "the nested region lives inside `on`, not `off`");

    // Seeded straight into `on`, initialise fills in the nested region under it.
    let mut seeded = o.clone();
    seeded.config.insert("power".into(), "on".into());
    initialise(&d, &mut seeded, t0());
    assert_eq!(seeded.config.get("power.on.level").map(String::as_str), Some("dim"));
    assert_eq!(seeded.entered_at.get("power.on.level"), Some(&t0()));
}

#[test]
fn response_fires_once_and_is_recorded() {
    let d = defs();
    let facts = Facts::default();
    let mut objects = BTreeMap::new();
    objects.insert("lamp/1".to_string(), lamp(&d, t0()));
    let mut register = Register::default();

    let decisions = rail_of(&d, &objects, &mut register, t0());
    assert_eq!(decisions.len(), 1);
    let number = decisions[0].number.expect("numbered");

    // A response to some other number is not this object's.
    let wrong = vec![answer("r0", number + 7, "on", t0())];
    let fired = tick(&d, &mut objects, &wrong, &register, &facts, t0());
    assert!(fired.is_empty(), "a response to a number the register does not know fires nothing");

    let responses = vec![answer("r1", number, "on", t0())];
    let now = t0() + Duration::minutes(1);
    let fired = tick(&d, &mut objects, &responses, &register, &facts, now);
    assert_eq!(fired.len(), 1);
    let f = &fired[0];
    assert_eq!((f.region.as_str(), f.from.as_str(), f.to.as_str()), ("power", "off", "on"));
    assert_eq!(f.response.as_ref().map(|(id, arg)| (id.as_str(), arg.as_str())), Some(("r1", "")));
    assert_eq!(f.effects.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), vec!["light"], "the entry effect of `on` is planned when unproven");

    let o = &objects["lamp/1"];
    assert_eq!(o.config.get("power").map(String::as_str), Some("on"));
    assert_eq!(o.config.get("power.on.level").map(String::as_str), Some("dim"), "entering `on` initialises its nested region");
    assert_eq!(o.applied_responses, vec!["r1".to_string()]);
    assert_eq!(o.seq, 1);

    // The same responses again: the response is applied, nothing else holds, nothing fires.
    let again = tick(&d, &mut objects, &responses, &register, &facts, now + Duration::minutes(1));
    assert!(again.is_empty(), "a repeated tick with nothing changed fires nothing");
    assert_eq!(objects["lamp/1"].seq, 1);
}

#[test]
fn proof_present_suppresses_the_effect() {
    let d = defs();
    let mut facts = Facts::default();
    facts.set("lamp/1", "lamp.lit", json!(true));
    let mut objects = BTreeMap::new();
    objects.insert("lamp/1".to_string(), lamp(&d, t0()));
    let mut register = Register::default();
    let number = rail_of(&d, &objects, &mut register, t0())[0].number.unwrap();

    let responses = vec![answer("r1", number, "on", t0())];
    let fired = tick(&d, &mut objects, &responses, &register, &facts, t0());
    assert_eq!(fired.len(), 1);
    assert!(fired[0].effects.is_empty(), "`light` is left out while its proof `lamp.lit` holds");
    assert_eq!(objects["lamp/1"].config.get("power").map(String::as_str), Some("on"));
}

#[test]
fn evidence_and_dictation_leave_on() {
    let d = defs();
    let mut facts = Facts::default();
    let mut objects = BTreeMap::new();
    objects.insert("lamp/1".to_string(), lamp(&d, t0()));
    let mut register = Register::default();
    let number = rail_of(&d, &objects, &mut register, t0())[0].number.unwrap();
    let mut responses = vec![answer("r1", number, "on", t0())];
    tick(&d, &mut objects, &responses, &register, &facts, t0());

    // The nested region reads its own evidence.
    facts.set("lamp/1", "lamp.level", json!(7));
    let fired = tick(&d, &mut objects, &responses, &register, &facts, t0() + Duration::minutes(1));
    assert_eq!(fired.len(), 1);
    assert_eq!((fired[0].region.as_str(), fired[0].to.as_str()), ("power.on.level", "bright"));

    // A dictation naming the object turns it off; what the state ran stays.
    responses.push(dictation("r2", "lamp/1", "off", t0()));
    let fired = tick(&d, &mut objects, &responses, &register, &facts, t0() + Duration::minutes(2));
    assert_eq!(fired.len(), 1, "{fired:?}");
    assert_eq!(fired[0].to, "off");
    let o = &objects["lamp/1"];
    assert_eq!(o.config.get("power").map(String::as_str), Some("off"));
    // Leaving `on` neither ends nor clears what it instantiated: the level
    // stays readable at its dotted path, holding the state it reached, for a
    // later state to guard on or command (model.md §1).
    assert_eq!(o.config.get("power.on.level").map(String::as_str), Some("bright"));
    assert_eq!(o.applied_responses, vec!["r1".to_string(), "r2".to_string()]);

    // Back on, then a fault turns it off with no response at all.
    let number2 = rail_of(&d, &objects, &mut register, t0() + Duration::minutes(2))
        .last()
        .unwrap()
        .number
        .unwrap();
    responses.push(answer("r3", number2, "on", t0()));
    tick(&d, &mut objects, &responses, &register, &facts, t0() + Duration::minutes(3));
    assert_eq!(objects["lamp/1"].config.get("power").map(String::as_str), Some("on"));
    // Entering `on` again instantiates it afresh, and the instance that was
    // there is set aside in the record under its attempt, its last state
    // readable there (model.md §1).
    assert_eq!(objects["lamp/1"].config.get("power.on.level").map(String::as_str), Some("dim"));
    assert_eq!(objects["lamp/1"].record["prior"]["power.on#1"]["level"], json!("bright"));
    facts.set("lamp/1", "lamp.faulty", json!(true));
    let fired = tick(&d, &mut objects, &responses, &register, &facts, t0() + Duration::minutes(4));
    // Re-entering `on` restarted the level at dim; with lamp.level still 7 it goes bright in the
    // same tick as the fault, innermost region first.
    assert_eq!(fired.iter().map(|f| (f.region.as_str(), f.to.as_str())).collect::<Vec<_>>(), vec![("power.on.level", "bright"), ("power", "off")]);
    assert!(fired[1].response.is_none(), "the evidence branch of `any` consumes no response");
    assert_eq!(objects["lamp/1"].config.get("power").map(String::as_str), Some("off"));
    assert_eq!(objects["lamp/1"].applied_responses.len(), 3);
}

fn holds(guard: &str, o: &Object, facts: &Facts, now: DateTime<Utc>) -> bool {
    let d = defs();
    let g: Guard = serde_yaml::from_str(guard).expect("guard parses");
    let objects: BTreeMap<String, Object> = [(o.id.clone(), o.clone())].into_iter().collect();
    let register = Register::default();
    let snap = Snapshot { objects: &objects, responses: &[], register: &register, evidence: facts, now };
    let st = State::default();
    let cx = Ctx { defs: &d, snap: &snap, object: o, region: "power", state_name: "off", state: &st };
    eval::eval(&g, &cx).holds
}

#[test]
fn older_guard_compares_entered_at_with_now() {
    let d = defs();
    let o = lamp(&d, t0());
    let facts = Facts::default();
    assert!(!holds("{ev: entered_at, older: 10m}", &o, &facts, t0() + Duration::minutes(5)));
    assert!(!holds("{ev: entered_at, older: 10m}", &o, &facts, t0() + Duration::minutes(10)), "older is strict");
    assert!(holds("{ev: entered_at, older: 10m}", &o, &facts, t0() + Duration::minutes(11)));
    // Any timestamp evidence works the same way; a missing one never holds.
    let mut f = Facts::default();
    f.set("lamp/1", "lamp.since", json!((t0() - Duration::days(2)).to_rfc3339()));
    assert!(holds("{ev: lamp.since, older: 1d}", &o, &f, t0()));
    assert!(!holds("{ev: lamp.since, older: 3d}", &o, &f, t0()));
    assert!(!holds("{ev: lamp.absent, older: 1s}", &o, &f, t0()));
}

#[test]
fn in_guard_matches_any_listed_value() {
    let d = defs();
    let o = lamp(&d, t0());
    let mut f = Facts::default();
    f.set("lamp/1", "lamp.mode", json!("warm"));
    f.set("lamp/1", "lamp.level", json!(3));
    assert!(holds("{ev: lamp.mode, in: [cool, warm]}", &o, &f, t0()));
    assert!(!holds("{ev: lamp.mode, in: [cool, daylight]}", &o, &f, t0()));
    assert!(holds("{ev: lamp.level, in: [1, 3, 5]}", &o, &f, t0()));
    assert!(holds("{ev: lamp.level, in: ['3']}", &o, &f, t0()), "a number matches its own text");
    assert!(!holds("{ev: lamp.level, in: [2, 4]}", &o, &f, t0()));
    assert!(!holds("{ev: lamp.missing, in: [anything]}", &o, &f, t0()), "absent evidence is in no set");
    // Engine-provided evidence is read the same way.
    assert!(holds("{ev: state, in: ['off', broken]}", &o, &f, t0()));
    assert!(!holds("{ev: state, in: ['on']}", &o, &f, t0()));
}

#[test]
fn eq_ev_compares_two_evidence_names() {
    let d = defs();
    let o = lamp(&d, t0());
    let mut f = Facts::default();
    f.set("lamp/1", "lamp.level", json!(5));
    f.set("lamp/1", "lamp.target", json!(5));
    f.set("lamp/1", "lamp.other", json!(6));
    assert!(holds("{ev: lamp.level, eq_ev: lamp.target}", &o, &f, t0()));
    assert!(!holds("{ev: lamp.level, eq_ev: lamp.other}", &o, &f, t0()));
    assert!(!holds("{ev: lamp.level, eq_ev: lamp.missing}", &o, &f, t0()), "a missing right-hand side never equals");
    assert!(!holds("{ev: lamp.missing, eq_ev: lamp.level}", &o, &f, t0()), "a missing left-hand side never equals");
    assert!(holds("{ev: lamp.level, ne_ev: lamp.other}", &o, &f, t0()));
    assert!(holds("{ev: lamp.other, gt_ev: lamp.level}", &o, &f, t0()));
    assert!(holds("{ev: lamp.level, lt_ev: lamp.other}", &o, &f, t0()));
    assert!(holds("{ev: lamp.level, gte_ev: lamp.target}", &o, &f, t0()));
}

#[test]
fn decision_number_is_stable_until_the_state_is_re_entered() {
    let d = defs();
    let facts = Facts::default();
    let mut objects = BTreeMap::new();
    objects.insert("lamp/1".to_string(), lamp(&d, t0()));
    let mut register = Register::default();

    let first = rail_of(&d, &objects, &mut register, t0());
    assert_eq!(first.len(), 1);
    let one = &first[0];
    assert_eq!((one.object.as_str(), one.kind.as_str(), one.state.as_str(), one.group.as_str()), ("lamp/1", "switch", "off", "decide"));
    assert_eq!(one.answers, vec!["on".to_string(), "mode <name>".to_string()]);
    let number = one.number.expect("numbered");

    // Ticks that change nothing keep the number.
    for i in 1..4 {
        tick(&d, &mut objects, &[], &register, &facts, t0() + Duration::minutes(i));
        let again = rail_of(&d, &objects, &mut register, t0() + Duration::minutes(i));
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].number, Some(number));
        assert_eq!(again[0].id, one.id);
    }

    // Leave `off`: no decision stands.
    let mut responses = vec![answer("r1", number, "on", t0())];
    tick(&d, &mut objects, &responses, &register, &facts, t0() + Duration::minutes(5));
    assert!(rail_of(&d, &objects, &mut register, t0() + Duration::minutes(5)).is_empty());

    // Re-enter `off` later: a new decision with a new number; the old number is never reused.
    responses.push(dictation("r2", "lamp/1", "off", t0()));
    tick(&d, &mut objects, &responses, &register, &facts, t0() + Duration::minutes(6));
    let third = rail_of(&d, &objects, &mut register, t0() + Duration::minutes(6));
    assert_eq!(third.len(), 1);
    assert_ne!(third[0].id, one.id);
    assert_eq!(third[0].number, Some(number + 1));
    assert_eq!(register.decision_of(number), Some(one.id.as_str()), "the register remembers the first decision");
}

#[test]
fn decision_created_and_retracted() {
    let d = defs();
    let mut facts = Facts::default();
    let mut objects = BTreeMap::new();
    objects.insert("lamp/1".to_string(), lamp(&d, t0()));
    let mut register = Register::default();

    // Created by entering the state: nothing wrote it, and it stands because
    // the state is active (9, I3).
    let standing = rail_of(&d, &objects, &mut register, t0());
    assert_eq!(standing.len(), 1);
    let one = standing[0].clone();
    let number = one.number.expect("numbered on the tick it was raised");
    let entry = register.entries.get(&one.id).expect("an entry of its own");
    assert_eq!(entry.number, number);
    assert_eq!(entry.since, Some(one.since));
    assert_eq!(entry.retracted_at, None, "it stands, so it is not retracted");
    assert_eq!(entry.answered_by, None);

    // Retracted by the operator's response: the entry keeps who answered it
    // and when, so a second response to that number is refused as already
    // answered (9, 15).
    let at = t0() + Duration::minutes(1);
    let responses = vec![answer("r1", number, "on", at)];
    tick(&d, &mut objects, &responses, &register, &facts, at);
    register.answered(&one.id, "r1", at);
    assert!(rail_of(&d, &objects, &mut register, at).is_empty());
    let entry = register.entries.get(&one.id).expect("the entry outlives the decision");
    assert_eq!(entry.retracted_at, Some(at));
    assert_eq!(entry.answered_by.as_deref(), Some("r1"));
    assert_eq!(entry.answered_at, Some(at));

    // Retracted by the machinery: the choice is no longer the operator's, no
    // response took it away, and the entry says so (9, I3).
    let back = t0() + Duration::minutes(2);
    let mut responses = responses;
    responses.push(dictation("r2", "lamp/1", "off", back));
    tick(&d, &mut objects, &responses, &register, &facts, back);
    let again = rail_of(&d, &objects, &mut register, back);
    assert_eq!(again.len(), 1, "re-entering `off` raises a decision again");
    let two = again[0].clone();
    assert_ne!(two.id, one.id);

    let forced = t0() + Duration::minutes(3);
    facts.set("lamp/1", "lamp.forced", json!(true));
    tick(&d, &mut objects, &responses, &register, &facts, forced);
    assert!(rail_of(&d, &objects, &mut register, forced).is_empty());
    let entry = register.entries.get(&two.id).expect("its entry stands");
    assert_eq!(entry.retracted_at, Some(forced));
    assert_eq!(
        entry.answered_by, None,
        "retracted by the machinery: no response answered it"
    );
}

#[test]
fn number_never_reused() {
    let d = defs();
    let facts = Facts::default();
    let mut objects = BTreeMap::new();
    objects.insert("lamp/1".to_string(), lamp(&d, t0()));
    let mut register = Register::default();

    let first = rail_of(&d, &objects, &mut register, t0())[0].clone();
    let number = first.number.expect("numbered");

    // Leave and re-enter: a new decision with a new number, and the old number
    // still resolves to the old decision, never to the new one (15).
    let mut responses = vec![answer("r1", number, "on", t0())];
    tick(&d, &mut objects, &responses, &register, &facts, t0() + Duration::minutes(1));
    responses.push(dictation("r2", "lamp/1", "off", t0()));
    tick(&d, &mut objects, &responses, &register, &facts, t0() + Duration::minutes(2));
    let second = rail_of(&d, &objects, &mut register, t0() + Duration::minutes(2))[0].clone();

    assert_ne!(second.id, first.id, "a re-entered state is a new decision");
    assert_eq!(second.number, Some(number + 1));
    assert_eq!(register.decision_of(number), Some(first.id.as_str()));
    assert_eq!(register.decision_of(number + 1), Some(second.id.as_str()));

    // The counter only grows, and one numbering is one write of it: numbering
    // again with nothing new to number changes nothing (15).
    let next = register.next_number;
    let standing = rail::derive(&d, &objects, &register);
    assert!(!register.unnumbered(&standing));
    assert!(!register.number_all(&standing), "nothing left to number");
    assert_eq!(register.next_number, next);
}

#[test]
fn grouping_and_single_answer() {
    let d = defs();
    let facts = Facts::default();
    let mut objects = BTreeMap::new();
    for n in 1..=6u64 {
        let id = format!("lamp/{n}");
        objects.insert(id.clone(), named_lamp(&d, &id, None, n, t0()));
    }
    let mut register = Register::default();
    let standing = rail_of(&d, &objects, &mut register, t0());
    assert_eq!(standing.len(), 6);
    assert!(standing.iter().all(|x| x.group == "decide"), "one group, so yes to all is meaningful");
    let numbers: Vec<u32> = standing.iter().map(|x| x.number.expect("numbered")).collect();

    // One decision answered on its own: it applies and the other five stand
    // unchanged (11).
    let at = t0() + Duration::minutes(1);
    let alone = vec![answer("r-alone", numbers[2], "on", at)];
    let fired = tick(&d, &mut objects, &alone, &register, &facts, at);
    assert_eq!(fired.len(), 1);
    assert_eq!(fired[0].object, "lamp/3");
    let after = rail_of(&d, &objects, &mut register, at);
    assert_eq!(after.len(), 5);
    assert!(!after.iter().any(|x| x.object == "lamp/3"));
    assert_eq!(
        after.iter().filter_map(|x| x.number).collect::<Vec<_>>(),
        numbers
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 2)
            .map(|(_, n)| *n)
            .collect::<Vec<_>>(),
        "the five keep the numbers they had"
    );

    // Yes to all: one response per decision, each with its own delivery
    // identity, each applied exactly once (11, 137).
    let all: Vec<Response> = after
        .iter()
        .map(|x| answer(&format!("yes-all/{}", x.object), x.number.unwrap(), "on", at))
        .collect();
    let at = t0() + Duration::minutes(2);
    let fired = tick(&d, &mut objects, &all, &register, &facts, at);
    assert_eq!(fired.len(), 5, "one transition per decision");
    for object in objects.values() {
        assert_eq!(object.config.get("power").map(String::as_str), Some("on"));
        assert_eq!(object.applied_responses.len(), 1, "each applied exactly once");
    }
    assert!(rail_of(&d, &objects, &mut register, at).is_empty());
    let again = tick(&d, &mut objects, &all, &register, &facts, at + Duration::minutes(1));
    assert!(again.is_empty(), "the same deliveries again ask nothing");
}

#[test]
fn new_material_joins_the_proposal() {
    let d = defs();
    let facts = Facts::default();
    let mut objects = BTreeMap::new();
    objects.insert("lamp/1".into(), named_lamp(&d, "lamp/1", Some("hall"), 1, t0()));
    let mut register = Register::default();

    let one = rail_of(&d, &objects, &mut register, t0());
    assert_eq!(one.len(), 1);
    let number = one[0].number.expect("numbered");
    assert_eq!(one[0].folds, vec!["lamp/1".to_string()]);

    // A second lamp of the same fixture is new material for the standing
    // decision: it joins it, and one decision stands, not two (11, 21).
    let at = t0() + Duration::minutes(1);
    objects.insert("lamp/2".into(), named_lamp(&d, "lamp/2", Some("hall"), 2, at));
    let joined = rail_of(&d, &objects, &mut register, at);
    assert_eq!(joined.len(), 1, "one decision on the fixture, not two");
    assert_eq!(joined[0].number, Some(number), "and it keeps the number it had");
    assert_eq!(joined[0].folds, vec!["lamp/1".to_string(), "lamp/2".to_string()]);
    assert_eq!(register.next_number, number + 1, "no second number was given");

    // One answer to that number applies to every object folded under it (11).
    let responses = vec![answer("r1", number, "on", at)];
    let fired = tick(&d, &mut objects, &responses, &register, &facts, at);
    assert_eq!(fired.len(), 2, "{fired:?}");
    for object in objects.values() {
        assert_eq!(object.config.get("power").map(String::as_str), Some("on"));
        assert_eq!(object.applied_responses, vec!["r1".to_string()]);
    }
    assert!(rail_of(&d, &objects, &mut register, at).is_empty());

    // A lamp of another fixture is a decision of its own.
    let at = t0() + Duration::minutes(2);
    objects.insert("lamp/3".into(), named_lamp(&d, "lamp/3", Some("porch"), 3, at));
    let other = rail_of(&d, &objects, &mut register, at);
    assert_eq!(other.len(), 1);
    assert_eq!(other[0].object, "lamp/3");
    assert_eq!(other[0].number, Some(number + 1));
}

#[test]
fn response_corrects_the_type() {
    let d = defs();
    let facts = Facts::default();
    let mut objects = BTreeMap::new();
    objects.insert("lamp/1".to_string(), lamp(&d, t0()));
    let mut register = Register::default();
    let standing = rail_of(&d, &objects, &mut register, t0());
    let number = standing[0].number.expect("numbered");

    // The answer names another mode. The decision is kept — the state is
    // re-entered, not left — and the act carries what the response named (27).
    let at = t0() + Duration::minutes(1);
    let responses = vec![answer("r1", number, "mode warm", at)];
    let fired = tick(&d, &mut objects, &responses, &register, &facts, at);
    assert_eq!(fired.len(), 1);
    let f = &fired[0];
    assert_eq!((f.from.as_str(), f.to.as_str()), ("off", "off"));
    assert_eq!(f.response.as_ref().map(|(id, arg)| (id.as_str(), arg.as_str())), Some(("r1", "warm")));
    assert_eq!(f.effects.len(), 1);
    assert_eq!(f.effects[0].name, "set_mode");
    assert_eq!(f.effects[0].args.get("mode"), Some(&json!("warm")));

    // The decision still stands, under the number it had: correcting the type
    // is not answering the approval (15, 27).
    let after = rail_of(&d, &objects, &mut register, at);
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].number, Some(number));
    assert_eq!(objects["lamp/1"].applied_responses, vec!["r1".to_string()]);
}

#[test]
fn tail_per_sink_mark() {
    let d = defs();
    let facts = Facts::default();
    let mut objects = BTreeMap::new();
    objects.insert("lamp/1".into(), named_lamp(&d, "lamp/1", None, 1, t0()));
    objects.insert("lamp/2".into(), named_lamp(&d, "lamp/2", None, 2, t0()));
    let mut register = Register::default();
    let standing = rail_of(&d, &objects, &mut register, t0());
    let numbers: BTreeMap<String, u32> = standing
        .iter()
        .map(|x| (x.object.clone(), x.number.expect("numbered")))
        .collect();

    // One reaches `done` early, the other later.
    let early = t0() + Duration::minutes(1);
    let late = t0() + Duration::minutes(5);
    let first = vec![answer("r1", numbers["lamp/1"], "on", early)];
    tick(&d, &mut objects, &first, &register, &facts, early);
    let mut both = first;
    both.push(answer("r2", numbers["lamp/2"], "on", late));
    tick(&d, &mut objects, &both, &register, &facts, late);

    // The page's mark is older than both; the chat's sits between them. Each
    // sink's tail is measured from its own mark, and the two differ (14, 15).
    let page = rail::tail(&d, &objects, t0());
    let chat = rail::tail(&d, &objects, t0() + Duration::minutes(2));
    assert_eq!(
        page.iter().map(|e| (e.object.as_str(), e.kind.as_str())).collect::<Vec<_>>(),
        vec![("lamp/1", "done"), ("lamp/2", "done")]
    );
    assert_eq!(
        chat.iter().map(|e| (e.object.as_str(), e.kind.as_str())).collect::<Vec<_>>(),
        vec![("lamp/2", "done")]
    );
    assert_ne!(page.len(), chat.len(), "the page's tail and the chat's are not one rendering");
    assert_eq!(page[0].by.as_deref(), Some("r1"), "the tail says what took it there");

    // Nothing is stored: the same call over the same state is the same tail,
    // and a sink whose mark is newer than everything has none (14, 15).
    assert_eq!(rail::tail(&d, &objects, t0()).len(), page.len());
    assert!(rail::tail(&d, &objects, late + Duration::minutes(1)).is_empty());
}
