//! Captures, signals and moves: the material under the machinery's prefix in
//! the blueprints, keyed by source event and never rewritten
//! (`signals/capture-and-curation`, 111, 113, 107, 203).

use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use flywheel_atoms::{Records, World};
use flywheel_domain::commands;
use flywheel_domain::signals::{self, Capture};
use flywheel_scenario::bindings::FilesWorld;
use flywheel_store_git::store::sandbox;
use flywheel_store_git::GitStore;
use flywheel_surface::catalogue::{self, Call};
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

fn a_store(name: &str) -> (GitStore, FilesWorld, flywheel_engine::Definitions) {
    let base = dir(name);
    (
        sandbox(&base, "mac-mini", at(0)).unwrap(),
        FilesWorld::new(),
        flywheel_domain::set::load().unwrap(),
    )
}

// ------------------------------------------------------------ 9.1 the capture

/// A capture is one source event: it carries its provenance and a pointer to
/// the raw material, it lives under the machinery's prefix in the blueprints,
/// and capturing the same event twice yields one (111, 203).
#[test]
fn capture_keyed_once() {
    let (mut store, mut world, defs) = a_store("keyed-once");

    let capture = Capture {
        key: "meeting/2026-09-02/willdan-weekly".into(),
        source: "meeting".into(),
        event_at: at(0).to_rfc3339(),
        captured_by: "chuck".into(),
        // A pointer, not the transcript: the raw material stays outside every
        // repository and the capture cites it (111).
        raw: "raw://meetings/2026-09-02-willdan-weekly.vtt".into(),
    };
    assert!(
        signals::write_capture(&mut world, &capture).unwrap(),
        "the first capture wrote nothing"
    );

    // Under the machinery's prefix, in the blueprints, and nowhere else (203).
    let path = signals::capture_path(&capture.key);
    assert_eq!(
        path,
        "flywheel/signals/captures/meeting/2026-09-02/willdan-weekly.rec"
    );
    assert!(path.starts_with("flywheel/"), "outside the prefix: {path}");
    let written: Vec<String> = world
        .files
        .keys()
        .filter(|p| !p.starts_with("flywheel/"))
        .cloned()
        .collect();
    assert!(written.is_empty(), "written outside the prefix: {written:?}");

    // Its provenance and its pointer, read back as they stand.
    let held = signals::read_capture(&world, &capture.key)
        .unwrap()
        .expect("the capture");
    assert_eq!(held, capture);
    let body = String::from_utf8(
        world
            .read_file(signals::BLUEPRINTS, &path)
            .unwrap()
            .expect("the record"),
    )
    .unwrap();
    assert!(body.contains(signals::CAPTURE_FORMAT), "{body}");
    // The record cites the raw material and does not hold it (111).
    assert!(body.contains("raw://meetings/"), "{body}");
    assert!(
        !body.contains("WEBVTT") && body.lines().count() < 12,
        "the record holds the material rather than a pointer to it: {body}"
    );

    // The same source event again: one capture, and the second write reports
    // itself as no write at all (111, 127).
    let again = Capture {
        captured_by: "someone else".into(),
        event_at: at(60 * 24).to_rfc3339(),
        ..capture.clone()
    };
    assert!(
        !signals::write_capture(&mut world, &again).unwrap(),
        "the same source event was captured twice"
    );
    assert_eq!(signals::captures(&world).unwrap().len(), 1);
    assert_eq!(
        signals::read_capture(&world, &capture.key).unwrap().unwrap(),
        capture,
        "the second capture overwrote the first"
    );

    // And through the tool the page's box and the chat forward both call: one
    // keyed capture, and the delivery recorded once (111, 193).
    let call = Call::new("capture", "chuck", "chat")
        .keyed("message/1421")
        .delivered("chat-1421")
        .arg("text", json!("discord message link 1421"))
        .arg("source", json!("forwarded-message"));
    catalogue::call(&mut store, &mut world, &defs, &call).unwrap();
    catalogue::call(&mut store, &mut world, &defs, &call).unwrap();
    assert_eq!(
        signals::captures(&world).unwrap().len(),
        2,
        "the same forwarded message made two captures"
    );
    let forwarded = signals::read_capture(&world, "message/1421")
        .unwrap()
        .expect("the forward's capture");
    assert_eq!(forwarded.source, "forwarded-message");
    assert_eq!(forwarded.captured_by, "chuck");
    assert_eq!(forwarded.raw, "discord message link 1421");
    // One object too, for the engine to tick: a key names its source first and
    // then the event, and an object id names one object, so the separators are
    // flattened (S21, S22).
    assert_eq!(signals::object_of("message/1421"), "capture/message-1421");
    assert!(
        Records::get(&store, "capture/message-1421")
            .unwrap()
            .is_some(),
        "the capture object was not written"
    );
}

