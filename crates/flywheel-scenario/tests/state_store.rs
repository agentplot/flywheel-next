//! The stand-in state store answers the contract, not a convenience.

use chrono::Utc;
use flywheel_atoms::{
    EffectWrite, LeaseOp, LeaseOutcome, PutOutcome, Received, Records, Scope, StateStore,
    ThreadEntry, WriteOutcome,
};
use flywheel_engine::runtime::{Object, Response, ResponseKind};
use flywheel_scenario::Store;
use serde_json::json;

fn an_object(id: &str, machine: &str) -> Object {
    let mut o = Object {
        id: id.into(),
        machine: machine.into(),
        parent: None,
        config: Default::default(),
        entered_at: Default::default(),
        record: Default::default(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 1,
    };
    o.config.insert("life".into(), "proposed".into());
    o
}

#[test]
fn store_get_put_append_list() {
    let mut store = Store::default();
    let o = an_object("elaboration/a/1", "elaboration");

    // put on an absent object writes from base 0 and takes seq 1.
    assert_eq!(
        store.put(&o.id, &o, 0).unwrap(),
        PutOutcome::Written { seq: 1 }
    );
    let read = store.get(&o.id).unwrap().expect("the object is there");
    assert_eq!(read.seq, 1);

    // A second writer from the stale base loses and is told the held sequence
    // (134); the winner's write stands.
    assert_eq!(
        store.put(&o.id, &o, 0).unwrap(),
        PutOutcome::Rejected { held_seq: 1 }
    );
    assert_eq!(store.get(&o.id).unwrap().unwrap().seq, 1);
    assert_eq!(
        store.put(&o.id, &o, 1).unwrap(),
        PutOutcome::Written { seq: 2 }
    );

    // append adds to the object's thread, in order.
    let entry = |kind: &str| ThreadEntry {
        at: Utc::now(),
        kind: kind.into(),
        by: Some("chuck".into()),
        fields: Default::default(),
    };
    store.append(&o.id, &entry("note")).unwrap();
    store.append(&o.id, &entry("exit")).unwrap();
    let thread = store.thread(&o.id).unwrap();
    assert_eq!(
        thread.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        vec!["note", "exit"]
    );

    // list enumerates a scope, so an engine that remembers nothing is complete
    // (131).
    let other = an_object("intent/a", "intent");
    store.put(&other.id, &other, 0).unwrap();
    assert_eq!(store.list_records(&Scope::All).unwrap().len(), 2);
    assert_eq!(
        store
            .list_records(&Scope::Machine("intent".into()))
            .unwrap()
            .len(),
        1
    );
    let listing = store.list(&Scope::All).unwrap();
    assert_eq!(listing.objects.len(), 2);
    assert_eq!(listing.as_of.seq, store.writes);
}

#[test]
fn read_names_its_point_and_writes_nothing() {
    let mut store = Store::default();
    let o = an_object("elaboration/a/1", "elaboration");
    store.put(&o.id, &o, 0).unwrap();
    store.set_given(&o.id, "session.pane", json!("absent"));

    let first = store.read(&o.id).unwrap();
    let second = store.read(&o.id).unwrap();
    assert_eq!(first.as_of, second.as_of, "the read names its point (126)");
    assert_eq!(first.evidence, second.evidence, "and writes nothing (126)");
    assert_eq!(first.evidence.get("state").is_some(), true);
}

#[test]
fn an_effect_written_twice_is_one_write() {
    let mut store = Store::default();
    let write = EffectWrite {
        effect_id: "lamp/1/on/lamp.lit/abc".into(),
        object: "lamp/1".into(),
        effect: "light".into(),
        reason: "the switch is up".into(),
        evidence: Default::default(),
    };
    assert!(matches!(
        store.write_effect(&write).unwrap(),
        WriteOutcome::Written { .. }
    ));
    assert!(
        matches!(
            store.write_effect(&write).unwrap(),
            WriteOutcome::AlreadyWritten { .. }
        ),
        "a repeat is neither an error nor a second write (127)"
    );
    assert_eq!(store.effects_written.len(), 1);
}

#[test]
fn two_would_be_holders_one_lease() {
    let mut store = Store::default();
    let take = |who: &str| LeaseOp::Take {
        object: "unit/x".into(),
        holder: who.into(),
    };
    assert!(matches!(
        store.lease(&take("mac-mini")).unwrap(),
        LeaseOutcome::Held(_)
    ));
    match store.lease(&take("studio")).unwrap() {
        LeaseOutcome::HeldByAnother(held) => assert_eq!(held.holder, "mac-mini"),
        other => panic!("two holders of one lease: {other:?}"),
    }
    // The holder releases; the other may then take it (128).
    assert!(matches!(
        store
            .lease(&LeaseOp::Release {
                object: "unit/x".into(),
                holder: "mac-mini".into()
            })
            .unwrap(),
        LeaseOutcome::Released
    ));
    assert!(matches!(
        store.lease(&take("studio")).unwrap(),
        LeaseOutcome::Held(_)
    ));
}

#[test]
fn notify_names_what_moved_and_a_response_applies_once() {
    let mut store = Store::default();
    let before = store.as_of();
    let o = an_object("elaboration/a/1", "elaboration");
    store.put(&o.id, &o, 0).unwrap();
    let notice = store.notify(&before).unwrap();
    assert_eq!(notice.objects, vec!["elaboration/a/1".to_string()]);
    assert!(
        store.notify(&notice.as_of).unwrap().objects.is_empty(),
        "a notice names what moved since a point, not everything (130)"
    );

    let response = Response {
        id: "discord/1001".into(),
        kind: ResponseKind::Dictation,
        decision: None,
        object: Some(o.id.clone()),
        answer: "keep".into(),
        given_by: "chuck".into(),
        given_at: store.now,
        delivery: "discord/1001".into(),
    };
    assert_eq!(
        store.receive(&response).unwrap(),
        Received::Recorded {
            id: "discord/1001".into()
        }
    );
    assert_eq!(
        store.receive(&response).unwrap(),
        Received::AlreadyApplied {
            id: "discord/1001".into()
        },
        "the same delivery twice takes effect once (137)"
    );

    // A response naming a number no decision carries is handed back, not
    // dropped (6, 129).
    let stray = Response {
        id: "discord/1002".into(),
        kind: ResponseKind::Answer,
        decision: Some(99),
        object: None,
        answer: "yes".into(),
        given_by: "chuck".into(),
        given_at: store.now,
        delivery: "discord/1002".into(),
    };
    assert!(matches!(
        store.receive(&stray).unwrap(),
        Received::Unapplicable { .. }
    ));
}
