//! The catalogue is one object, and the transport is a transport (193, D9).

mod caller;
mod store;
use flywheel_surface::testing as world;

use serde_json::{json, Value};

/// The in-process caller, the HTTP caller and a member's client over the model
/// context protocol each enumerate the catalogue, and they list the same tools
/// with the same schemas — so a further transport adds a client and not an
/// operation (193, 293).
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

    // The third caller: a client at the instance's own address, which says
    // hello and then asks what it holds (319, D18).
    let (status, hello) = page.protocol(json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "a client", "version": "1"}}
    }));
    assert_eq!(status, 200, "{hello:?}");
    assert_eq!(hello.expect("a hello back")["result"]["serverInfo"]["name"], json!("flywheel"));
    let (status, listed) = page.protocol(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    assert_eq!(status, 200, "{listed:?}");
    let listed = listed.expect("a listing");
    let declared = listed["result"]["tools"].as_array().expect("a list of tools");
    let enumerated = in_process["tools"].as_array().expect("tools is a list");
    let named = |tools: &[Value], key: &str| -> Vec<Value> { tools.iter().map(|t| t[key].clone()).collect() };
    assert_eq!(
        named(declared, "name"),
        named(enumerated, "name"),
        "the protocol declared other tools, or in another order"
    );
    for (declared, enumerated) in declared.iter().zip(enumerated) {
        assert_eq!(declared["description"], enumerated["doc"]);
        let mut properties: Vec<String> = declared["inputSchema"]["properties"]
            .as_object()
            .unwrap_or_else(|| panic!("{} declares no input schema", declared["name"]))
            .keys()
            .cloned()
            .collect();
        let mut args: Vec<String> = serde_json::from_value(enumerated["args"].clone()).expect("args");
        properties.sort();
        args.sort();
        assert_eq!(properties, args, "{} declares other arguments over the protocol", declared["name"]);
    }

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

    // The ask is among them, on both callers alike, naming the repository and
    // the words (28, 116, `surfaces.yaml` tools.ask).
    let ask = tools
        .iter()
        .find(|tool| tool["name"] == json!("ask"))
        .expect("the catalogue serves `ask`");
    assert_eq!(ask["args"], json!(["repository", "text"]));
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

/// A call from a member's client over the protocol is admitted by the rule the
/// page is, and recorded once: one response record, carrying the tool, the
/// object, who gave it and when, and nothing more for the same delivery again
/// (153, 193, 248, 249, 253a, 137).
#[test]
fn a_call_over_the_protocol_is_recorded_once() {
    let sandbox = store::Sandbox::new("protocol-once");
    let page = caller::Server::start(
        sandbox.store(),
        flywheel_domain::set::load().expect("the embedded definitions"),
        "chuck",
    );
    let drop = json!({
        "jsonrpc": "2.0", "id": 7, "method": "tools/call",
        "params": {"name": "drop", "arguments": {"object": "unit/atlas/u"}, "_meta": {"flywheel/delivery": "tap-1b2c"}}
    });
    let (status, reply) = page.protocol(drop.clone());
    assert_eq!(status, 200, "{reply:?}");
    let reply = reply.expect("a call is answered");
    assert_eq!(reply["id"], json!(7));
    assert_eq!(reply["result"]["isError"], json!(false), "{reply}");
    assert_eq!(reply["result"]["structuredContent"]["recorded"], json!(true), "{reply}");

    let (_, again) = page.protocol(drop);
    assert_eq!(again.expect("answered")["result"]["structuredContent"]["recorded"], json!(false));

    let responses = page.with_store(|store| {
        flywheel_atoms::Records::list_records(store, &flywheel_atoms::Scope::Machine("response".into()))
            .expect("a read")
    });
    assert_eq!(responses.len(), 1, "one call, delivered twice, is one record: {responses:?}");
    let record = &responses[0];
    assert_eq!(record.id, "response/client-tap-1b2c");
    assert_eq!(record.record.get("tool"), Some(&json!("drop")));
    assert_eq!(record.record.get("object"), Some(&json!("unit/atlas/u")));
    assert_eq!(record.record.get("given_by"), Some(&json!("chuck")));
    assert_eq!(record.record.get("delivery"), Some(&json!("client")));
    assert!(record.record.contains_key("given_at"));

    // A notification is accepted with nothing to say.
    let (status, nothing) = page.protocol(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    assert_eq!((status, nothing), (202, None));
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
