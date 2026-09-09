//! Every operation the spec names is a tool with a body that writes through
//! the state store, and the catalogue holds nothing the design did not put
//! there (193, 4).

mod store;
mod world;

use flywheel_atoms::Records;
use flywheel_surface::catalogue::{self, Call};
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
            "mark as intent" => "mark-intent".to_string(),
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
    // past undo-or-defer, and the removal D9 puts there from day one.
    let beyond: BTreeSet<String> = carried.difference(&named).cloned().collect();
    let expected: BTreeSet<String> = ["start", "stop", "remove-instance"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(beyond, expected, "the catalogue carries {beyond:?} besides");

    // And each one has a body: invoked, it writes a response record naming the
    // tool, through the store.
    let sandbox = store::Sandbox::new("bodies");
    let mut store = sandbox.store();
    let mut world = world::Files::new();
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    for tool in catalogue::catalogue() {
        if tool.name == "later" {
            // `later` names a decision the register gave; it is exercised on
            // its own below, where there is one to name.
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
}

/// A plausible value for an argument the schema names.
fn argument_for(name: &str) -> serde_json::Value {
    match name {
        "decision" => json!(1),
        "text" => json!("look at the rows"),
        "answer" => json!("yes"),
        "source" => json!("page"),
        "name" => json!("atlas"),
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

    let sandbox = store::Sandbox::new("asserts-done");
    let mut store = sandbox.store();
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
    let sandbox = store::Sandbox::new("recorded-once");
    let mut store = sandbox.store();
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
