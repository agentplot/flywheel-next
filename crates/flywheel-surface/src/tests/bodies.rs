//! Every operation the spec names is a tool with a body that writes through
//! the state store, and the catalogue holds nothing the design did not put
//! there (193, 4).

use crate::testing as world;

use flywheel_atoms::Records;
use crate::catalogue::{self, Call};
use serde_json::json;
use std::collections::BTreeSet;

/// The operations `specs/tools/tool-server/spec.md` enumerates, read from the
/// spec itself so the catalogue cannot drift from it silently.
fn spec_list() -> BTreeSet<String> {
    let spec = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../openspec/changes/the-loop/specs/tools/tool-server/spec.md"),
    )
    .expect("the tool-server spec");
    let opening = "Every operation the operator may invoke — ";
    let start = spec.find(opening).expect("the spec's enumeration") + opening.len();
    let rest = &spec[start..];
    let end = rest.find(" — ").expect("the enumeration's close");
    let named: BTreeSet<String> = rest[..end]
        .replace('\n', " ")
        .split(',')
        .map(|item| item.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|item| !item.starts_with("and the rest"))
        .map(|item| match item.as_str() {
            "propose a unit" => "propose-unit".to_string(),
            "answer a decision" => "answer".to_string(),
            other => other.to_string(),
        })
        .collect();
    named
}

