//! `flywheel scenario apply`: a scenario's actions played into a real instance,
//! which stands at the moment they reached (19.2, 19.3).
//!
//! Every test here makes a real instance — `flywheel init`, then `flywheel host
//! join` — and runs the machinery over it. Nothing sets state: a capture
//! arrives through the adapter, a session's delivery is a file committed on a
//! repository's shared line, and the exit goes through the reporting path a
//! real session reports through (67, 93, 111, 125, 193).

use chrono::{DateTime, Duration, TimeZone, Utc};
use flywheel::apply::{self, Applied};
use flywheel_atoms::{Records, Scope};
use std::path::{Path, PathBuf};

fn at() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 2, 9, 0, 0).unwrap()
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures")).join(name)
}

/// A fresh directory under `target/`, removed when the test is done.
struct Under(PathBuf);

impl Under {
    fn new(name: &str) -> Under {
        let dir = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/flywheel-apply"))
            .join(format!("{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the directory is made");
        Under(dir)
    }
}

impl Drop for Under {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn apply_through(scenario: &Path, under: &Path, through: usize) -> Applied {
    apply::apply(scenario, under, Some(through), at(), Duration::seconds(60))
        .unwrap_or_else(|e| panic!("applying {} through {through}: {e:#}", scenario.display()))
}

/// Every object's id and the states it stands in: what "the instance stands at
/// this moment" means, read from the state repository itself.
fn standing(applied: &Applied) -> Vec<(String, Vec<(String, String)>)> {
    let host = flywheel::host::Host::open(&applied.manifest, "local", None, applied.at)
        .expect("the host opens over the instance the apply left");
    let mut out: Vec<(String, Vec<(String, String)>)> = host
        .store
        .list_records(&Scope::All)
        .expect("the state repository reads")
        .into_iter()
        // The run record's own objects carry the moment they were written and
        // say nothing about where the machinery stands.
        .filter(|o| !o.id.starts_with("fact/place/"))
        .map(|o| {
            let mut states: Vec<(String, String)> = o
                .config
                .iter()
                .map(|(r, s)| (r.clone(), s.clone()))
                .collect();
            states.sort();
            (o.id.clone(), states)
        })
        .collect();
    out.sort();
    out
}

fn decisions(applied: &Applied) -> Vec<(Option<u32>, String, String)> {
    let mut host = flywheel::host::Host::open(&applied.manifest, "local", None, applied.at)
        .expect("the host opens");
    let defs = host.defs.clone();
    flywheel_domain::commands::rail(&mut host.store, &defs)
        .expect("the rail is derived")
        .into_iter()
        .map(|d| (d.number, d.kind, d.object))
        .collect()
}

/// Applying through n plays exactly n actions and leaves the instance standing
/// where the machinery took it. Applying through n twice, into two fresh
/// instances, gives the same state: the actions are the clock and nothing in a
/// run depends on when it was run.
#[test]
fn applying_through_n_lands_the_instance_at_n() {
    let scenario = fixture("reading");

    let one = Under::new("through-1");
    let after_one = apply_through(&scenario, &one.0, 1);
    assert_eq!(after_one.through, 1);
    assert_eq!(after_one.actions, 2, "the scenario holds two actions");

    let two = Under::new("through-2");
    let after_two = apply_through(&scenario, &two.0, 2);

    // The capture is read at two and is not at one: the instance stands at the
    // moment the action number names, and there is no stepping backwards.
    let capture = "capture/meeting-2026-09-02-storefront-weekly";
    let reading = |applied: &Applied| {
        standing(applied)
            .into_iter()
            .find(|(id, _)| id == capture)
            .unwrap_or_else(|| panic!("the capture is an object of the instance"))
            .1
            .into_iter()
            .find(|(region, _)| region == "reading")
            .expect("the capture has a reading region")
            .1
    };
    assert_eq!(
        reading(&after_one),
        "reading",
        "at one action the reader has been started and is waiting on the session"
    );
    assert_eq!(
        reading(&after_two),
        "read",
        "at two it has delivered, so the capture is read (111, 115)"
    );

    // The same action number into a fresh instance gives the same state.
    let again = Under::new("through-2-again");
    let after_two_again = apply_through(&scenario, &again.0, 2);
    assert_eq!(
        standing(&after_two),
        standing(&after_two_again),
        "applying through the same action twice gives the same state"
    );
    assert_eq!(after_two.at, after_two_again.at, "and the same moment");

    // And there is no action past the end.
    let past = Under::new("past-the-end");
    let refused = apply::apply(&scenario, &past.0, Some(3), at(), Duration::seconds(60))
        .expect_err("there is no action 3");
    assert!(format!("{refused:#}").contains("no action 3"), "{refused:#}");
}

/// What a session delivers is a real file at a real path from that moment on:
/// the artifact is copied out of the scenario's bundle on to the repository's
/// shared line, and the machinery reads it as it finds it (111, 115, 193).
#[test]
fn a_delivered_artifact_is_a_file_in_the_repository() {
    let under = Under::new("delivered");
    let applied = apply_through(&fixture("reading"), &under.0, 2);

    let path = "flywheel/signals/meeting/2026-09-02/storefront-weekly/1.rec";
    let checkout = under
        .0
        .canonicalize()
        .expect("the directory resolves")
        .join("root/t-reading/flywheel-blueprints")
        .join(path);
    assert!(
        checkout.is_file(),
        "the deliverable is a file in this host's checkout: {}",
        checkout.display()
    );
    let body = std::fs::read_to_string(&checkout).expect("it reads");
    assert!(
        body.contains("cards are declining at checkout"),
        "and it is the bundle's artifact rather than a stand-in for one: {body}"
    );

    // The machinery read it: `capture.signals_present` is what moves the
    // capture to `read`, and nothing set that evidence.
    let mut host = flywheel::host::Host::open(&applied.manifest, "local", None, applied.at)
        .expect("the host opens");
    let signals = host.store.with_world(|_, world| {
        world.list_files("flywheel-blueprints", "flywheel/signals/")
    });
    assert!(
        signals.expect("the blueprints list").iter().any(|p| p.ends_with("1.rec")),
        "the delivered record is on the shared line, which is where the machinery reads it (167)"
    );
    assert!(
        applied.lines[1].contains("done"),
        "and the session reported its exit through the reporting path: {}",
        applied.lines[1]
    );
}

/// A session that comes back with a question instead of an answer puts the
/// question on the rail as a numbered decision (10, 24, 68, 82).
#[test]
fn a_session_that_asks_reaches_the_rail() {
    let under = Under::new("asking");
    let applied = apply_through(&fixture("asking"), &under.0, 2);

    let standing = decisions(&applied);
    let asked = standing
        .iter()
        .find(|(_, kind, _)| kind == "question")
        .unwrap_or_else(|| panic!("the question is on the rail; standing: {standing:?}"));
    assert!(
        asked.0.is_some(),
        "and it carries a number, answerable anywhere (15): {asked:?}"
    );
    assert!(
        asked.2.starts_with("capture/meeting-2026-09-02-storefront-weekly"),
        "and it names the work it is about: {asked:?}"
    );
}

/// The instance a scenario is applied into covers what the scenario names, so
/// nothing it seeds arrives uncovered (149, 205, 206).
///
/// An object whose record names a repository the instance does not track is one
/// no host's declaration covers, and 149 makes that a decision under attention
/// — rightly, because the machinery acts on none of it. But the operator can
/// answer such a decision with nothing but "seen": the demo opened on a rail of
/// them instead of the decisions it was written to show, and the seeded work
/// stood still. So the instance is made to track what the scenario names before
/// anything goes into it.
#[test]
fn an_applied_instance_covers_what_it_seeds() {
    let under = Under::new("covering");
    let applied = apply_through(
        &PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../scenarios/storefront")),
        &under.0,
        1,
    );

    // The repository the described state names is one the instance tracks and
    // this host holds a checkout of.
    let manifest = flywheel_world_host::Manifest::read(&applied.manifest).expect("the manifest");
    assert!(
        manifest.repositories.contains_key("storefront"),
        "the instance tracks what the scenario named: {:?}",
        manifest.repositories.keys().collect::<Vec<_>>()
    );

    // And no lease is uncovered, so every seeded object is one a host will act
    // on.
    let host = flywheel::host::Host::open(&applied.manifest, "local", None, applied.at)
        .expect("the host opens over the instance the apply left");
    let uncovered: Vec<String> = host
        .store
        .list_records(&Scope::All)
        .expect("the state repository reads")
        .into_iter()
        .filter(|o| flywheel_domain::leases::leasable(o))
        .filter(|o| {
            host.store
                .leases(&o.id)
                .unwrap_or_default()
                .is_some_and(|l| l.state == "uncovered")
        })
        .map(|o| o.id)
        .collect();
    assert!(
        uncovered.is_empty(),
        "the seed put objects in that no declaration covers: {uncovered:?}"
    );
}

/// An apply ends at the present and never ahead of it, and its actions reach
/// back as far as they happened (D15, 231).
///
/// The actions are things that happened, and the last of them happened now. A
/// run that started at the present and advanced sixty seconds an action ended
/// that far ahead of the clock, so every moment in the instance was in the
/// future of the host that picked it up: its ages were negative, its history
/// read backwards, and the run record — what a person reads to know what
/// happened (79, 167) — could not be read as a sequence at all.
#[test]
fn an_apply_ends_at_the_present_and_ages_what_came_before() {
    let under = Under::new("clock");
    let scenario = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scenarios/storefront"
    ));
    let per_action = Duration::seconds(60);
    let now = at();
    let through = 4;
    let applied = apply::apply(&scenario, &under.0, Some(through), now, per_action)
        .expect("the scenario applies");

