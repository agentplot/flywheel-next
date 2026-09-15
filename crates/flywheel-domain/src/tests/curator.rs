//! The curator session's work order and what its delivery becomes: the order
//! lists the unmoved signals, the standing claims and the open intents; the
//! moves and intent proposals it delivers are parsed against their schemas and
//! written through `record_moves` and `propose_intents` (107, 109, 110, 116;
//! model.md §9).

use crate::report::{deliver, write_report, Report};
use crate::{changes, commands, offers, order, signals};
use chrono::{DateTime, TimeZone, Utc};
use flywheel_atoms::testing::{FakeStore, FakeWorld};
use flywheel_atoms::Records;
use serde_json::json;
use std::collections::BTreeMap;

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap()
}

const KEY: &str = "signals/signals/2026-09-08-dev-standup";

/// Four signals read before, as a signals folder carries them in.
fn four_signals(world: &mut FakeWorld) -> Vec<String> {
    let capture = signals::Capture {
        key: KEY.into(),
        source: "wispr-flow".into(),
        event_at: "2026-09-08".into(),
        captured_by: "chuck".into(),
        raw: "signals/.raw/dev-standup.txt".into(),
    };
    signals::write_capture(world, &capture).unwrap();
    let said = [
        ("ask", "who owns the export when two teams publish one schema"),
        ("question", "whether the export should be versioned per team"),
        ("constraint", "the rows lose their numbers on the second page"),
        ("reaction", "the demo ran long"),
    ];
    let mut ids = Vec::new();
    for (at, (kind, assertion)) in said.iter().enumerate() {
        let ordinal = at as u64 + 1;
        let signal = signals::Signal {
            id: signals::signal_object(KEY, ordinal),
            capture: signals::object_of(KEY),
            kind: kind.to_string(),
            asserted_by: "Amy".into(),
            subject_tags: vec!["export".into()],
            assertion: assertion.to_string(),
            excerpt: format!("\"{assertion}\""),
            position: "line 12".into(),
            argues_with: match ordinal {
                3 => vec!["page/the-page-is-a-capture-surface".into()],
                _ => vec![],
            },
        };
        signals::write_signal(world, KEY, ordinal, &signal).unwrap();
        ids.push(signal.id);
    }
    ids
}

/// The standing specification a challenge may name.
fn a_standing_spec(world: &mut FakeWorld) {
    world.files.insert(
        "openspec/specs/page/spec.md".into(),
        "## Purpose\n\n### Requirement: The page is a capture surface\n\nThe box SHALL…\n".into(),
    );
}

/// The curator's order carries every unmoved signal with its kind, who said it,
/// its subjects, the claims it argues with, its assertion and its excerpt; the
/// standing claims by name with the file each stands in; the open intents with
/// their subjects; the types and repositories a proposal or a route may name;
/// and the two files it delivers in (model.md §9; context.yaml
/// sessions.curation).
#[test]
fn the_curators_order_lists_signals_claims_and_intents() {
    let mut world = FakeWorld::new();
    let ids = four_signals(&mut world);
    a_standing_spec(&mut world);
    let inputs = order::Curation {
        signals: signals::unmoved(&signals::Blueprints(&world)),
        claims: changes::standing_claims(&signals::Blueprints(&world)),
        intents: vec![("intent/rows-keep-numbers".into(), "the rows keep their numbers".into(), 2)],
        types: vec!["self-closing".into(), "standing".into()],
        repositories: vec!["flywheel-next".into(), "blueprints".into()],
    };
    let rendered = order::curation(&inputs);
    for id in &ids {
        assert!(rendered.contains(&format!("`{id}`")), "the order lacks {id}: {rendered}");
    }
    for shown in [
        "· ask · Amy · subjects: export",
        "  who owns the export when two teams publish one schema",
        "  > \"the rows lose their numbers on the second page\"",
        "argues with: page/the-page-is-a-capture-surface",
        "- `page/the-page-is-a-capture-surface` · The page is a capture surface · openspec/specs/page/spec.md",
        "- `intent/rows-keep-numbers` · the rows keep their numbers · 2 signal(s) attached",
        "self-closing, standing",
        "flywheel-next, blueprints",
        ".flywheel/deliverables/move.rec",
        ".flywheel/deliverables/intent-proposal.rec",
        "--deliverable move --deliverable intent-proposal",
    ] {
        assert!(rendered.contains(shown), "the order lacks `{shown}`: {rendered}");
    }
}