/// The catalogue and the spec's list agree: every operation the spec names has
/// a body, and what the catalogue carries beyond them is what the design named
/// beyond them and nothing else (193, 4, 47, D9).
#[test]
fn every_named_tool_has_a_body() {
    let named = spec_list();
    assert_eq!(named.len(), 16, "the spec names {named:?}");

    let carried: BTreeSet<String> = catalogue::catalogue()
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    let missing: Vec<&String> = named.difference(&carried).collect();
    assert!(missing.is_empty(), "the catalogue lacks {missing:?}");

    // Beyond the spec's sixteen the catalogue carries the pair clause 47 grants
    // past undo-or-defer, the removal D9 puts there from day one, and the
    // curator's moves — the one write the page made as no tool until audit 6
    // (93b, 107, 116).
    let beyond: BTreeSet<String> = carried.difference(&named).cloned().collect();
    let expected: BTreeSet<String> = ["start", "stop", "remove-instance", "curate"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(beyond, expected, "the catalogue carries {beyond:?} besides");

    // And each one has a body: invoked, it writes a response record naming the
    // tool, through the store.
    let mut store = flywheel_atoms::testing::FakeStore::default();
    let mut world = world::Files::new();
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    for tool in catalogue::catalogue() {
        if tool.name == "later" {
            // `later` names a decision the register gave; it is exercised on
            // its own below, where there is one to name.
            continue;
        }
        if tool.name == "curate" {
            // `curate` is a session's delivery and writes the session's exit,
            // not a response record; it is exercised on its own below.
            continue;
        }
        let mut call = Call::new(tool.name, "chuck", "page");
        for argument in tool.args {
            call = call.arg(argument, argument_for(argument));
        }
        let outcome = catalogue::call(&mut store, &mut world, &defs, &call)
            .unwrap_or_else(|e| panic!("`{}` has no body: {e}", tool.name));
        let record = store
            .get(&format!("response/{}", outcome.id))
            .expect("a read")
            .unwrap_or_else(|| panic!("`{}` wrote no response record", tool.name));
        assert_eq!(
            record.record.get("tool").and_then(|v| v.as_str()),
            Some(tool.name),
            "`{}` recorded another tool",
            tool.name
        );
        assert_eq!(
            record.record.get("given_by").and_then(|v| v.as_str()),
            Some("chuck"),
            "`{}` recorded no identity",
            tool.name
        );
        assert!(
            record.record.contains_key("given_at"),
            "`{}` recorded no time",
            tool.name
        );
    }

    // `curate` has a body too: the moves land in the blueprints and the
    // session's thread carries the exit naming `move` as what it delivered
    // (67, 80, 107, 116).
    let session = "curation/willdan/main/1";
    let call = Call::new("curate", "chuck", "page")
        .arg("session", json!(session))
        .arg("moves", json!([{"signal": "signal/atlas/rows/1", "move": "drop", "target": ""}]));
    let outcome = catalogue::call(&mut store, &mut world, &defs, &call).expect("`curate` has a body");
    assert_eq!(outcome.id, session);
    let exit = store
        .thread(session)
        .expect("a read")
        .into_iter()
        .find(|e| e.kind == "exit")
        .expect("the session reported its exit");
    assert_eq!(exit.fields.get("deliverables"), Some(&json!(["move"])));
    let standing = flywheel_domain::signals::standing_move(&world, "signal/atlas/rows/1")
        .expect("a read")
        .expect("the move stands");
    assert_eq!(standing.target, "drop");
}

/// A plausible value for an argument the schema names.
fn argument_for(name: &str) -> serde_json::Value {
    match name {
        "decision" => json!(1),
        "text" => json!("look at the rows"),
        "answer" => json!("yes"),
        "source" => json!("page"),
        "name" => json!("atlas"),
        // `propose-unit`: a bolt given whole, and the one-stage type (34, 60).
        "bolt" => json!("bolt/atlas/one"),
        "type" => json!("chore"),
        other => json!(format!("{other}/one")),
    }
}

/// A tool that would assert work was done does not exist, and a call claiming
/// one is not silently dropped: it is recorded as the response it was, for the
/// response machine to report unapplicable (4, 6).
#[test]
fn a_tool_asserting_done_is_not_in_the_catalogue() {
    for asserted in ["done", "pass", "complete", "finished-it"] {
        assert!(
            catalogue::tool(asserted).is_none(),
            "the catalogue carries `{asserted}`"
        );
        assert!(
            !flywheel_domain::commands::is_operation(asserted),
            "`{asserted}` is an operation the response machine would apply"
        );
    }

    let mut store = flywheel_atoms::testing::FakeStore::default();
    let mut world = world::Files::new();
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let call = Call::new("done", "chuck", "chat").arg("object", json!("work-item/atlas/v/1"));
    let outcome = catalogue::call(&mut store, &mut world, &defs, &call).expect("recorded, not dropped");
    let record = store
        .get(&format!("response/{}", outcome.id))
        .expect("a read")
        .expect("the claim was recorded");
    assert_eq!(
        record.record.get("tool").and_then(|v| v.as_str()),
        Some("done"),
        "the claim was recorded as some other tool"
    );
}

/// One call, one response record — carrying the tool, the object, who gave it
/// and when — however many times the delivery arrives (153, 137).
#[test]
fn call_recorded_once() {
    let mut store = flywheel_atoms::testing::FakeStore::default();
    let mut world = world::Files::new();
    let defs = flywheel_domain::set::load().expect("the embedded definitions");

    let call = Call::new("drop", "chuck", "chat")
        .arg("object", json!("unit/atlas/u"))
        .delivered("discord-1");

    let first = catalogue::call(&mut store, &mut world, &defs, &call).expect("the first delivery");
    let again = catalogue::call(&mut store, &mut world, &defs, &call).expect("the same delivery again");
    assert_eq!(first.id, again.id, "the repeat took a delivery id of its own");
    assert!(
        matches!(again.outcome, flywheel_atoms::Received::AlreadyApplied { .. }),
        "the repeat was not recognised: {:?}",
        again.outcome
    );

    let records: Vec<String> = flywheel_atoms::StateStore::list(&store, &flywheel_atoms::Scope::Machine("response".into()))
        .expect("a listing")
        .objects
        .iter()
        .map(|o| o.id.clone())
        .collect();
    assert_eq!(
        records,
        vec!["response/discord-1".to_string()],
        "the call was recorded more than once"
    );

    let record = store
        .get("response/discord-1")
        .expect("a read")
        .expect("the record");
    assert_eq!(record.record.get("tool").and_then(|v| v.as_str()), Some("drop"));
    assert_eq!(
        record.record.get("object").and_then(|v| v.as_str()),
        Some("unit/atlas/u")
    );
    assert_eq!(
        record.record.get("given_by").and_then(|v| v.as_str()),
        Some("chuck")
    );
    assert!(record.record.contains_key("given_at"), "the record carries no time");

    // And the store holds one response for that delivery, not two (137).
    let held = store.responses(flywheel_domain::RAIL).expect("the responses");
    assert_eq!(
        held.iter().filter(|r| r.id == "discord-1").count(),
        1,
        "the store holds the delivery twice"
    );
}

/// `propose-unit` is the operator's dictation naming a bolt, applied directly
/// (34, 12): the unit stands in `approved` with the call as its approval, the
/// bolt it names is made when it does not exist, its one item is made, and the
/// capture it came from is its document with the capture's own words as the
/// job (19, I1, `surfaces.yaml` propose-unit).
#[test]
fn propose_unit_approves_a_unit_on_a_new_bolt_with_its_item() {
    let mut store = flywheel_atoms::testing::FakeStore::default();
    let mut world = world::Files::new();
    let defs = flywheel_domain::set::load().expect("the embedded definitions");

    // The capture's one ask signal, as the page's box writes it (19).
    let signal = flywheel_domain::signals::signal_object("page/2026-09-14/tidy", 1);
    let at = flywheel_domain::commands::now(&store).expect("a point");
    flywheel_domain::commands::put_new(
        &mut store,
        &defs,
        &signal,
        "signal",
        Some("capture/page/2026-09-14/tidy"),
        [("assertion".to_string(), json!("the rows lose their numbers on the second page"))]
            .into_iter()
            .collect(),
        at,
    )
    .expect("the signal");

    let call = Call::new("propose-unit", "chuck", "page")
        .arg("bolt", json!("bolt/atlas/Plan Rows"))
        .arg("capture", json!("capture/page/2026-09-14/tidy"));
    let outcome = catalogue::call(&mut store, &mut world, &defs, &call).expect("the dictation is applied");

    let unit = store.get("unit/atlas/plan-rows").expect("a read").expect("the unit exists");
    assert_eq!(unit.config.get("life").map(String::as_str), Some("approved"), "approved by the call (12)");
    assert_eq!(unit.record.get("approval").and_then(|v| v.as_str()), Some(outcome.id.as_str()), "the approval is the response (I1)");
    assert_eq!(unit.record.get("type").and_then(|v| v.as_str()), Some("chore"));
    assert_eq!(unit.record.get("type_version").and_then(|v| v.as_u64()), Some(2));
    assert_eq!(unit.parent.as_deref(), Some("bolt/atlas/plan-rows"), "the unit is the bolt's");
    assert_eq!(unit.record.get("document").and_then(|v| v.as_str()), Some("capture/page/2026-09-14/tidy"));
    assert_eq!(
        unit.record.get("subject").and_then(|v| v.as_str()),
        Some("the rows lose their numbers on the second page"),
        "the capture's words are the job"
    );

    let bolt = store.get("bolt/atlas/plan-rows").expect("a read").expect("the bolt was made (34)");
    assert_eq!(bolt.config.get("life").map(String::as_str), Some("open"));
    let items: Vec<_> = store
        .list_records(&flywheel_atoms::Scope::All)
        .expect("a listing")
        .into_iter()
        .filter(|o| o.machine == "work-item" && o.parent.as_deref() == Some("unit/atlas/plan-rows"))
        .collect();
    assert_eq!(items.len(), 1, "one item, of the unit's type");
    assert_eq!(items[0].record.get("type").and_then(|v| v.as_str()), Some("chore"));

    // Given twice, a name is given once (I1).
    let again = catalogue::call(&mut store, &mut world, &defs, &call);
    assert!(again.is_err(), "the same unit is not made twice");

    // A type that is no unit type is refused, and so is a name for nothing.
    let wrong = Call::new("propose-unit", "chuck", "page")
        .arg("bolt", json!("bolt/atlas/other"))
        .arg("type", json!("standing"));
    assert!(catalogue::call(&mut store, &mut world, &defs, &wrong).is_err(), "an elaboration type is no unit type");
}

/// `build_from_signal` is the operator's build on a signal's rail card (19a):
/// a chore unit in approved on a bolt named from the capture's own words, on
/// the one tracked repository, the capture as its document, the response that
/// said build as its approval; and the signal's one move is route, naming the
/// unit (12, 34, 107, 116). The operator types no name.
#[test]
fn build_from_signal_makes_the_unit_and_routes_the_signal() {
    let mut store = flywheel_atoms::testing::FakeStore::default();
    let mut world = world::Files::new().tracking("atlas");
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let signal = flywheel_domain::signals::signal_object("page-7", 1);
    let at = flywheel_domain::commands::now(&store).expect("a point");
    flywheel_domain::commands::put_new(
        &mut store,
        &defs,
        &signal,
        "signal",
        Some("capture/page-7"),
        [("assertion".to_string(), json!("The README's crates table lacks a row for the Herdr binding, I think."))]
            .into_iter()
            .collect(),
        at,
    )
    .expect("the signal");
    // The response that said build, as the answer tool records it.
    let call = Call::new("answer", "chuck", "page")
        .arg("decision", json!(1))
        .arg("answer", json!("build"));
    let _ = call;
    flywheel_domain::commands::put_new(
        &mut store,
        &defs,
        "response/page-9",
        "response",
        None,
        [
            ("object".to_string(), json!(signal)),
            ("answer".to_string(), json!("build")),
            ("given_by".to_string(), json!("chuck")),
        ]
        .into_iter()
        .collect(),
        at,
    )
    .expect("the response");

    let unit = catalogue::build_from_signal(&mut store, &mut world, &defs, &signal, at).expect("built");
    assert_eq!(unit, "unit/atlas/readme-crates-table-lacks", "named from its first meaningful words, and typed by nobody");
    let held = store.get(&unit).expect("a read").expect("the unit");
    assert_eq!(held.config.get("life").map(String::as_str), Some("approved"));
    assert_eq!(held.record.get("approval").and_then(|v| v.as_str()), Some("response/page-9"), "the response is the approval (I1)");
    assert_eq!(held.record.get("document").and_then(|v| v.as_str()), Some("capture/page-7"));
    assert_eq!(held.record.get("proposed_by").and_then(|v| v.as_str()), Some("chuck"));
    assert!(store.get("bolt/atlas/readme-crates-table-lacks").expect("a read").is_some(), "the bolt was made (34)");
    let moved = flywheel_domain::signals::moves(&flywheel_domain::signals::Blueprints(&world));
    assert_eq!(moved.len(), 1, "the signal's one move");
    assert_eq!(moved[0].target, format!("route {unit}"), "route, naming the unit (116)");

    // The same words again take the next name; a name is given once (I1).
    let again = catalogue::build_from_signal(&mut store, &mut world, &defs, &signal, at).expect("built again");
    assert_eq!(again, "unit/atlas/readme-crates-table-lacks-2");
}

/// The operator's own move on a signal, from the rail (19a): join proposes an
/// intent named from the capture's words and citing the signal; drop drops it
/// with the response as the reason (107, 109, 116).
#[test]
fn the_operators_move_on_a_signal_joins_or_drops_it() {
    let mut store = flywheel_atoms::testing::FakeStore::default();
    let mut world = world::Files::new();
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let at = flywheel_domain::commands::now(&store).expect("a point");
    for (n, said) in [(1, "checkout drops the cart on refresh"), (2, "the footer overlaps the cookie bar")] {
        flywheel_domain::commands::put_new(
            &mut store,
            &defs,
            &flywheel_domain::signals::signal_object(&format!("page-{n}"), 1),
            "signal",
            Some(&format!("capture/page-{n}")),
            [("assertion".to_string(), json!(said))].into_iter().collect(),
            at,
        )
        .expect("the signal");
    }
    let one = flywheel_domain::signals::signal_object("page-1", 1);
    let two = flywheel_domain::signals::signal_object("page-2", 1);

    let joined = flywheel_domain::signals::move_by_operator(&mut store, &mut world, &defs, &one, "join", "the rail", at)
        .expect("joined");
    assert_eq!(joined.target, "join intent/checkout-drops-cart-refresh");
    let intent = store.get("intent/checkout-drops-cart-refresh").expect("a read").expect("the intent is proposed (109)");
    assert_eq!(intent.config.get("life").map(String::as_str), Some("proposed"));
    assert_eq!(intent.record.get("signals"), Some(&json!([one.clone()])), "citing the signal, which is its weight");

    let dropped = flywheel_domain::signals::move_by_operator(&mut store, &mut world, &defs, &two, "drop", "the rail", at)
        .expect("dropped");
    assert_eq!(dropped.target, "drop");
    let moved = flywheel_domain::signals::moves(&flywheel_domain::signals::Blueprints(&world));
    assert_eq!(moved.len(), 2);
    assert!(flywheel_domain::signals::move_by_operator(&mut store, &mut world, &defs, &two, "attach", "the rail", at).is_err(), "attach is curation's, not the rail's");
}
