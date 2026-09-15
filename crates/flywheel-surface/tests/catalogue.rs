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
    // The protocol has one list of tools, and it is the operations and then
    // the read-only tools, as the other two callers enumerate them (193, 322).
    let enumerated: Vec<Value> = in_process["tools"]
        .as_array()
        .expect("tools is a list")
        .iter()
        .chain(in_process["queries"].as_array().expect("queries is a list"))
        .cloned()
        .collect();
    let enumerated = &enumerated;
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

const BOLT: &str = "bolt/atlas/plan-rows";

/// A bolt whose close is offered, standing on the rail, and the number the
/// register gave it (22, 39, 15).
fn a_standing_close(store: &mut flywheel_store_git::GitStore, defs: &flywheel_engine::Definitions) -> u64 {
    use flywheel_atoms::Records;
    use flywheel_domain::commands;
    let at = commands::now(store).expect("a point");
    let record = [("repository".to_string(), json!("atlas"))].into_iter().collect();
    commands::put_new(store, defs, BOLT, "bolt", None, record, at).expect("the bolt");
    let mut bolt = Records::get(store, BOLT).expect("a read").expect("the bolt");
    bolt.config.insert("life".into(), "open".into());
    bolt.config.insert("life.open.close".into(), "offered".into());
    let base = bolt.seq;
    Records::put(store, BOLT, &bolt, base).expect("its close offered");
    let rail = commands::rail(store, defs).expect("the rail derives");
    rail.iter().find(|d| d.object == BOLT).and_then(|d| d.number).expect("numbered") as u64
}

/// The text between two markers, after the first.
fn between(text: &str, open: &str, close: &str) -> String {
    let at = text.find(open).map(|at| at + open.len()).unwrap_or_else(|| panic!("no `{open}` in {text}"));
    text[at..].split(close).next().unwrap_or_default().to_string()
}

/// The control a rail card carries for one answer, read off the view's markup
/// the way the bundle reads it: the tool its form posts to, and its fields
/// (193, 311).
fn control_on(rail: &str, number: u64, answer: &str) -> (String, serde_json::Map<String, Value>) {
    let card = rail
        .split("<article class=\"card decision")
        .find(|card| card.contains(&format!("data-number=\"{number}\"")))
        .unwrap_or_else(|| panic!("no card {number} on {rail}"));
    let form = card
        .split("<form ")
        .find(|form| form.contains(&format!("data-answer=\"{answer}\"")))
        .unwrap_or_else(|| panic!("card {number} carries no `{answer}` control: {card}"));
    let tool = between(form, "action=\"/api/tools/", "\"");
    let mut fields = serde_json::Map::new();
    for input in form.split("<input ").skip(1) {
        let input = input.split('>').next().unwrap_or_default();
        if input.contains("type=\"hidden\"") {
            let (name, value) = (between(input, "name=\"", "\""), between(input, "value=\"", "\""));
            // The bundle sends a decision as the number it is.
            let value = match (name.as_str(), value.parse::<u64>()) {
                ("decision", Ok(number)) => json!(number),
                _ => json!(value),
            };
            fields.insert(name, value);
        }
    }
    (tool, fields)
}

fn responses(page: &caller::Server) -> Vec<flywheel_engine::Object> {
    page.with_store(|store| {
        flywheel_atoms::Records::list_records(store, &flywheel_atoms::Scope::Machine("response".into()))
            .expect("a read")
    })
}

