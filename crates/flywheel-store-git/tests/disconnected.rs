//! A laptop loses its route often, so C.2's disconnected half is proved here
//! rather than left as a footnote (151, 165, D4a).

use chrono::{Duration, TimeZone, Utc};
use flywheel_atoms::{
    EffectWrite, LeaseOp, LeaseOutcome, PutOutcome, Records, StateStore, WriteOutcome,
};
use flywheel_engine::runtime::Object;
use flywheel_store_git::{store::sandbox, GitStore};
use serde_json::json;

fn at(minute: i64) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap() + Duration::minutes(minute)
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
    o
}

#[test]
fn renewals_add_no_commit_to_main() {
    // A month of minute-by-minute renewals, and `main` is where it was: the
    // history is state changes and nothing else, so it stays a readable audit
    // record (163, 167, D5).
    let sandbox = Sandbox::new("renewals");
    let mut a = sandbox.host("a");
    a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();
    let main_before = a
        .repo
        .git(&["rev-list", "--count", &a.fetched])
        .unwrap()
        .trim()
        .to_string();

    a.lease(&LeaseOp::Take {
        object: "lamp/1".into(),
        holder: "a".into(),
    })
    .unwrap();
    // A month at one renewal a minute is 43 200; a hundred proves the shape and
    // keeps the test a second long.
    for minute in 1..=100 {
        a.now = at(minute);
        assert!(matches!(
            a.lease(&LeaseOp::Renew {
                object: "lamp/1".into(),
                holder: "a".into()
            })
            .unwrap(),
            LeaseOutcome::Held(_)
        ));
    }
    a.fetch().unwrap();
    let main_after = a
        .repo
        .git(&["rev-list", "--count", &a.fetched])
        .unwrap()
        .trim()
        .to_string();
    assert_eq!(main_before, main_after, "no renewal touched main");

    // The lease branch holds exactly one commit: renewal replaces it, and the
    // old commit becomes unreachable (D5).
    let count = a
        .repo
        .git(&["rev-list", "--count", "refs/remotes/origin/lease/lamp/1/lease"])
        .unwrap();
    assert_eq!(count.trim(), "1");
    let held = a.leases("lamp/1").unwrap().expect("the lease");
    assert_eq!(held.renewed_at, at(100));
    assert_eq!(held.taken_at, at(0), "the take is when it was taken");
}

#[test]
fn disconnected_rules() {
    // What a disconnected host may do: keep ticking what it holds and commit
    // locally. What it may not: take a new lease, or report a write as written
    // (151, 161).
    let sandbox = Sandbox::new("disconnected");
    let mut a = sandbox.host("a");
    a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();
    a.lease(&LeaseOp::Take {
        object: "lamp/1".into(),
        holder: "a".into(),
    })
    .unwrap();
    let landed = a.fetched.clone();

    a.disconnected = true;
    a.now = at(5);

    // It keeps ticking what it holds, and commits locally.
    let mut lamp = a.get("lamp/1").unwrap().unwrap();
    lamp.config.insert("power".into(), "on".into());
    assert_eq!(
        a.put("lamp/1", &lamp, 1).unwrap(),
        PutOutcome::Written { seq: 2 }
    );

    // A write is reported as pending, never as written: a local commit is an
    // intention (161).
    let write = EffectWrite {
        effect_id: "lamp/1/on/lamp.lit/aaaa".into(),
        object: "lamp/1".into(),
        effect: "light".into(),
        reason: "the switch is up".into(),
        evidence: [("lamp.switch".to_string(), json!("up"))].into_iter().collect(),
    };
    assert!(matches!(
        a.write_effect(&write).unwrap(),
        WriteOutcome::Pending { .. }
    ));
    // And it is not performed twice while it waits (127).
    assert!(matches!(
        a.write_effect(&write).unwrap(),
        WriteOutcome::AlreadyWritten { .. }
    ));

    // It takes no new lease (151).
    let refused = a.lease(&LeaseOp::Take {
        object: "lamp/2".into(),
        holder: "a".into(),
    });
    assert!(refused.is_err(), "a disconnected host takes no new lease");

    // Nothing it did reached the git host: another host reads the state before.
    let elsewhere = sandbox.host("c");
    assert_eq!(
        elsewhere
            .get("lamp/1")
            .unwrap()
            .unwrap()
            .config
            .get("power")
            .map(String::as_str),
        Some("off")
    );
    assert!(!a.unpushed().unwrap().is_empty(), "the intentions are queued");
    assert_eq!(a.fetched, landed, "it read nothing newer while away");
}