/// A source-event key cannot be one of the two names the layout keeps for
/// itself, so a capture can never be mistaken for a move (111, 203).
#[test]
fn a_key_never_shadows_the_layout() {
    let (_store, mut world, _defs) = a_store("reserved-key");
    for key in ["moves/s1", "captures/x"] {
        let refused = signals::write_capture(
            &mut world,
            &Capture {
                key: key.into(),
                ..Default::default()
            },
        )
        .expect_err("a key shadowing the layout");
        assert!(refused.to_string().contains("keeps for itself"), "{refused}");
    }
}

// ---------------------------------------------- 9.5 a signal is never rewritten

/// An instance on disk: real git repositories, so a test can read the history
/// of the file a record was written to (113, 167).
struct Instance {
    dir: std::path::PathBuf,
    key_from: String,
}

impl Instance {
    fn new(name: &str) -> Instance {
        let dir = dir(&format!("instance-{name}"));
        let key_from = format!("FLYWHEEL_SIGNALS_KEY_{}", name.to_uppercase());
        std::env::set_var(&key_from, "the operator placed this");
        let ask = flywheel::init::Init {
            instance: "willdan".into(),
            host: "mac-mini".into(),
            root: dir.join("root"),
            git_host: dir.join("git-host"),
            app: "12345".into(),
            app_key_from: key_from.clone(),
            manifest: dir.join("flywheel.yaml"),
            state: dir.join("store.json"),
        };
        flywheel::init::run(ask).expect("the instance bootstraps");
        let instance = Instance { dir, key_from };
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

    /// How many commits touched one path on the blueprints' shared line.
    fn commits_touching(&self, path: &str) -> usize {
        let checkout = self
            .dir
            .join("root")
            .join("willdan")
            .join("flywheel-blueprints");
        let out = std::process::Command::new("git")
            .current_dir(&checkout)
            .args(["log", "--oneline", "--", path])
            .output()
            .expect("git log");
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .count()
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        std::env::remove_var(&self.key_from);
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A signal carries what it asserts and is immutable once written: no tool
/// edits one, and its record is never rewritten in history. A correction is a
/// new signal or a change of move (113, 193).
#[test]
fn signal_is_never_rewritten() {
    let instance = Instance::new("never-rewritten");
    let mut world = instance.world();

    let signal = signals::Signal {
        id: "signal/meeting-2026-09-02-willdan-weekly".into(),
        capture: "capture/meeting-2026-09-02-willdan-weekly".into(),
        // Its kind, from the small fixed set (113).
        kind: "constraint".into(),
        asserted_by: "dana".into(),
        subject_tags: vec!["providers".into(), "leases".into()],
        assertion: "only one host may write to a provider at a time".into(),
        excerpt: "That is the one-writer claim, isn't it.".into(),
        position: "00:04:29.400".into(),
        argues_with: vec!["providers/one-writer@3".into()],
    };
    let key = "meeting/2026-09-02/willdan-weekly";
    assert!(signals::write_signal(&mut world, key, 1, &signal).unwrap());

    // Everything 113 asks for is in the record, and it reads back whole.
    let read = signals::signals_of(&world, key).unwrap();
    assert_eq!(read, vec![signal.clone()], "the record lost a field");
    assert!(
        signals::KINDS.contains(&signal.kind.as_str()),
        "a kind outside the fixed set (113)"
    );

    let path = signals::signal_path(key, 1);
    assert_eq!(
        instance.commits_touching(&path),
        1,
        "the signal's record took more than one commit"
    );

    // A second write of the same signal, saying something else, is not made:
    // the record already there stands (113).
    let corrected = signals::Signal {
        assertion: "two hosts may write, actually".into(),
        ..signal.clone()
    };
    assert!(
        !signals::write_signal(&mut world, key, 1, &corrected).unwrap(),
        "a signal was rewritten"
    );
    assert_eq!(signals::signals_of(&world, key).unwrap(), vec![signal]);
    assert_eq!(
        instance.commits_touching(&path),
        1,
        "the signal's record was rewritten in history (113, 167)"
    );

    // And no tool of the catalogue edits one: `revive` clears a move, which is
    // the move changing and never the signal (107, 113, 193).
    for tool in catalogue::catalogue() {
        assert!(
            !matches!(tool.name, "edit-signal" | "amend-signal" | "correct-signal"),
            "the catalogue carries `{}`",
            tool.name
        );
        if tool.args.contains(&"signal") {
            assert_eq!(
                tool.name, "revive",
                "`{}` takes a signal; only `revive` may, and it clears the move (107)",
                tool.name
            );
        }
    }
}

// ---------------------------------------- 9.6 an older record reads unconverted

/// The signal and move formats are versioned and stable: a record written
/// under an earlier version is read as it stands, with no migration step, so
/// captures made before the flywheel existed read without conversion (114).
#[test]
fn older_signal_reads_unconverted() {
    let instance = Instance::new("older-format");
    let mut world = instance.world();

    // The formats say which version wrote them (114).
    assert_eq!(signals::CAPTURE_FORMAT, "flywheel-capture/1");
    assert_eq!(signals::SIGNAL_FORMAT, "flywheel-signal/1");
    assert_eq!(signals::MOVE_FORMAT, "flywheel-move/1");
    for format in [
        signals::CAPTURE_FORMAT,
        signals::SIGNAL_FORMAT,
        signals::MOVE_FORMAT,
    ] {
        let (name, version) = format.split_once('/').expect("a versioned format name");
        assert!(!name.is_empty() && version.parse::<u32>().is_ok(), "{format}");
    }

    // A record an earlier version wrote, put where this one would look for it.
    let older = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/older-signal.rec"),
    )
    .expect("the fixture");
    assert!(
        !older.contains("format:"),
        "the fixture is not in the earlier format"
    );
    let key = "inbox/2024-11-03";
    world
        .write_file(
            signals::BLUEPRINTS,
            &signals::signal_path(key, 1),
            older.as_bytes(),
            None,
        )
        .expect("the older record is placed");

    // It reads, as it stands: the fields it has are read and the ones it never
    // carried are simply absent. Nothing migrated it (114).
    let read = signals::signals_of(&world, key).expect("the older record reads");
    assert_eq!(read.len(), 1, "{read:?}");
    let signal = &read[0];
    assert_eq!(signal.id, "signal/inbox-2024-11-03-1");
    assert_eq!(signal.capture, "capture/inbox-2024-11-03");
    assert_eq!(signal.kind, "constraint");
    assert_eq!(signal.asserted_by, "dana");
    assert_eq!(
        signal.assertion,
        "only one host may write to a provider at a time"
    );
    assert_eq!(
        signal.excerpt,
        "both wrote to the same provider inside a minute"
    );
    // What the earlier version did not write is absent, not invented.
    assert_eq!(signal.position, "");
    assert!(signal.argues_with.is_empty());
    assert!(signal.subject_tags.is_empty());

    // And reading it wrote nothing back: no migration step touched the file
    // (114, 78).
    assert_eq!(
        instance.commits_touching(&signals::signal_path(key, 1)),
        1,
        "reading the older record rewrote it"
    );
    assert_eq!(
        String::from_utf8(
            world
                .read_file(signals::BLUEPRINTS, &signals::signal_path(key, 1))
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        older,
        "the older record was converted on read"
    );

    // A move written under an earlier version reads the same way.
    let older_move = "%rec: move\n\nsignal: signal/inbox-2024-11-03-1\ntarget: drop\nreason: superseded\n";
    world
        .write_file(
            signals::BLUEPRINTS,
            &signals::move_path("signal/inbox-2024-11-03-1"),
            older_move.as_bytes(),
            None,
        )
        .expect("the older move is placed");
    let moved = signals::standing_move(&world, "signal/inbox-2024-11-03-1")
        .expect("a read")
        .expect("the move");
    assert_eq!(moved.word(), "drop");
    assert_eq!(moved.reason, "superseded");
    assert_eq!(moved.at, "", "a date the earlier version never wrote");
}

// ------------------------------------- 9.7 one standing move per signal

/// Every signal has exactly one standing move — the signal id, the target, the
/// reason and the date — and curation sees only the signals that have none
/// (107).
#[test]
fn one_standing_move_per_signal() {
    let instance = Instance::new("one-move");
    let mut world = instance.world();
    let key = "meeting/2026-09-02/willdan-weekly";

    // Three signals from one transcript, none of them moved.
    for (n, assertion) in [
        (1u64, "only one host may write to a provider at a time"),
        (2, "the retry jitter is not in the spec"),
        (3, "the status copy says pending when it means queued"),
    ] {
        signals::write_signal(
            &mut world,
            key,
            n,
            &signals::Signal {
                id: signals::signal_object(key, n),
                capture: signals::object_of(key),
                kind: "constraint".into(),
                asserted_by: "dana".into(),
                assertion: assertion.into(),
                excerpt: assertion.into(),
                ..Default::default()
            },
        )
        .unwrap();
    }
    assert_eq!(signals::unmoved(&signals::Blueprints(&world)).len(), 3);

    // One move, with everything 107 asks of it.
    let first = signals::signal_object(key, 1);
    let attached = signals::Move {
        signal: first.clone(),
        target: "attach intent/atlas-provider-limits".into(),
        reason: "it is evidence on the open intent".into(),
        at: at(0).to_rfc3339(),
    };
    assert!(signals::write_move(&mut world, &attached).unwrap());
    let standing = signals::standing_move(&world, &first)
        .unwrap()
        .expect("the move");
    assert_eq!(standing, attached);
    assert_eq!(standing.word(), "attach");
    assert_eq!(standing.names(), "intent/atlas-provider-limits");

    // Exactly one: a second move on the same signal replaces the first rather
    // than standing beside it (107).
    let challenged = signals::Move {
        signal: first.clone(),
        target: "challenge providers/one-writer@3".into(),
        reason: "the operator said so".into(),
        at: at(5).to_rfc3339(),
    };
    assert!(signals::write_move(&mut world, &challenged).unwrap());
    assert_eq!(
        signals::standing_move(&world, &first).unwrap().unwrap(),
        challenged,
        "the replacement did not stand"
    );
    assert_eq!(
        signals::moves(&world).unwrap().len(),
        1,
        "a signal carries two moves"
    );

    // Curation sees only the unmoved ones, and never re-judges a moved one
    // (107).
    let files = signals::Blueprints(&world);
    let unmoved: Vec<String> = signals::unmoved(&files)
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(
        unmoved,
        vec![signals::signal_object(key, 2), signals::signal_object(key, 3)],
        "curation would re-judge a signal that has a move"
    );
    assert_eq!(
        signals::evidence(&files, &first, "signal.move"),
        Some(json!("challenge"))
    );
    assert_eq!(
        signals::evidence(&files, "curation/willdan", "curation.unmoved_count"),
        Some(json!(2))
    );

    // The operator's response is what replaces a move, and clearing one makes
    // the signal unmoved again so the next run clusters it (107, S24).
    signals::clear_move(&mut world, &first).unwrap();
    assert_eq!(signals::standing_move(&world, &first).unwrap(), None);
    let files = signals::Blueprints(&world);
    assert_eq!(signals::unmoved(&files).len(), 3);
    assert_eq!(
        signals::evidence(&files, &first, "signal.move"),
        Some(json!("none"))
    );

    // A move outside the six is no move at all (107).
    let refused = signals::write_move(
        &mut world,
        &signals::Move {
            signal: first,
            target: "done".into(),
            ..Default::default()
        },
    )
    .expect_err("a move outside the six");
    assert!(refused.to_string().contains("is no move"), "{refused}");
}

// ------------------------------------------ 9.8 the moves and their consequences

/// Every move has a stated consequence, and a challenge's is to record the
/// claim it argues with by name and version — and nothing of the ledger, which
/// belongs to the phase that has one (116, 101).
#[test]
fn challenge_records_claim_only() {
    let instance = Instance::new("challenge");
    let mut world = instance.world();
    let (mut store, _files, defs) = a_store("challenge-store");
    let key = "meeting/2026-09-02/willdan-weekly";
    let signal = signals::signal_object(key, 1);
    let at0 = at(0);

    // A signal, an open intent for it to attach to, and a proposed one to join.
    commands::put_new(&mut store, &defs, &signal, "signal", None, Default::default(), at0).unwrap();
    for intent in ["intent/atlas-provider-limits", "intent/retry-jitter"] {
        commands::put_new(&mut store, &defs, intent, "intent", None, Default::default(), at0)
            .unwrap();
    }

    // The challenge: the claim by name and version, on the signal (116).
    let applied = signals::apply_move(
        &mut store,
        &mut world,
        &signals::Move {
            signal: signal.clone(),
            target: "challenge providers/one-writer@3".into(),
            reason: "switchboard has its own client and nothing says only one writes".into(),
            at: at0.to_rfc3339(),
        },
        at0,
    )
    .unwrap();
    assert_eq!(applied.claim.as_deref(), Some("providers/one-writer"));
    assert_eq!(applied.claim_version, Some(3));
    let held = Records::get(&store, &signal).unwrap().unwrap();
    assert_eq!(
        held.record.get("argues_with").unwrap(),
        &json!(["providers/one-writer@3"]),
        "the signal does not record the claim it argues with"
    );

    // And nothing of the ledger was attempted: no cell was written, no ledger
    // file exists, and nothing was read from one (101, A.14 is phase 3).
    let cells: Vec<_> = Records::list_records(&store, &flywheel_atoms::Scope::All)
        .unwrap()
        .into_iter()
        .filter(|o| o.machine == "ledger-cell" || o.id.starts_with("cell/"))
        .collect();
    assert!(cells.is_empty(), "a challenge wrote a ledger cell: {cells:?}");
    assert!(
        world.list_files(signals::BLUEPRINTS, "flywheel/ledger/").unwrap().is_empty(),
        "a challenge wrote into the ledger"
    );

    // The other five have their own consequences (116).
    let cases = [
        ("attach intent/atlas-provider-limits", Some("intent/atlas-provider-limits")),
        ("join intent/retry-jitter", Some("intent/retry-jitter")),
        ("answered providers/one-writer@4", Some("providers/one-writer@4")),
        ("route offer/chore-1", Some("offer/chore-1")),
        ("drop", None),
    ];
    for (target, expected) in cases {
        let applied = signals::apply_move(
            &mut store,
            &mut world,
            &signals::Move {
                signal: signal.clone(),
                target: target.into(),
                reason: "curation said so".into(),
                at: at0.to_rfc3339(),
            },
            at0,
        )
        .unwrap();
        assert_eq!(applied.target.as_deref(), expected, "`{target}`");
        // One standing move, whatever came before it (107).
        assert_eq!(
            signals::standing_move(&world, &signal).unwrap().unwrap().target,
            target
        );
    }

    // Attach and join put the signal on the intent, which says how many it has
    // (109, 116).
    for intent in ["intent/atlas-provider-limits", "intent/retry-jitter"] {
        let held = Records::get(&store, intent).unwrap().unwrap();
        assert_eq!(held.record.get("signals").unwrap(), &json!([signal.clone()]));
        assert_eq!(held.record.get("signals_count").unwrap(), &json!(1));
    }

    // Still no ledger, after every move (101).
    assert!(world
        .list_files(signals::BLUEPRINTS, "flywheel/ledger/")
        .unwrap()
        .is_empty());
}

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