/// A tap on a rendered view's control is the call its form names, sent through
/// the client: recorded once with the tool, the decision, who gave it and when,
/// taking effect once however often the client delivers it, and the view drawn
/// again shows the answer given (321, 323, 137, 153, S230).
#[test]
fn a_tap_in_a_rendered_view_is_a_recorded_call() {
    let sandbox = store::Sandbox::new("tap");
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let mut store = sandbox.store();
    let number = a_standing_close(&mut store, &defs);
    let page = caller::Server::start(store, defs, "chuck");

    // The rail as the client hands it to the view, and the yes on its card.
    let (_, drawn) = page.protocol(json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "rail"}}));
    let drawn = drawn.expect("the rail");
    let rail = drawn["result"]["structuredContent"]["regions"]["rail"].as_str().expect("the rail's region");
    let (tool, arguments) = control_on(rail, number, "yes");
    assert_eq!(tool, "answer", "the card's yes posts to the answer tool");
    assert_eq!(arguments.get("decision"), Some(&json!(number)));

    // The call the bundle sends through the client for that tap.
    let tap = json!({
        "jsonrpc": "2.0", "id": "flywheel-2", "method": "tools/call",
        "params": {"name": tool, "arguments": arguments, "_meta": {"flywheel/delivery": "tap-5e1f0a2b3c4d5e6f"}}
    });
    let (status, answered) = page.protocol(tap.clone());
    assert_eq!(status, 200);
    let answered = answered.expect("answered");
    assert_eq!(answered["result"]["isError"], json!(false), "{answered}");
    assert_eq!(answered["result"]["structuredContent"]["recorded"], json!(true), "{answered}");
    // The client delivers it again; it takes effect once (137).
    let (_, again) = page.protocol(tap);
    assert_eq!(again.expect("answered")["result"]["structuredContent"]["recorded"], json!(false));

    let recorded = responses(&page);
    assert_eq!(recorded.len(), 1, "{recorded:?}");
    let record = &recorded[0];
    assert_eq!(record.id, "response/client-tap-5e1f0a2b3c4d5e6f");
    assert_eq!(record.record.get("tool"), Some(&json!("answer")));
    assert_eq!(record.record.get("decision"), Some(&json!(number)));
    assert_eq!(record.record.get("answer"), Some(&json!("yes")));
    assert_eq!(record.record.get("given_by"), Some(&json!("chuck")));
    assert_eq!(record.record.get("delivery"), Some(&json!("client")));
    assert!(record.record.contains_key("given_at"));

    // The view the bundle draws again after the tap shows the answer given.
    let (_, redrawn) = page.protocol(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "rail"}}));
    let redrawn = redrawn.expect("the rail again");
    let rail = redrawn["result"]["structuredContent"]["regions"]["rail"].as_str().expect("the region");
    assert!(rail.contains("yes · chuck"), "the answer given is not on the view: {rail}");
}

/// The same tool on the same object, called from a view through a client and
/// posted from the page's form, writes the same record but for where it came
/// from (193, 321).
#[test]
fn a_client_call_and_a_page_call_record_alike() {
    let on_page = store::Sandbox::new("alike-page");
    let page = caller::Server::start(on_page.store(), flywheel_domain::set::load().expect("defs"), "chuck");
    let posted = page.form("/api/tools/drop", &[("object", "unit/atlas/u")]);
    assert!(posted.recorded(), "the page's drop: {:?}", posted.refused());

    let in_client = store::Sandbox::new("alike-client");
    let client = caller::Server::start(in_client.store(), flywheel_domain::set::load().expect("defs"), "chuck");
    let (_, called) = client.protocol(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "drop", "arguments": {"object": "unit/atlas/u"}}
    }));
    assert_eq!(called.expect("answered")["result"]["isError"], json!(false));

    let (by_page, by_client) = (responses(&page), responses(&client));
    assert_eq!((by_page.len(), by_client.len()), (1, 1));
    let (by_page, by_client) = (&by_page[0], &by_client[0]);
    for field in ["tool", "object", "answer", "given_by", "given_at", "args"] {
        assert_eq!(
            by_page.record.get(field),
            by_client.record.get(field),
            "`{field}` differs between the page's call and the client's"
        );
    }
    assert_eq!(by_page.record.get("delivery"), Some(&json!("page")));
    assert_eq!(by_client.record.get("delivery"), Some(&json!("client")));
    assert_eq!(by_page.machine, by_client.machine);
    assert_eq!(by_page.config, by_client.config);
}

/// The value a run record entry carries for a field.
fn field<'a>(entry: &'a flywheel_domain::records::RunEntry, name: &str) -> Option<&'a str> {
    entry.fields.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
}

