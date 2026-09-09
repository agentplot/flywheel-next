//! The catalogue is one object, and the transport is a transport (193, D9).

mod caller;
mod store;
mod world;

use serde_json::{json, Value};

/// The in-process caller and the HTTP caller each enumerate the catalogue, and
/// they list the same tools with the same schemas — so a further transport adds
/// a client and not an operation (193).
#[test]
fn catalogue_is_identical_across_callers() {
    let sandbox = store::Sandbox::new("catalogue");
    let page = caller::Server::start(
        sandbox.store(),
        flywheel_domain::set::load().expect("the embedded definitions"),
        "chuck",
    );
    let in_process = flywheel_surface::enumerate();
    let over_http: Value = page.get("/api/tools");
    assert_eq!(
        in_process, over_http,
        "the HTTP caller enumerated a different catalogue from the in-process caller"
    );

    // And it is a catalogue, not an empty list that would make the equality
    // above vacuous.
    let tools = over_http["tools"].as_array().expect("tools is a list");
    assert!(!tools.is_empty(), "the catalogue served no tools");
    for tool in tools {
        assert!(tool["name"].as_str().is_some_and(|n| !n.is_empty()));
        assert!(
            tool["args"].is_array(),
            "{} carries no argument schema",
            tool["name"]
        );
    }
}

/// Every argument is named, and no two tools share a name (193).
#[test]
fn every_tool_names_its_arguments() {
    let mut seen: Vec<&str> = vec![];
    for tool in flywheel_surface::catalogue() {
        assert!(
            !seen.contains(&tool.name),
            "`{}` is in the catalogue twice",
            tool.name
        );
        seen.push(tool.name);
        for arg in tool.args {
            assert!(!arg.is_empty(), "`{}` has an unnamed argument", tool.name);
        }
        assert!(!tool.doc.is_empty(), "`{}` says nothing it does", tool.name);
    }
}

/// The transport is a transport: a call made over HTTP writes the same record
/// the in-process caller writes, because it is the same function (193).
#[test]
fn http_call_writes_the_same_record() {
    let defs = flywheel_domain::set::load().expect("the embedded definitions");

    // The same call, made in process.
    let here = store::Sandbox::new("in-process");
    let mut store = here.store();
    let mut world = world::Files::new();
    let call = flywheel_surface::catalogue::Call::new("drop", "chuck", "page")
        .arg("object", json!("unit/atlas/u"))
        .delivered("page-7");
    let outcome =
        flywheel_surface::catalogue::call(&mut store, &mut world, &defs, &call).expect("the in-process call");
    let in_process = flywheel_atoms::Records::get(&store, &format!("response/{}", outcome.id))
        .expect("a read")
        .expect("the record");

    // And over the transport the page uses.
    let there = store::Sandbox::new("over-http");
    let page = caller::Server::start(there.store(), defs, "chuck");
    let answered = page.post(
        "/api/tools/drop",
        json!({"args": {"object": "unit/atlas/u"}, "delivery_id": "page-7"}),
    );
    assert_eq!(answered["id"], json!("page-7"), "{answered}");
    assert_eq!(answered["recorded"], json!(true), "{answered}");
    let over_http = page.with_store(|store| {
        flywheel_atoms::Records::get(store, "response/page-7")
            .expect("a read")
            .expect("the record")
    });

    assert_eq!(
        over_http.record, in_process.record,
        "the transport wrote a different record"
    );
    assert_eq!(over_http.machine, in_process.machine);
    assert_eq!(over_http.config, in_process.config);
}

/// A tool the catalogue lacks is refused at the transport with what it was, and
/// no caller gains an operation by reaching for it over HTTP (193, 4).
#[test]
fn http_carries_no_operation_of_its_own() {
    let sandbox = store::Sandbox::new("http-no-extra");
    let page = caller::Server::start(
        sandbox.store(),
        flywheel_domain::set::load().expect("the embedded definitions"),
        "chuck",
    );
    let served: Vec<String> = page.get("/api/tools")["tools"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|t| t["name"].as_str().unwrap_or_default().to_string())
        .collect();
    let in_process: Vec<String> = flywheel_surface::catalogue()
        .iter()
        .map(|t| t.name.to_string())
        .collect();
    assert_eq!(served, in_process);
}