    assert_eq!(
        applied.at, now,
        "the last action happened now, so the instance stands at now"
    );

    // Nothing in it is stamped ahead of the moment it stands at, and the
    // machinery's own reads are all `now` less a moment a state was entered.
    let host = flywheel::host::Host::open(&applied.manifest, "local", None, applied.at)
        .expect("the host opens");
    let mut ahead: Vec<String> = Vec::new();
    let mut oldest = applied.at;
    for object in host
        .store
        .list_records(&Scope::All)
        .expect("the state repository reads")
    {
        for (region, entered) in &object.entered_at {
            if *entered > applied.at {
                ahead.push(format!("{}/{region} at {}", object.id, entered.to_rfc3339()));
            }
            oldest = oldest.min(*entered);
        }
    }
    assert!(
        ahead.is_empty(),
        "the apply stamped moments ahead of where the instance stands: {ahead:?}"
    );

    // And what happened first is as old as the actions say: a scenario played
    // through four actions leaves its earliest moment four intervals back, so
    // signals have aged and "seen at" means something without anyone waiting.
    assert!(
        applied.at - oldest >= per_action * (through as i32 - 1),
        "the run did not reach back over its actions: oldest {} against {}",
        oldest.to_rfc3339(),
        applied.at.to_rfc3339()
    );
}
