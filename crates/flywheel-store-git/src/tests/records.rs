//! The six record operations over a local bare state repository. No network.

use chrono::{Duration, TimeZone, Utc};
use flywheel_atoms::{
    EffectWrite, LeaseOp, LeaseOutcome, PutOutcome, Received, Records, Scope, StateStore,
    ThreadEntry, WriteOutcome,
};
use flywheel_engine::runtime::{Object, Response, ResponseKind};
use crate::{store::sandbox, GitStore};
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

/// What anyone reading a host's state checkout with git sees:
/// `git status --porcelain` there.
fn status(store: &GitStore) -> String {
    let out = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&store.repo.dir)
        .output()
        .expect("git status runs");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// A host's commits leave its checkout clean: the index is written from the
/// tree each commit puts the branch at, so git lists nothing staged and
/// nothing changed where nothing is lost (169, 167).
#[test]
fn a_commit_leaves_the_checkout_clean() {
    let sandbox = Sandbox::new("clean-commit");
    let mut a = sandbox.host("a");
    assert_eq!(status(&a), "", "the checkout init leaves is clean");
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
    a.put("lamp/2", &a_lamp("lamp/2"), 0).unwrap();
    assert_eq!(status(&a), "", "a host's commits leave its checkout clean");
}

/// A host that takes up another's writes puts its checkout at them, index and
/// all, so its checkout is clean after the reset too (169, 165).
#[test]
fn a_reset_leaves_the_checkout_clean() {
    let sandbox = Sandbox::new("clean-reset");
    let mut a = sandbox.host("a");
    let mut b = sandbox.host("b");
    a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();
    a.put("lamp/2", &a_lamp("lamp/2"), 0).unwrap();
    b.fetch().unwrap();
    assert!(b.get("lamp/2").unwrap().is_some(), "the other host's checkout is at the writes");
    assert!(b.repo.dir.join(crate::layout::object("lamp/2")).exists(), "and its files are");
    assert_eq!(status(&b), "", "a reset leaves the checkout clean");
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

/// The status projection is committed on the shared line and read back from it
/// with no host running (132, 145, S20).
#[test]
fn status_is_committed_on_the_shared_line() {
    let sandbox = Sandbox::new("status");
    let mut store = sandbox.host("mac-mini");
    assert_eq!(store.committed_status().unwrap(), None);
    store.commit_status("<html>as of commit x</html>").unwrap();
    assert_eq!(
        store.committed_status().unwrap().as_deref(),
        Some("<html>as of commit x</html>")
    );
    // A reader with no host running clones and reads the file (160, 167).
    let reader = sandbox.host("phone");
    assert_eq!(
        reader.committed_status().unwrap().as_deref(),
        Some("<html>as of commit x</html>")
    );
}

// ------------------------ 16.4 an effect repeated inside one tick writes once

/// An effect written twice inside one tick writes one commit (127).
///
/// A tick's commits are local until it ends, so the point the tick fetched does
/// not move while it runs; the repeat has to be found against what this host
/// has written, not against what it last read.
#[test]
fn an_effect_repeated_in_one_tick_writes_once() {
    let sandbox = Sandbox::new("one-tick");
    let mut a = sandbox.host("a");
    a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();
    let write = EffectWrite {
        effect_id: "lamp/1/on/lamp.lit/one-tick".into(),
        object: "lamp/1".into(),
        effect: "light".into(),
        reason: "the switch is up".into(),
        evidence: [("lamp.switch".to_string(), json!("up"))].into_iter().collect(),
    };

    let before = a.repo.git(&["rev-list", "--count", "HEAD"]).unwrap();
    let before: usize = before.trim().parse().unwrap();

    // One tick, and the same effect written twice inside it.
    a.begin_tick();
    let first = a.write_effect(&write).unwrap();
    let second = a.write_effect(&write).unwrap();
    a.end_tick().unwrap();

    assert!(
        matches!(first, WriteOutcome::Written { .. } | WriteOutcome::Pending { .. }),
        "the first write of an effect id did not write: {first:?}"
    );
    assert!(
        matches!(second, WriteOutcome::AlreadyWritten { .. }),
        "the same effect id written twice in one tick wrote twice (127): {second:?}"
    );

    // And the history carries it once.
    let after: usize = a
        .repo
        .git(&["rev-list", "--count", "HEAD"])
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(
        after - before,
        1,
        "one effect, written twice in one tick, made {} commits (127)",
        after - before
    );
    let log = a.repo.git(&["log", "--format=%B", "-n", "5", "HEAD"]).unwrap();
    assert_eq!(
        log.matches("lamp/1/on/lamp.lit/one-tick").count(),
        1,
        "the effect id is in the history twice: {log}"
    );
}

#[test]
fn an_answer_that_names_only_its_number_reaches_the_object_the_register_gave_it() {
    // What a response carries is the decision's number (15): the page's control
    // and the chat's reply grammar both write that and nothing else, and it is
    // the register that says which object the number belongs to. A store that
    // matched on the response's `object` field alone would hand the object no
    // answer at all, its guard would never fire, and the response would come
    // back as an unapplicable decision of its own
    // (`record-derived.yaml` responses, 13).
    let sandbox = Sandbox::new("answer-by-number");
    let mut a = sandbox.host("a");
    a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();

    let mut register = flywheel_engine::runtime::Register::default();
    let number = register.number_for("lamp/1/lamp-proposed/2026-01-01T00:00:00Z", Some(at(0)));
    flywheel_domain::commands::set_register(
        &mut a,
        &register,
        &["lamp/1/lamp-proposed/2026-01-01T00:00:00Z".to_string()],
    )
    .unwrap();

    a.receive(&Response {
        id: "page-1".into(),
        kind: ResponseKind::Answer,
        decision: Some(number),
        object: None,
        answer: "on".into(),
        given_by: "chuck".into(),
        given_at: at(1),
        delivery: "page".into(),
    })
    .unwrap();

    let found = a.responses("lamp/1").unwrap();
    assert_eq!(found.len(), 1, "the answer reaches the object its number names");
    assert_eq!(found[0].answer, "on");

    // And it reaches no other object, so an answer is applied to one thing.
    a.put("lamp/2", &a_lamp("lamp/2"), 0).unwrap();
    assert!(a.responses("lamp/2").unwrap().is_empty());

    // The rail holds every response, which is what a tick reads before it
    // decides anything.
    assert_eq!(a.responses(flywheel_domain::RAIL).unwrap().len(), 1);
}

#[test]
fn a_put_of_what_is_already_there_is_not_a_write() {
    // Reading the same stores twice with nothing changed produces the same
    // conclusion and no writes (78). A put is how a caller says what an object
    // now is, and a caller that has read an object, decided nothing moved and
    // put it back has said nothing: the sequence is the token of a change and
    // not of a pass, so it does not move and there is no commit on the shared
    // line to say it did (127, 167).
    let sandbox = Sandbox::new("put-unchanged");
    let mut a = sandbox.host("a");
    let commits = |store: &GitStore| -> usize {
        store
            .repo
            .git(&["log", "--format=%H", "HEAD"])
            .unwrap()
            .lines()
            .count()
    };

    let outcome = a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();
    assert_eq!(outcome, PutOutcome::Written { seq: 1 });
    let after_the_write = commits(&a);

    // The same object, read back and put again unchanged.
    let held = a.get("lamp/1").unwrap().expect("the object is there");
    let outcome = a.put("lamp/1", &held, held.seq).unwrap();
    assert_eq!(
        outcome,
        PutOutcome::Written { seq: 1 },
        "the sequence moved for a put that changed nothing"
    );
    assert_eq!(
        commits(&a),
        after_the_write,
        "a put of what is already there made a commit (78, 167)"
    );
    assert_eq!(a.get("lamp/1").unwrap().unwrap().seq, 1);

    // A put that does change something writes, and the sequence moves with it.
    let mut moved = a.get("lamp/1").unwrap().unwrap();
    moved.config.insert("state".into(), "lit".into());
    let outcome = a.put("lamp/1", &moved, moved.seq).unwrap();
    assert_eq!(outcome, PutOutcome::Written { seq: 2 });
    assert_eq!(commits(&a), after_the_write + 1);
}

#[test]
fn an_answer_is_a_notice_about_the_object_it_answers() {
    // A notice names what moved since a point, so a host re-reads only that
    // (130, D6). An answer moves no object file — a response is a record of its
    // own, under `responses/` and not under `objects/` — so a notice taken from
    // the changed object files alone named nothing at all when the operator
    // answered: the loop woke on the answer, found nothing to tick, and the
    // object waited out the sweep. Up to a minute in which the operator had
    // clicked and the page showed them nothing, which is the moment 13 says
    // they never have to nudge through.
    let sandbox = Sandbox::new("answer-notice");
    let mut a = sandbox.host("a");
    a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();

    let mut register = flywheel_engine::runtime::Register::default();
    let decision = "lamp/1/lamp-proposed/2026-01-01T00:00:00Z";
    let number = register.number_for(decision, Some(at(0)));
    flywheel_domain::commands::set_register(&mut a, &register, &[decision.to_string()]).unwrap();

    // The point the host last read at, before the answer arrives.
    let point = StateStore::read(&a, "lamp/1").unwrap().as_of;
    assert!(
        a.notify(&point).unwrap().objects.is_empty(),
        "nothing has moved yet"
    );

    a.receive(&Response {
        id: "page-1".into(),
        kind: ResponseKind::Answer,
        decision: Some(number),
        object: None,
        answer: "on".into(),
        given_by: "chuck".into(),
        given_at: at(1),
        delivery: "page".into(),
    })
    .unwrap();

    let notice = a.notify(&point).unwrap();
    assert!(
        notice.objects.iter().any(|o| o == "lamp/1"),
        "the answer named no object to tick: {:?}",
        notice.objects
    );
}