#[test]
fn reconnect_pushes_renewals_first() {
    // On reconnect the lease renewals push first; a rejection means the object
    // was taken over (165, D4a).
    let sandbox = Sandbox::new("reconnect");
    let mut a = sandbox.host("a");
    a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();
    a.lease(&LeaseOp::Take {
        object: "lamp/1".into(),
        holder: "a".into(),
    })
    .unwrap();

    a.disconnected = true;
    a.now = at(5);
    let mut lamp = a.get("lamp/1").unwrap().unwrap();
    lamp.config.insert("power".into(), "on".into());
    a.put("lamp/1", &lamp, 1).unwrap();
    assert!(!a.unpushed().unwrap().is_empty());

    // The route comes back and the host reconnects: renewals first, then the
    // commits it made while away.
    a.disconnected = false;
    a.now = at(10);
    assert!(matches!(
        a.lease(&LeaseOp::Renew {
            object: "lamp/1".into(),
            holder: "a".into()
        })
        .unwrap(),
        LeaseOutcome::Held(_)
    ));
    let pushed = a.push_unpushed().expect("the queued commits land");
    assert!(pushed, "what was an intention is now a fact (161)");
    assert!(a.unpushed().unwrap().is_empty());

    // And another host reads it.
    let elsewhere = sandbox.host("c");
    assert_eq!(
        elsewhere
            .get("lamp/1")
            .unwrap()
            .unwrap()
            .config
            .get("power")
            .map(String::as_str),
        Some("on")
    );
}

#[test]
fn unpushed_commits_at_start() {
    // The disk holds only what git already holds or what is about to be
    // committed (I14), so at start, before the first tick, the host pushes any
    // unpushed commits on main; nothing is inferred from the working tree.
    let sandbox = Sandbox::new("unpushed");
    let mut a = sandbox.host("a");
    a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();
    a.disconnected = true;
    let mut lamp = a.get("lamp/1").unwrap().unwrap();
    lamp.config.insert("power".into(), "on".into());
    a.put("lamp/1", &lamp, 1).unwrap();
    let queued = a.unpushed().unwrap().len();
    assert_eq!(queued, 1);

    // The host is stopped and started again; the queue is on disk, in git.
    drop(a);
    let mut restarted = sandbox.host("a");
    assert_eq!(restarted.unpushed().unwrap().len(), 1, "read from git, not from memory");
    assert!(restarted.push_unpushed().unwrap());
    assert!(restarted.unpushed().unwrap().is_empty());

    let elsewhere = sandbox.host("c");
    assert_eq!(
        elsewhere
            .get("lamp/1")
            .unwrap()
            .unwrap()
            .config
            .get("power")
            .map(String::as_str),
        Some("on")
    );
}

#[test]
fn local_notify_ticks_at_once() {
    // A local cause notifies in-process immediately and does not wait for the
    // poll (130, D6).
    let sandbox = Sandbox::new("local-notify");
    let mut a = sandbox.host("a");
    let before = a.as_of();
    a.put("lamp/1", &a_lamp("lamp/1"), 0).unwrap();

    // The write is its own cause: the host's next read names it without a poll.
    let notice = a.notify(&before).unwrap();
    assert_eq!(notice.objects, vec!["lamp/1".to_string()]);
    assert_eq!(a.poll_bound(), chrono::Duration::seconds(30));
}
