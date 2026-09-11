//! Captures, signals and moves: the material under the machinery's prefix in
//! the blueprints, keyed by source event and never rewritten
//! (`signals/capture-and-curation`, 111, 113, 107, 203).

use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use flywheel_domain::commands;
use flywheel_domain::signals::{self, Capture};
use flywheel_store_git::store::sandbox;
use std::collections::BTreeMap;
use serde_json::json;

fn at(minute: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap() + Duration::minutes(minute)
}

fn dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "flywheel-signals-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ------------------------------------------------------------ 9.1 the capture



// ---------------------------------------------- 9.5 a signal is never rewritten

/// An instance on disk: real git repositories, so a test can read the history
/// of the file a record was written to (113, 167).
/// The operator's key is handed over rather than put in the process's own
/// environment: the manifest names where the operator placed it and the
/// machinery only asks whether it is there (207, 207a).
struct Instance {
    dir: std::path::PathBuf,
}

impl Instance {
    fn new(name: &str) -> Instance {
        let dir = dir(&format!("instance-{name}"));
        let key_from = format!("FLYWHEEL_SIGNALS_KEY_{}", name.to_uppercase());
        let ask = flywheel::init::Init {
            instance: "willdan".into(),
            host: "mac-mini".into(),
            root: dir.join("root"),
            git_host: dir.join("git-host"),
            app: "12345".into(),
            app_key_from: key_from.clone(),
            app_key: Some("the operator placed this".into()),
            address: "http://laptop.example".into(),
            manifest: dir.join("flywheel.yaml"),
            curation: None,
        };
        flywheel::init::run(ask).expect("the instance bootstraps");
        let instance = Instance { dir };
        // The host joins by one command, which is what puts the clones and the
        // one checkout per shared line under the root (205).
        let mut world = instance.world();
        flywheel_world_host::join::join(&mut world).expect("the host joins");
        instance
    }

    fn world(&self) -> flywheel_world_host::HostWorld {
        let manifest =
            flywheel_world_host::Manifest::read(&self.dir.join("flywheel.yaml")).expect("the manifest");
        flywheel_world_host::HostWorld::open(manifest, "mac-mini").expect("the world")
    }

}

impl Drop for Instance {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}


// ---------------------------------------- 9.6 an older record reads unconverted


// ------------------------------------- 9.7 one standing move per signal


// ------------------------------------------ 9.8 the moves and their consequences


// ------------------------------------------------- 9.9 curation's cadence

/// Curation is charged on its cadence and on the unmoved threshold, and a
/// cadence missed while the host was down is caught up once, however many
/// firings were missed (110, 231, 111).
#[test]
fn curation_cadence_caught_up_once() {
    use flywheel_domain::cadence;

    // The shipped cadence: six in the morning, weekdays
    // (`blueprints.yaml` evidence.curation.cadence_due).
    let weekdays = cadence::Cadence::parse(cadence::DEFAULT).expect("the shipped cadence");
    // Monday 2026-01-05 at 06:00 is a firing; the Sunday before is not.
    let monday = Utc.with_ymd_and_hms(2026, 1, 5, 6, 0, 0).unwrap();
    assert!(weekdays.fires_at(monday));
    assert!(!weekdays.fires_at(monday - Duration::days(1)));
    assert!(!weekdays.fires_at(monday + Duration::hours(1)));

    // Nothing has fired since the last run an hour ago.
    assert!(!cadence::due(
        cadence::DEFAULT,
        monday + Duration::minutes(1),
        monday + Duration::hours(1)
    ));
    // One has, an hour later on the next weekday.
    assert!(cadence::due(
        cadence::DEFAULT,
        monday + Duration::hours(1),
        monday + Duration::days(1) + Duration::hours(1)
    ));

    // A day down. Four weekday firings were missed while the host was off, and
    // what the guard reads is one: the cadence is due, once, and the run that
    // follows moves the mark past all of them (231, 111).
    let ran = monday - Duration::days(7);
    let back = monday + Duration::days(3);
    let firings = (0..=10)
        .map(|d| ran + Duration::days(d))
        .filter(|at| {
            let at = Utc
                .with_ymd_and_hms(at.year(), at.month(), at.day(), 6, 0, 0)
                .unwrap();
            at > ran && at <= back && weekdays.fires_at(at)
        })
        .count();
    assert!(firings > 1, "the test does not miss several firings");
    assert!(cadence::due(cadence::DEFAULT, ran, back));
    // The run catches them up: the mark moves to now, and nothing is due again
    // until the next firing.
    assert!(!cadence::due(cadence::DEFAULT, back, back));

    // A cadence the machinery cannot read fires never, rather than at a time
    // nobody asked for.
    assert_eq!(cadence::Cadence::parse("every morning"), None);
    assert!(!cadence::due("every morning", ran, back));

    // And on a host: the curation record's threshold and cadence are what the
    // guard reads, and a day down charges one run (110, 231).
    let mut host = a_host("cadence");
    let curation = "curation/willdan";
    commands::put_new(
        &mut host.store,
        &host.defs.clone(),
        curation,
        "curation",
        None,
        [
            ("threshold".to_string(), json!(12)),
            ("cadence".to_string(), json!(cadence::DEFAULT)),
        ]
        .into_iter()
        .collect(),
        ran,
    )
    .unwrap();
    let read = |host: &flywheel::host::Host, name: &str| {
        flywheel_engine::runtime::EvidenceSource::evidence(&host.store, curation, "run", name)
    };
    assert_eq!(read(&host, "curation.threshold"), Some(json!(12)));

    // Nothing has fired since it was made.
    host.set_now(ran + Duration::minutes(1));
    assert_eq!(read(&host, "curation.cadence_due"), Some(json!(false)));

    // A day down, and several firings missed: due, once.
    host.set_now(back);
    assert_eq!(read(&host, "curation.cadence_due"), Some(json!(true)));
}

