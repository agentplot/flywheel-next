//! The catalogue as a remote server of the model context protocol: the wire a
//! member's own client speaks, answered over the fake store (293, 319, D18).

use crate::protocol::{self, Caller};
use crate::testing as world;
use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::{Records, Scope};
use serde_json::{json, Value};

fn defs() -> flywheel_engine::Definitions {
    flywheel_domain::set::load().expect("the embedded definitions")
}

/// One message, answered for the operator `chuck`.
fn send(store: &mut FakeStore, world: &mut world::Files, message: Value) -> protocol::Handled {
    let defs = defs();
    let mut caller = Caller {
        store,
        world,
        defs: &defs,
        address: "http://studio.tailnet.ts.net/willdan",
        by: "chuck",
    };
    protocol::handle(&mut caller, &message)
}

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

/// `initialize` names the server and its version, says it serves tools and
/// resources, and answers in the version the client asked for where it speaks
/// that one, and in its newest otherwise (293, 307).
#[test]
fn initialize_names_the_server_and_what_it_serves() {
    let (mut store, mut world) = (FakeStore::default(), world::Files::new());
    let asked = request(1, "initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}}));
    let reply = send(&mut store, &mut world, asked).reply.expect("a request is answered");
    assert_eq!(reply["id"], json!(1));
    let result = &reply["result"];
    assert_eq!(result["protocolVersion"], json!("2025-06-18"));
    assert!(result["capabilities"]["tools"].is_object(), "{result}");
    assert!(result["capabilities"]["resources"].is_object(), "{result}");
    assert_eq!(result["serverInfo"]["name"], json!("flywheel"));
    assert_eq!(result["serverInfo"]["version"], json!(crate::page::VERSION));

    let unknown = request(2, "initialize", json!({"protocolVersion": "1999-01-01"}));
    let reply = send(&mut store, &mut world, unknown).reply.expect("answered");
    assert_eq!(reply["result"]["protocolVersion"], json!(protocol::VERSIONS[0]));
}

/// A notification and a response are answered with nothing; a ping with an
/// empty result; a method the server does not answer, and a message that is not
/// JSON-RPC, with the protocol's own errors (JSON-RPC 2.0).
#[test]
fn the_wire_answers_what_json_rpc_asks_and_nothing_more() {
    let (mut store, mut world) = (FakeStore::default(), world::Files::new());
    let initialized = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
    assert!(send(&mut store, &mut world, initialized).reply.is_none());
    let a_response = json!({"jsonrpc": "2.0", "id": 9, "result": {}});
    assert!(send(&mut store, &mut world, a_response).reply.is_none());

    let pong = send(&mut store, &mut world, request(3, "ping", json!({}))).reply.expect("answered");
    assert_eq!(pong["result"], json!({}));

    let unknown = send(&mut store, &mut world, request(4, "sampling/createMessage", json!({})));
    assert_eq!(unknown.reply.expect("answered")["error"]["code"], json!(protocol::METHOD_NOT_FOUND));

    let not_rpc = send(&mut store, &mut world, json!({"method": "ping", "id": 5}));
    assert_eq!(not_rpc.reply.expect("answered")["error"]["code"], json!(protocol::INVALID_REQUEST));

    // A batch is answered one reply per request, notifications left out.
    let batch = json!([request(6, "ping", json!({})), initialized_again(), request(7, "ping", json!({}))]);
    let replies = send(&mut store, &mut world, batch).reply.expect("answered");
    let ids: Vec<Value> = replies.as_array().expect("a batch reply").iter().map(|r| r["id"].clone()).collect();
    assert_eq!(ids, vec![json!(6), json!(7)]);

    // None of it wrote anything.
    assert!(store.list_records(&Scope::All).unwrap().is_empty());
}

fn initialized_again() -> Value {
    json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
}

/// The tool listing is the catalogue and its read-only tools, in their order,
/// each tool's arguments the properties of its input schema and its doc its
/// description (193).
#[test]
fn the_tool_listing_declares_the_catalogue() {
    let (mut store, mut world) = (FakeStore::default(), world::Files::new());
    let reply = send(&mut store, &mut world, request(1, "tools/list", json!({}))).reply.expect("answered");
    let listed = reply["result"]["tools"].as_array().expect("a list of tools").clone();
    let catalogue: Vec<&crate::catalogue::Tool> =
        crate::catalogue::catalogue().iter().chain(crate::catalogue::queries()).collect();
    assert_eq!(listed.len(), catalogue.len());
    for (declared, tool) in listed.iter().zip(catalogue) {
        assert_eq!(declared["name"], json!(tool.name));
        assert_eq!(declared["description"], json!(tool.doc));
        assert_eq!(declared["inputSchema"]["type"], json!("object"));
        let mut properties: Vec<&str> = declared["inputSchema"]["properties"]
            .as_object()
            .expect("properties")
            .keys()
            .map(String::as_str)
            .collect();
        let mut args = tool.args.to_vec();
        properties.sort_unstable();
        args.sort_unstable();
        assert_eq!(properties, args, "`{}` declares other arguments", tool.name);
    }
}

/// A call is the catalogue's call under the caller's identity, recorded once
/// with the protocol as where it came from; the same delivery named twice
/// writes nothing the second time (153, 193, 323, 137).
#[test]
fn a_call_is_the_catalogues_and_a_repeat_delivery_writes_nothing() {
    let (mut store, mut world) = (FakeStore::default(), world::Files::new());
    let drop = request(
        1,
        "tools/call",
        json!({"name": "drop", "arguments": {"object": "unit/atlas/u"}, "_meta": {"flywheel/delivery": "tap-7f3a"}}),
    );
    let handled = send(&mut store, &mut world, drop.clone());
    assert!(handled.wrote);
    let result = &handled.reply.expect("answered")["result"];
    assert_eq!(result["isError"], json!(false), "{result}");
    assert_eq!(result["structuredContent"]["id"], json!("client-tap-7f3a"));
    assert_eq!(result["structuredContent"]["recorded"], json!(true));
    let record = store.get("response/client-tap-7f3a").unwrap().expect("the call was recorded");
    assert_eq!(record.record.get("tool"), Some(&json!("drop")));
    assert_eq!(record.record.get("object"), Some(&json!("unit/atlas/u")));
    assert_eq!(record.record.get("given_by"), Some(&json!("chuck")));
    assert_eq!(record.record.get("delivery"), Some(&json!("client")));

    let again = send(&mut store, &mut world, drop);
    assert!(!again.wrote, "a repeat delivery is no local cause");
    assert_eq!(again.reply.expect("answered")["result"]["structuredContent"]["recorded"], json!(false));
    let responses = store.list_records(&Scope::Machine("response".into())).unwrap();
    assert_eq!(responses.len(), 1, "the repeat wrote a second response: {responses:?}");

    // A call naming no delivery is counted like a page submission is.
    let unnamed = request(2, "tools/call", json!({"name": "hold", "arguments": {"object": "unit/atlas/u"}}));
    let reply = send(&mut store, &mut world, unnamed).reply.expect("answered");
    assert_eq!(reply["result"]["structuredContent"]["id"], json!("client-1"));
}

/// A call the catalogue refuses is the tool's error with the reason, and writes
/// nothing; a delivery named in a shape the store cannot key is refused by the
/// wire before any call (193, 137).
#[test]
fn a_refused_call_says_why_and_writes_nothing() {
    let (mut store, mut world) = (FakeStore::default(), world::Files::new());
    let no_number = request(1, "tools/call", json!({"name": "answer", "arguments": {"answer": "yes"}}));
    let handled = send(&mut store, &mut world, no_number);
    assert!(!handled.wrote);
    let result = &handled.reply.expect("answered")["result"];
    assert_eq!(result["isError"], json!(true));
    assert!(
        result["content"][0]["text"].as_str().is_some_and(|t| t.contains("decision")),
        "the refusal says what was missing: {result}"
    );

    for shape in [json!("../escape"), json!("412"), json!(7)] {
        let named = request(2, "tools/call", json!({"name": "drop", "arguments": {"object": "unit/a/b"}, "_meta": {"flywheel/delivery": shape}}));
        let reply = send(&mut store, &mut world, named).reply.expect("answered");
        assert_eq!(reply["error"]["code"], json!(protocol::INVALID_PARAMS), "{reply}");
    }
    assert!(store.list_records(&Scope::All).unwrap().is_empty());
}

/// A delivery's name keys its response: letters, digits and `.` `_` `-`, and
/// never a bare number, which is what an unnamed delivery is counted by (137).
#[test]
fn delivery_id_keeps_a_named_delivery_apart_from_a_counted_one() {
    assert_eq!(protocol::delivery_id("tap-7f3a").as_deref(), Some("client-tap-7f3a"));
    assert_eq!(protocol::delivery_id("a.b_c-9").as_deref(), Some("client-a.b_c-9"));
    for refused in ["", "12", "a/b", "a b", &"x".repeat(65)] {
        assert!(protocol::delivery_id(refused).is_none(), "`{refused}` was taken");
    }
}