/// A call from a client the flywheel refuses writes no response, and its
/// refusal is in the host's run record with the identity, the tool and the
/// object — refused by the catalogue, or refused at the door (321, 79, 253a).
#[test]
fn a_refused_client_call_reaches_the_run_record() {
    let sandbox = store::Sandbox::new("refused-client");
    let page = caller::Server::start(sandbox.store(), flywheel_domain::set::load().expect("defs"), "chuck");
    let (status, reply) = page.protocol(json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "ask", "arguments": {"repository": "nowhere", "text": "keep the row numbers"}}
    }));
    assert_eq!(status, 200);
    assert_eq!(reply.expect("answered")["result"]["isError"], json!(true));
    assert!(responses(&page).is_empty(), "a refused call wrote a response");
    let run = page.with_store(|git| git.run_record().expect("the run record"));
    let refusal = run.iter().find(|e| e.kind == "refusal").unwrap_or_else(|| panic!("no refusal in {run:?}"));
    assert_eq!(refusal.object, "nowhere");
    assert_eq!(field(refusal, "identity"), Some("chuck"));
    assert_eq!(field(refusal, "operation"), Some("ask"));
    assert_eq!(field(refusal, "delivery"), Some("client"));
    assert!(refusal.reason.contains("repository"), "{}", refusal.reason);

    // At the door: a second operator is listed, so nothing is served unsigned-in.
    let door = store::Sandbox::new("refused-door");
    let two = caller::Server::for_operators(
        door.store(),
        flywheel_domain::set::load().expect("defs"),
        &["chuck".to_string(), "lee".to_string()],
        "http://host.example/instance",
    );
    let (status, reply) = two.protocol(json!({
        "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": {"name": "drop", "arguments": {"object": "unit/atlas/u"}}
    }));
    assert_eq!(status, 403);
    assert_eq!(reply.expect("refused")["error"]["code"], json!(flywheel_surface::protocol::REFUSED));
    assert!(responses(&two).is_empty());
    let run = two.with_store(|git| git.run_record().expect("the run record"));
    let refusal = run.iter().find(|e| e.kind == "refusal").unwrap_or_else(|| panic!("no refusal in {run:?}"));
    assert_eq!(refusal.object, "unit/atlas/u");
    assert_eq!(field(refusal, "identity"), Some("unsigned-in"));
    assert_eq!(field(refusal, "operation"), Some("drop"));
    assert!(refusal.reason.contains("operators list"), "{}", refusal.reason);
}

/// A control on the page the flywheel refuses writes no response and answers
/// the operator on the page as it always has, and its refusal is in the host's
/// run record beside a client's, naming the page's delivery — refused by the
/// catalogue, or refused at the door (321, 79, 253a).
#[test]
fn a_refused_page_call_reaches_the_run_record() {
    let sandbox = store::Sandbox::new("refused-page");
    let page = caller::Server::start(sandbox.store(), flywheel_domain::set::load().expect("defs"), "chuck");
    let posted = page.form("/api/tools/ask", &[("repository", "nowhere"), ("text", "keep the row numbers")]);
    let shown = posted.refused().unwrap_or_else(|| panic!("the page did not refuse: {}", posted.body));
    assert!(responses(&page).is_empty(), "a refused control wrote a response");
    let run = page.with_store(|git| git.run_record().expect("the run record"));
    let refusals: Vec<_> = run.iter().filter(|e| e.kind == "refusal").collect();
    assert_eq!(refusals.len(), 1, "one refused control is one entry: {run:?}");
    let refusal = refusals[0];
    assert_eq!(refusal.object, "nowhere");
    assert_eq!(field(refusal, "identity"), Some("chuck"));
    assert_eq!(field(refusal, "operation"), Some("ask"));
    assert_eq!(field(refusal, "delivery"), Some("page"));
    // What the operator reads on the page is what the record keeps.
    assert_eq!(shown, refusal.reason);

    // At the door: a second operator is listed, so nothing is served unsigned-in.
    let door = store::Sandbox::new("refused-page-door");
    let two = caller::Server::for_operators(
        door.store(),
        flywheel_domain::set::load().expect("defs"),
        &["chuck".to_string(), "lee".to_string()],
        "http://host.example/instance",
    );
    let posted = two.form("/api/tools/drop", &[("object", "unit/atlas/u")]);
    assert_eq!(posted.status, 403);
    assert!(responses(&two).is_empty());
    let run = two.with_store(|git| git.run_record().expect("the run record"));
    let refusal = run.iter().find(|e| e.kind == "refusal").unwrap_or_else(|| panic!("no refusal in {run:?}"));
    assert_eq!(refusal.object, "unit/atlas/u");
    assert_eq!(field(refusal, "identity"), Some("unsigned-in"));
    assert_eq!(field(refusal, "operation"), Some("drop"));
    assert_eq!(field(refusal, "delivery"), Some("page"));
    assert!(posted.refused().is_some_and(|shown| shown.contains(&refusal.reason)), "{}", posted.body);
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