/// A host over a store already open, for a test that reads evidence.
fn a_host(name: &str) -> flywheel::host::Host {
    let base = dir(&format!("host-{name}"));
    let now = at(0);
    flywheel::host::Host::over(
        "mac-mini",
        "willdan",
        flywheel_domain::set::load().unwrap(),
        sandbox(&base, "mac-mini", now).unwrap(),
        flywheel::host::Bindings {
            world: "host".into(),
            workspace: "recorded".into(),
            sessions: "operator".into(),
        },
        Default::default(),
        now,
    )
}

// ----------------------------------- 9.13 unmoved signals on the status view

/// The status view shows the unmoved signals' count and age by source, and
/// none of them is discarded (118).
#[test]
fn unmoved_signals_by_source() {
    let instance = Instance::new("unmoved-view");
    let mut world = instance.world();
    let mut host = a_host("unmoved-view");

    // Signals from two sources: three from a meeting a week before now, one
    // from a chat forward the day before.
    let now = at(0);
    let sources = [
        ("meeting/2026-09-02/willdan-weekly", "meeting", now - Duration::days(7), 3u64),
        ("message/1421", "forwarded-message", now - Duration::days(1), 1),
    ];
    for (key, source, event_at, how_many) in sources {
        signals::write_capture(
            &mut world,
            &Capture {
                key: key.into(),
                source: source.into(),
                event_at: event_at.to_rfc3339(),
                captured_by: "chuck".into(),
                raw: format!("raw://{key}"),
            },
        )
        .unwrap();
        for n in 1..=how_many {
            signals::write_signal(
                &mut world,
                key,
                n,
                &signals::Signal {
                    id: signals::signal_object(key, n),
                    capture: signals::object_of(key),
                    kind: "ask".into(),
                    asserted_by: "dana".into(),
                    assertion: "something was said".into(),
                    excerpt: "something was said".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        }
    }
    // One of the four is moved; the other three are not (107).
    let moved = signals::signal_object("meeting/2026-09-02/willdan-weekly", 1);
    signals::write_move(
        &mut world,
        &signals::Move {
            signal: moved.clone(),
            target: "attach intent/atlas-provider-limits".into(),
            reason: "evidence".into(),
            at: now.to_rfc3339(),
        },
    )
    .unwrap();

    // The status view is read from the same material (118, 141).
    host.store.world = Box::new(instance.world());
    host.set_now(now);
    let status = host.status().unwrap();
    let by_source: BTreeMap<&str, &signals::UnmovedSource> = status
        .unmoved
        .iter()
        .map(|u| (u.source.as_str(), u))
        .collect();
    assert_eq!(
        by_source.keys().collect::<Vec<_>>(),
        vec![&"forwarded-message", &"meeting"],
        "the unmoved signals are not grouped by source: {:?}",
        status.unmoved
    );
    assert_eq!(by_source["meeting"].count, 2, "the moved one was counted");
    assert_eq!(by_source["forwarded-message"].count, 1);
    // Their age, counted from the event date (109, 118).
    assert_eq!(
        by_source["meeting"].oldest.as_deref(),
        Some((now - Duration::days(7)).to_rfc3339().as_str())
    );

    // And the view says so, with the count and the age by source.
    let view = flywheel_domain::status::render(&status);
    assert!(
        view.body.contains("<h2>unmoved signals</h2>"),
        "the status view has no unmoved section: {}",
        view.body
    );
    assert!(view.body.contains("data-source=\"meeting\" data-count=\"2\""), "{}", view.body);
    assert!(view.body.contains("2 from meeting, oldest 7d"), "{}", view.body);
    assert!(view.body.contains("1 from forwarded-message, oldest 1d"), "{}", view.body);

    // None is discarded: every signal record still stands, moved or not (118).
    let held = signals::all_signals_from(&signals::Blueprints(&instance.world()));
    assert_eq!(held.len(), 4, "a signal was discarded: {held:?}");
    assert_eq!(
        signals::unmoved(&signals::Blueprints(&instance.world())).len(),
        3
    );
    assert!(
        signals::standing_move(&instance.world(), &moved).unwrap().is_some(),
        "the moved signal lost its move"
    );
}
