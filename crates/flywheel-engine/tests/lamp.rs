//! A toy machine driven end to end through the engine: definitions loaded from
//! inline YAML, evidence from a hand-written source, responses through the
//! register, decisions derived by `plan`.

use chrono::{DateTime, Duration, TimeZone, Utc};
use flywheel_engine::defs::{Atoms, Definitions, Guard, Machine, State};
use flywheel_engine::eval::{self, Ctx};
use flywheel_engine::runtime::{EvidenceSource, Object, Register, Response, ResponseKind, Snapshot};
use flywheel_engine::{apply, initialise, rail, plan_tick, Fired};
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
        decision: {kind: switch, group: decide, answers: ["on"]}
        transitions:
          - when: {response: "on"}
            to: "on"
      "on":
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
effects:
  light: {of: lamp, args: [], proof: lamp.lit}
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
    let mut o = Object {
        id: "lamp/1".into(),
        machine: "lamp".into(),
        parent: None,
        config: BTreeMap::new(),
        entered_at: BTreeMap::new(),
        record: BTreeMap::new(),
        counters: BTreeMap::new(),
        applied_responses: vec![],
        seq: 0,
        created: 1,
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

    let decisions = rail::derive(&d, &objects, &mut register);
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
    let number = rail::derive(&d, &objects, &mut register)[0].number.unwrap();

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
    let number = rail::derive(&d, &objects, &mut register)[0].number.unwrap();
    let mut responses = vec![answer("r1", number, "on", t0())];
    tick(&d, &mut objects, &responses, &register, &facts, t0());

    // The nested region reads its own evidence.
    facts.set("lamp/1", "lamp.level", json!(7));
    let fired = tick(&d, &mut objects, &responses, &register, &facts, t0() + Duration::minutes(1));
    assert_eq!(fired.len(), 1);
    assert_eq!((fired[0].region.as_str(), fired[0].to.as_str()), ("power.on.level", "bright"));

    // A dictation naming the object turns it off; the nested region goes with the state.
    responses.push(dictation("r2", "lamp/1", "off", t0()));
    let fired = tick(&d, &mut objects, &responses, &register, &facts, t0() + Duration::minutes(2));
    assert_eq!(fired.len(), 1, "{fired:?}");
    assert_eq!(fired[0].to, "off");
    let o = &objects["lamp/1"];
    assert_eq!(o.config.get("power").map(String::as_str), Some("off"));
    assert!(!o.config.contains_key("power.on.level"));
    assert_eq!(o.applied_responses, vec!["r1".to_string(), "r2".to_string()]);

    // Back on, then a fault turns it off with no response at all.
    let number2 = rail::derive(&d, &objects, &mut register).last().unwrap().number.unwrap();
    responses.push(answer("r3", number2, "on", t0()));
    tick(&d, &mut objects, &responses, &register, &facts, t0() + Duration::minutes(3));
    assert_eq!(objects["lamp/1"].config.get("power").map(String::as_str), Some("on"));
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

    let first = rail::derive(&d, &objects, &mut register);
    assert_eq!(first.len(), 1);
    let one = &first[0];
    assert_eq!((one.object.as_str(), one.kind.as_str(), one.state.as_str(), one.group.as_str()), ("lamp/1", "switch", "off", "decide"));
    assert_eq!(one.answers, vec!["on".to_string()]);
    let number = one.number.expect("numbered");

    // Ticks that change nothing keep the number.
    for i in 1..4 {
        tick(&d, &mut objects, &[], &register, &facts, t0() + Duration::minutes(i));
        let again = rail::derive(&d, &objects, &mut register);
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].number, Some(number));
        assert_eq!(again[0].id, one.id);
    }

    // Leave `off`: no decision stands.
    let mut responses = vec![answer("r1", number, "on", t0())];
    tick(&d, &mut objects, &responses, &register, &facts, t0() + Duration::minutes(5));
    assert!(rail::derive(&d, &objects, &mut register).is_empty());

    // Re-enter `off` later: a new decision with a new number; the old number is never reused.
    responses.push(dictation("r2", "lamp/1", "off", t0()));
    tick(&d, &mut objects, &responses, &register, &facts, t0() + Duration::minutes(6));
    let third = rail::derive(&d, &objects, &mut register);
    assert_eq!(third.len(), 1);
    assert_ne!(third[0].id, one.id);
    assert_eq!(third[0].number, Some(number + 1));
    assert_eq!(register.decision_of(number), Some(one.id.as_str()), "the register remembers the first decision");
}
