//! The six record operations over a local bare state repository. No network.

use chrono::{Duration, TimeZone, Utc};
use flywheel_atoms::{
    EffectWrite, LeaseOp, LeaseOutcome, PutOutcome, Received, Records, Scope, StateStore,
    ThreadEntry, WriteOutcome,
};
use flywheel_engine::runtime::{Object, Response, ResponseKind};
use flywheel_store_git::{store::sandbox, GitStore};
use serde_json::json;

fn at(minute: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 0, minute, 0).unwrap()
}

struct Sandbox {
    dir: std::path::PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let dir = std::env::temp_dir().join(format!(
            "flywheel-git-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Sandbox { dir }
    }
    fn host(&self, name: &str) -> GitStore {
        sandbox(&self.dir, name, at(0)).expect("a state repository and a host's checkout")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn a_lamp(id: &str) -> Object {
    let mut o = Object {
        id: id.into(),
        machine: "lamp".into(),
        parent: None,
        config: Default::default(),
        entered_at: Default::default(),
        record: Default::default(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 1,
    };
    o.config.insert("power".into(), "off".into());
    o.entered_at.insert("power".into(), at(0));
    o
}

#[test]
fn a_record_is_a_file_readable_with_no_host_running() {
    let sandbox = Sandbox::new("durable");
    let mut a = sandbox.host("a");
    let lamp = a_lamp("lamp/1");
    assert_eq!(a.put("lamp/1", &lamp, 0).unwrap(), PutOutcome::Written { seq: 1 });

    // Written means on the git host: a host that never saw the write reads it
    // after every other host is lost (133, 132, 161).
    let fresh = sandbox.host("c");
    let read = fresh.get("lamp/1").unwrap().expect("the object is there");
    assert_eq!(read.config.get("power").map(String::as_str), Some("off"));
    assert_eq!(read.seq, 1);
    assert!(a.unpushed().unwrap().is_empty(), "nothing is left as an intention");
}

#[test]
fn get_put_append_and_list_over_the_shared_line() {
    let sandbox = Sandbox::new("records");
    let mut a = sandbox.host("a");
    for id in ["lamp/1", "lamp/2", "lamp/3"] {
        a.put(id, &a_lamp(id), 0).unwrap();
    }
    assert_eq!(a.list_records(&Scope::All).unwrap().len(), 3);
    assert_eq!(
        a.list(&Scope::Machine("lamp".into())).unwrap().objects.len(),
        3
    );

    // A write from a stale base is refused and says what the store holds (134).
    assert_eq!(
        a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap(),
        PutOutcome::Rejected { held_seq: 1 }
    );

    // The thread is append-only (144).
    let entry = |kind: &str| ThreadEntry {
        at: at(1),
        kind: kind.into(),
        by: Some("chuck".into()),
        fields: [("text".to_string(), json!("said"))].into_iter().collect(),
    };
    a.append("lamp/1", &entry("note")).unwrap();
    a.append("lamp/1", &entry("exit")).unwrap();
    let thread = a.thread("lamp/1").unwrap();
    assert_eq!(
        thread.iter().map(|e| e.kind.as_str()).collect::<Vec<_>>(),
        vec!["note", "exit"]
    );
    assert_eq!(thread[0].fields.get("text"), Some(&json!("said")));
}

#[test]
fn a_read_names_its_point_and_two_reads_are_equal() {
    let sandbox = Sandbox::new("read");
    let mut a = sandbox.host("a");
    a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();
    a.set_given("lamp/1", "lamp.switch", json!("down"));

    let first = a.read("lamp/1").unwrap();
    let second = a.read("lamp/1").unwrap();
    assert_eq!(first.as_of, second.as_of, "the read names its point (126)");
    assert_eq!(first.evidence, second.evidence);
    assert_eq!(first.as_of.mark.len(), 40, "the point is the commit");
    assert_eq!(first.evidence.get("lamp.switch"), Some(&json!("down")));
    // A read writes nothing.
    assert!(a.unpushed().unwrap().is_empty());
}

#[test]
fn an_effect_is_written_with_an_identity_and_a_repeat_is_not_a_second_write() {
    let sandbox = Sandbox::new("effect");
    let mut a = sandbox.host("a");
    a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();
    let write = EffectWrite {
        effect_id: "lamp/1/on/lamp.lit/9f2c".into(),
        object: "lamp/1".into(),
        effect: "light".into(),
        reason: "the switch is up".into(),
        evidence: [("lamp.switch".to_string(), json!("up"))].into_iter().collect(),
    };
    assert!(matches!(
        a.write_effect(&write).unwrap(),
        WriteOutcome::Written { .. }
    ));
    // The act may run again — that is the proof's question (73) — but the
    // write with that identity changes nothing and is not a second write (127).
    assert!(matches!(
        a.write_effect(&write).unwrap(),
        WriteOutcome::AlreadyWritten { .. }
    ));

    // The identity, the reason and the evidence are in the message (79, 167).
    let log = a
        .repo
        .git(&["log", "--format=%B", "-n", "3", &a.fetched])
        .unwrap();
    assert!(log.contains("lamp/1/on/lamp.lit/9f2c"));
    assert!(log.contains("reason: the switch is up"));
    assert!(log.contains("lamp.switch=\"up\""));

    // A fresh host finds the same effect already written (127).
    let mut fresh = sandbox.host("c");
    assert!(matches!(
        fresh.write_effect(&write).unwrap(),
        WriteOutcome::AlreadyWritten { .. }
    ));
}

#[test]
fn two_would_be_holders_of_one_lease_and_the_expiry_rule() {
    let sandbox = Sandbox::new("lease");
    let mut a = sandbox.host("a");
    let mut b = sandbox.host("b");
    let take = |who: &str| LeaseOp::Take {
        object: "lamp/1".into(),
        holder: who.into(),
    };

    assert!(matches!(a.lease(&take("a")).unwrap(), LeaseOutcome::Held(_)));
    b.fetch().unwrap();
    match b.lease(&take("b")).unwrap() {
        LeaseOutcome::HeldByAnother(held) => assert_eq!(held.holder, "a"),
        other => panic!("two holders of one lease: {other:?}"),
    }
    // b learned who holds it from the fetch, before attempting anything: the
    // loser reads the holder and moves on (128, 134). A push that actually
    // races is the single-writer contract's case.

    // Renewal keeps it, and adds no commit to main (163, 167).
    let main_before = a.repo.git(&["rev-list", "--count", &a.fetched]).unwrap();
    a.now = at(1);
    assert!(matches!(
        a.lease(&LeaseOp::Renew {
            object: "lamp/1".into(),
            holder: "a".into()
        })
        .unwrap(),
        LeaseOutcome::Held(_)
    ));
    a.fetch().unwrap();
    let main_after = a.repo.git(&["rev-list", "--count", &a.fetched]).unwrap();
    assert_eq!(main_before, main_after, "leases are never files on main (D5)");

    // Stale at 5 minutes, expired at 24 hours (128, 163).
    let held = a.leases("lamp/1").unwrap().expect("a lease");
    a.now = at(1) + Duration::minutes(6);
    assert!(a.lease_stale(&held));
    assert!(!a.lease_expired(&held));
    a.now = at(1) + Duration::hours(25);
    assert!(a.lease_expired(&held));

    // Once expired, the other host may take it.
    b.now = a.now;
    b.fetch().unwrap();
    assert!(matches!(b.lease(&take("b")).unwrap(), LeaseOutcome::Held(_)));
}

#[test]
fn a_response_delivered_twice_writes_one_file() {
    let sandbox = Sandbox::new("response");
    let mut a = sandbox.host("a");
    a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();
    let response = Response {
        id: "chat/7".into(),
        kind: ResponseKind::Answer,
        decision: Some(1),
        object: Some("lamp/1".into()),
        answer: "on".into(),
        given_by: "chuck".into(),
        given_at: at(1),
        delivery: "chat/7".into(),
    };
    assert_eq!(
        a.receive(&response).unwrap(),
        Received::Recorded { id: "chat/7".into() }
    );
    assert_eq!(
        a.receive(&response).unwrap(),
        Received::AlreadyApplied { id: "chat/7".into() },
        "the file is named by the delivery id, so a second delivery is a no-op (137)"
    );
    let found = a.responses("lamp/1").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].given_by, "chuck");
}

#[test]
fn notify_names_what_moved_and_a_never_notified_host_converges() {
    let sandbox = Sandbox::new("notify");
    let mut a = sandbox.host("a");
    let mut b = sandbox.host("b");
    let before = b.as_of();

    a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();
    a.append(
        "lamp/1",
        &ThreadEntry {
            at: at(1),
            kind: "note".into(),
            by: None,
            fields: Default::default(),
        },
    )
    .unwrap();

    b.fetch().unwrap();
    let notice = b.notify(&before).unwrap();
    assert_eq!(
        notice.objects,
        vec!["lamp/1".to_string()],
        "a notice names what moved, not everything (130, 166)"
    );
    assert!(b.notify(&notice.as_of).unwrap().objects.is_empty());

    // A host that is never notified still converges by reading (130).
    let never = sandbox.host("never");
    assert!(never.get("lamp/1").unwrap().is_some());
}

#[test]
fn the_status_view_is_readable_with_no_host_running() {
    let sandbox = Sandbox::new("status");
    let mut a = sandbox.host("a");
    a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();
    let view = a.status().unwrap();
    assert!(view.body.contains("lamp/1"));
    assert!(
        view.body.contains(&view.as_of.mark),
        "it states the commit it is as of (145)"
    );
    // Derived from `list` and `get` alone, so a fresh clone renders the same.
    let fresh = sandbox.host("c");
    assert_eq!(fresh.status().unwrap().body, view.body);
}

/// Two hosts write one object from the same read. The second push is rejected;
/// the host fetches, sees that the object moved under it, discards its own
/// commit and reads again rather than rebasing its state over one it never read
/// (3, 134, 162, 164, I15).
#[test]
fn a_write_over_one_this_host_never_read_is_a_loss() {
    let sandbox = Sandbox::new("loss");
    let mut a = sandbox.host("a");
    let mut b = sandbox.host("b");

    let lamp = a_lamp("lamp/1");
    assert!(matches!(
        a.put("lamp/1", &lamp, 0).unwrap(),
        PutOutcome::Written { seq: 1 }
    ));

    // b read the object before a wrote it, and writes against that read.
    let mut theirs = lamp.clone();
    theirs.config.insert("power".into(), "on".into());
    match b.put("lamp/1", &theirs, 0).unwrap() {
        PutOutcome::Rejected { held_seq } => assert_eq!(held_seq, 1),
        other => panic!("the loser's write must be rejected, not {other:?}"),
    }

    // What stands is a's, whole: nothing of b's landed on top of it.
    a.fetch().unwrap();
    let held = a.get("lamp/1").unwrap().expect("the object");
    assert_eq!(held.seq, 1);
    assert_eq!(held.config.get("power").map(String::as_str), Some("off"));
}

/// The operator edits an object's file and commits it by hand: the commit
/// carries no sequence of its own, and every host reads the same delivery id
/// out of the same fetch (3, 159, 164).
#[test]
fn the_operators_own_commit_is_read_from_the_fetch() {
    let sandbox = Sandbox::new("operator");
    let mut a = sandbox.host("a");
    let mut b = sandbox.host("b");

    let lamp = a_lamp("lamp/1");
    a.put("lamp/1", &lamp, 0).unwrap();
    assert_eq!(a.operator_commit_on("lamp/1").unwrap(), None);

    let mut edited = a.get("lamp/1").unwrap().expect("the object");
    edited.config.insert("power".into(), "on".into());
    a.commit_as_operator(&edited, "abc123", "chuck").unwrap();

    b.fetch().unwrap();
    let seen = b.get("lamp/1").unwrap().expect("the object");
    assert_eq!(seen.config.get("power").map(String::as_str), Some("on"));
    assert_eq!(seen.seq, edited.seq, "a person's commit takes no sequence");
    assert_eq!(
        b.operator_commit_on("lamp/1").unwrap().as_deref(),
        Some("abc123"),
        "every host derives the same delivery id from the same fetch"
    );
}