/// A curator's delivery is parsed against the schemas when its offers are
/// recorded: the moves and the proposal the schemas admit are written on its
/// thread, and a move with no reason, a second move on one signal, a move on a
/// signal not on record and a proposal of a type the registry does not know are
/// refused there. `record_moves` writes each admitted move, and `propose_intents`
/// one proposed intent per join with its subject, its signals and so its weight;
/// a second pass parses nothing again (107, 109, 116, 80, 127).
#[test]
fn a_curators_delivery_writes_its_moves_and_proposed_intents() {
    let defs = crate::set::load().unwrap();
    let mut store = FakeStore::default();
    let mut world = FakeWorld::new();
    let ids = four_signals(&mut world);
    a_standing_spec(&mut world);
    commands::put_new(&mut store, &defs, "curation/willdan", "curation", None, Default::default(), now()).unwrap();
    commands::put_new(&mut store, &defs, "intent/rows-keep-numbers", "intent", None, Default::default(), now()).unwrap();
    let session = "curation/willdan/main/1";

    let moves = format!(
        "Signal: {a}\nMove: join\nTarget: intent/export-ownership\nReason: both ask who owns a shared export\n\n\
         Signal: {b}\nMove: join\nTarget: intent/export-ownership\nReason: versioning per team is the same question\n\n\
         Signal: {c}\nMove: challenge\nTarget: page/the-page-is-a-capture-surface\nReason: the rows lose their numbers there\n\n\
         Signal: {d}\nMove: drop\nReason:\n\n\
         Signal: {a}\nMove: drop\nReason: a second look\n\n\
         Signal: signal/nowhere\nMove: drop\nReason: gone\n",
        a = ids[0],
        b = ids[1],
        c = ids[2],
        d = ids[3],
    );
    let proposals = format!(
        "Intent: intent/export-ownership\nSubject: who owns an export two teams publish\nSignals: {} {}\nElaboration: self-closing\n\n\
         Intent: intent/export-versions\nSubject: whether exports are versioned per team\nSignals: {}\nElaboration: no-such-type\n",
        ids[0], ids[1], ids[1]
    );
    deliver(&mut store, session, "curator", now(), "move", &moves).unwrap();
    deliver(&mut store, session, "curator", now(), "intent-proposal", &proposals).unwrap();
    let exit = Report::Exit {
        kind: "done".into(),
        deliverables: vec!["move".into(), "intent-proposal".into()],
        question: None,
        text: None,
    };
    write_report(&mut store, session, "curator", now(), &exit).unwrap();
    assert_eq!(offers::deliveries_pending(&store, session).unwrap().len(), 2);

    offers::record(&mut store, &mut world, &defs, session, "curation/willdan", now()).unwrap();
    assert!(offers::deliveries_pending(&store, session).unwrap().is_empty(), "the delivery is parsed");
    let thread = store.thread(session).unwrap();
    let refused: Vec<String> = thread
        .iter()
        .filter(|e| e.kind == "refusal")
        .map(|e| e.fields.get("reason").and_then(|v| v.as_str()).unwrap_or_default().to_string())
        .collect();
    assert_eq!(refused.len(), 4, "{refused:?}");
    assert!(refused.iter().any(|r| r.contains("gives no reason")), "{refused:?}");
    assert!(refused.iter().any(|r| r.contains("moved twice")), "{refused:?}");
    assert!(refused.iter().any(|r| r.contains("no signal on record")), "{refused:?}");
    assert!(refused.iter().any(|r| r.contains("does not know")), "{refused:?}");

    let (delivered, stated) = offers::delivered(&store, session).unwrap();
    assert_eq!(delivered.len(), 3, "three moves are admitted: {delivered:?}");
    assert_eq!(stated.len(), 1, "one proposal is admitted: {stated:?}");
    signals::record_moves(&mut store, &mut world, &delivered, now()).unwrap();
    let proposed = offers::proposals_with(&delivered, &stated);
    signals::propose_intents(&mut store, &defs, &proposed, now()).unwrap();

    let moved = |signal: &str| signals::standing_move(&world, signal).unwrap().map(|m| m.target);
    assert_eq!(moved(&ids[0]).as_deref(), Some("join intent/export-ownership"));
    assert_eq!(moved(&ids[2]).as_deref(), Some("challenge page/the-page-is-a-capture-surface"));
    assert_eq!(moved(&ids[3]), None, "a move with no reason was written");
    let intent = store.get("intent/export-ownership").unwrap().expect("the proposed intent");
    assert_eq!(intent.config.get("life").map(String::as_str), Some("proposed"));
    assert_eq!(intent.record.get("subject"), Some(&json!("who owns an export two teams publish")));
    assert_eq!(intent.record.get("elaborations"), Some(&json!(["self-closing"])));
    let cited: Vec<String> = serde_json::from_value(intent.record.get("signals").cloned().unwrap()).unwrap();
    assert_eq!(cited, vec![ids[0].clone(), ids[1].clone()]);
    let files: BTreeMap<String, String> = world.files.clone();
    let weight = signals::weight_of(&files, &cited);
    assert_eq!((weight.count, weight.sources.as_slice()), (2, &["wispr-flow".to_string()][..]));
    assert_eq!(weight.span().as_deref(), Some("2026-09-08"));
    assert!(store.get("intent/export-versions").unwrap().is_none(), "a refused proposal made an intent");

    let before = store.thread(session).unwrap().len();
    offers::record(&mut store, &mut world, &defs, session, "curation/willdan", now()).unwrap();
    assert_eq!(store.thread(session).unwrap().len(), before, "a second pass parsed the delivery again");
}
