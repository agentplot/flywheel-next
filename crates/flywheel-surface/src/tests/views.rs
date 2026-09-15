//! The page's own views as a member's client gets them: named on the tool's
//! declaration, read as the page's bundle, versioned with the binary, and a copy
//! of another version drawing none of the state (293a, 322, 326, S230, D18).

use crate::page::{self, VERSION, VIEWS};
use crate::protocol::{self, Caller};
use crate::testing as world;
use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::{Records, Scope};
use flywheel_domain::commands;
use flywheel_engine::Definitions;
use serde_json::{json, Value};

const ADDRESS: &str = "http://studio.tailnet.ts.net/willdan";
const BOLT: &str = "bolt/atlas/plan-rows";

fn defs() -> Definitions {
    flywheel_domain::set::load().expect("the embedded definitions")
}

/// A bolt whose close is offered: one decision, standing (22, 39).
fn a_decision(store: &mut FakeStore, defs: &Definitions) {
    let at = commands::now(store).expect("a point");
    commands::put_new(
        store,
        defs,
        BOLT,
        "bolt",
        None,
        [("repository".to_string(), json!("atlas"))].into_iter().collect(),
        at,
    )
    .expect("the bolt");
    let mut bolt = Records::get(store, BOLT).expect("a read").expect("the bolt");
    bolt.config.insert("life".into(), "open".into());
    bolt.config.insert("life.open.close".into(), "offered".into());
    let base = bolt.seq;
    Records::put(store, BOLT, &bolt, base).expect("the bolt with its close offered");
    commands::rail(store, defs).expect("the rail derives");
}

/// A store with the decision standing, and the world beside it.
fn a_rail() -> (FakeStore, world::Files, Definitions) {
    let defs = defs();
    let mut store = FakeStore::default();
    a_decision(&mut store, &defs);
    (store, world::Files::new(), defs)
}

/// One request's reply, answered for the operator `chuck`.
fn ask(store: &mut FakeStore, world: &mut world::Files, defs: &Definitions, method: &str, params: Value) -> Value {
    let mut caller = Caller { store, world, defs, address: ADDRESS, by: "chuck" };
    let message = json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params});
    protocol::handle(&mut caller, &message).reply.expect("a request is answered")
}

/// The one `<tag>…</tag>` block a document carries, whole.
fn block<'a>(html: &'a str, tag: &str) -> &'a str {
    let open = html.find(&format!("<{tag}>")).unwrap_or_else(|| panic!("no <{tag}>"));
    let close = html[open..].find(&format!("</{tag}>")).expect("closed") + open;
    &html[open..close]
}

/// Every key named `name` anywhere in a value.
fn holds_key(value: &Value, name: &str) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(key, inner)| key == name || holds_key(inner, name)),
        Value::Array(items) => items.iter().any(|item| holds_key(item, name)),
        _ => false,
    }
}

/// Each read-only tool names its view's resource on its declaration, at the
/// view's address under the binary's version; no operation names one; and the
/// rail's result names no resource, carries the version it was rendered under,
/// and records nothing (293a, 322, 326, S230).
#[test]
fn a_view_tool_names_its_resource_on_its_declaration() {
    let (mut store, mut world, defs) = a_rail();
    let listed = ask(&mut store, &mut world, &defs, "tools/list", json!({}));
    let tools = listed["result"]["tools"].as_array().expect("a list of tools");
    for view in VIEWS {
        let declared = tools
            .iter()
            .find(|tool| tool["name"] == json!(view))
            .unwrap_or_else(|| panic!("no tool's result is the {view}"));
        assert_eq!(
            declared["_meta"]["ui"]["resourceUri"],
            json!(format!("ui://flywheel/{VERSION}/{view}")),
            "{declared}"
        );
        assert_eq!(declared["annotations"]["readOnlyHint"], json!(true), "{declared}");
    }
    for operation in crate::catalogue::catalogue() {
        let declared = tools.iter().find(|tool| tool["name"] == json!(operation.name)).expect("declared");
        assert!(declared.get("_meta").is_none(), "`{}` names a view: {declared}", operation.name);
    }

    let before = store.list_records(&Scope::All).unwrap().len();
    let reply = ask(&mut store, &mut world, &defs, "tools/call", json!({"name": "rail"}));
    let result = &reply["result"];
    assert_eq!(result["isError"], json!(false), "{result}");
    assert!(
        !holds_key(result, "resourceUri") && !result.to_string().contains("ui://"),
        "the result names a resource: {result}"
    );
    assert_eq!(result["structuredContent"]["version"], json!(VERSION));
    assert_eq!(result["structuredContent"]["view"], json!("rail"));
    let rail = result["structuredContent"]["regions"]["rail"].as_str().expect("the rail's region");
    assert!(rail.contains(&format!("data-object=\"{BOLT}\"")), "the region is the rail's cards: {rail}");
    let words = result["content"][0]["text"].as_str().expect("the rail in words");
    assert!(words.contains(BOLT) && words.contains("answers: "), "{words}");
    assert_eq!(store.list_records(&Scope::All).unwrap().len(), before, "a view wrote something");
}

/// All four addresses read the page's own bundle with the extension's media
/// type: the same stylesheet and script the served page carries, every region
/// empty, nothing of the state, no origin declared and no permission asked (293a,
/// 307, 310, 204).
#[test]
fn ui_resource_is_the_pages_own_bundle() {
    let (mut store, mut world, defs) = a_rail();
    let listed = ask(&mut store, &mut world, &defs, "resources/list", json!({}));
    let resources = listed["result"]["resources"].as_array().expect("a list of resources");
    assert_eq!(resources.len(), VIEWS.len(), "{listed}");

    let read = page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("the page reads");
    let served = page::render(&read);
    let bundle = page::bundle();
    for resource in resources {
        let uri = resource["uri"].as_str().expect("an address");
        assert_eq!(resource["mimeType"], json!(protocol::VIEW_MEDIA_TYPE));
        let answered = ask(&mut store, &mut world, &defs, "resources/read", json!({"uri": uri}));
        let contents = answered["result"]["contents"].as_array().expect("contents");
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["uri"], json!(uri));
        assert_eq!(contents[0]["mimeType"], json!("text/html;profile=mcp-app"));
        assert_eq!(contents[0]["text"], json!(bundle), "`{uri}` is not the page's bundle");
        for asked in ["csp", "permissions", "connectDomains", "resourceDomains"] {
            assert!(!holds_key(&answered, asked) && !holds_key(resource, asked), "`{uri}` declares `{asked}`");
        }
    }

    // The page the host serves is this bundle with the state drawn into it.
    assert_eq!(block(&bundle, "style"), block(&served, "style"), "a second stylesheet");
    assert_eq!(block(&bundle, "script"), block(&served, "script"), "a second script");
    assert!(served.contains(BOLT), "the served page draws the state");
    assert!(!bundle.contains(BOLT), "the bundle carries state");
    assert!(bundle.contains("<aside class=\"rail\" id=\"rail\" aria-label=\"Decisions\"></aside>"));
    assert!(bundle.contains("data-served=\"view\"") && served.contains("data-served=\"page\""));
    for reaching_out in ["src=\"http", "@import", "fetch(\"http", "fetch('http", "XMLHttpRequest", "WebSocket(", "localStorage"] {
        assert!(!bundle.contains(reaching_out), "the bundle reaches out: {reaching_out}");
    }
}

/// The addresses and the bundle carry the binary's version; an address of
/// another version is not this binary's and says which it serves; and every
/// result carries the version it was answered under (307, 326, S230).
#[test]
fn ui_resource_version_is_the_binarys() {
    assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
    let (mut store, mut world, defs) = a_rail();
    let listed = ask(&mut store, &mut world, &defs, "resources/list", json!({}));
    let addresses: Vec<&str> = listed["result"]["resources"]
        .as_array()
        .expect("resources")
        .iter()
        .filter_map(|r| r["uri"].as_str())
        .collect();
    let expected: Vec<String> = VIEWS.iter().map(|v| format!("ui://flywheel/{}/{v}", env!("CARGO_PKG_VERSION"))).collect();
    assert_eq!(addresses, expected);
    assert!(page::bundle().contains(&format!("<meta name=\"flywheel-version\" content=\"{}\">", env!("CARGO_PKG_VERSION"))));

    let older = ask(&mut store, &mut world, &defs, "resources/read", json!({"uri": "ui://flywheel/0.0.1/rail"}));
    assert_eq!(older["error"]["code"], json!(protocol::RESOURCE_NOT_FOUND), "{older}");
    let said = older["error"]["message"].as_str().unwrap_or_default();
    assert!(said.contains(VERSION) && said.contains(&expected[0]), "{said}");

    for (name, arguments) in [
        ("rail", json!({})),
        ("object", json!({"object": BOLT})),
        ("object", json!({"object": "bolt/atlas/none"})),
        ("hold", json!({"object": BOLT})),
        ("answer", json!({})),
    ] {
        let reply = ask(&mut store, &mut world, &defs, "tools/call", json!({"name": name, "arguments": arguments}));
        assert_eq!(
            reply["result"]["structuredContent"]["version"],
            json!(VERSION),
            "`{name}` answered with no version: {reply}"
        );
    }
}

/// A copy of the bundle an earlier binary served, handed a result this binary
/// returned, draws none of the state and says it is out of date (326, S230).
///
/// The bundle draws nothing of its own: every region is empty until it is
/// handed a result, and its script draws a result's regions only where the
/// result's version is the one its own `flywheel-version` names, and otherwise
/// empties every region a result could fill and shows the notice. That a
/// browser does so with a real client's messages is the system tier's
/// `a_view_of_another_version_shows_it_is_out_of_date_in_a_browser`.
#[test]
fn a_view_of_another_version_shows_it_is_out_of_date() {
    let (mut store, mut world, defs) = a_rail();
    // A newer binary's result...
    let newer = ask(&mut store, &mut world, &defs, "tools/call", json!({"name": "rail"}));
    let handed = &newer["result"]["structuredContent"];
    assert_eq!(handed["version"], json!(VERSION));
    // ...and the copy of the bundle a client kept from an earlier one.
    let earlier = page::bundle().replace(
        &format!("<meta name=\"flywheel-version\" content=\"{VERSION}\">"),
        "<meta name=\"flywheel-version\" content=\"0.0.1\">",
    );
    assert!(earlier.contains("content=\"0.0.1\""), "the version change was not driven");
    assert_ne!(handed["version"], json!("0.0.1"));

    // What the copy shows when it will not draw the result, hidden until then.
    assert!(earlier.contains(
        "<p class=\"view-stale\" id=\"view-stale\" hidden>this view is out of date · fetch it again</p>"
    ));
    // Nothing of the state is in the copy, and every region is empty.
    assert!(!earlier.contains(BOLT));
    // The rule it draws by: its own version against the result's, and on a
    // difference every region any view's result fills is emptied.
    let script = block(&earlier, "script");
    assert!(script.contains("meta[name=\"flywheel-version\"]"), "the script reads no version of its own");
    assert!(script.contains("handed.version!==VERSION"), "the script draws whatever it is handed");
    for view in VIEWS {
        let arguments = match view {
            "object" => json!({"object": BOLT}),
            _ => json!({}),
        };
        let reply = ask(&mut store, &mut world, &defs, "tools/call", json!({"name": view, "arguments": arguments}));
        let regions = reply["result"]["structuredContent"]["regions"].as_object().expect("regions");
        for id in regions.keys() {
            assert!(
                script.contains(&format!("'{id}'")),
                "the {view} view fills `{id}`, which an out-of-date copy would leave drawn"
            );
            assert!(
                earlier.contains(&format!("id=\"{id}\"")),
                "the {view} view fills `{id}`, which the bundle has no place for"
            );
        }
    }
}

/// The views offered are the page's own and no others — the rail, the board,
/// the status view and one object's detail — each listed among the resources
/// before any tool is called, each named by the read-only tool that answers
/// it, and every region each view draws is a region the served page has (322,
/// 307, S230).
#[test]
fn the_offered_views_are_the_pages_own() {
    let (mut store, mut world, defs) = a_rail();
    // Before any tool has been called.
    let listed = ask(&mut store, &mut world, &defs, "resources/list", json!({}));
    let names: Vec<&str> = listed["result"]["resources"]
        .as_array()
        .expect("resources")
        .iter()
        .filter_map(|r| r["name"].as_str())
        .collect();
    assert_eq!(names, ["rail", "board", "status", "object"], "the resources are not the page's four views");

    let tools = ask(&mut store, &mut world, &defs, "tools/list", json!({}));
    let named: Vec<String> = tools["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .filter_map(|t| t["_meta"]["ui"]["resourceUri"].as_str().map(String::from))
        .collect();
    let offered: Vec<String> = VIEWS.iter().map(|v| crate::catalogue::view_address(v)).collect();
    assert_eq!(named, offered, "a tool names a view the resources do not list, or one goes unnamed");

    // Each view draws regions the page has, by the ids the page gives them.
    let read = page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("the page reads");
    let served = page::render(&read);
    let mut drawn: Vec<String> = Vec::new();
    for view in VIEWS {
        let object = (view == "object").then_some(BOLT);
        let shown = page::view(&read, view, object).unwrap_or_else(|e| panic!("the {view}: {e}"));
        assert!(!shown.regions.is_empty(), "the {view} draws nothing");
        for id in shown.regions.keys() {
            assert!(served.contains(&format!("id=\"{id}\"")), "the {view} draws `{id}`, which the page has no region for");
            drawn.push(id.clone());
        }
    }
    // And between them they draw the page's rail, board, status view and dock.
    for region in ["rail", "board-h", "lane-inception", "lane-plan", "lane-construction", "lane-operation", "hosts", "dk-b"] {
        assert!(drawn.iter().any(|id| id == region), "no view draws the page's `{region}`");
    }
}

/// A caller that renders no view still holds every tool, and every decision on
/// the rail is answerable by a call it holds: the rail in words names each
/// decision's number and its answers, and `answer` takes them (322, 311).
#[test]
fn every_decision_is_answerable_without_a_view() {
    let defs = defs();
    let mut store = FakeStore::default();
    a_decision(&mut store, &defs);
    let mut world = world::Files::new();
    // A client with no user-interface extension says hello like any other.
    let hello = ask(&mut store, &mut world, &defs, "initialize", json!({"protocolVersion": "2025-06-18", "capabilities": {}}));
    assert!(hello["result"]["capabilities"]["tools"].is_object());
    let tools = ask(&mut store, &mut world, &defs, "tools/list", json!({}));
    let answer = tools["result"]["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .find(|t| t["name"] == json!("answer"))
        .expect("the answer tool is held without a view")
        .clone();
    for takes in ["decision", "answer"] {
        assert!(answer["inputSchema"]["properties"].get(takes).is_some(), "`answer` takes no `{takes}`");
    }

    let read = page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("the page reads");
    assert!(!read.decisions.is_empty(), "no decision stands to answer");
    let reply = ask(&mut store, &mut world, &defs, "tools/call", json!({"name": "rail"}));
    let words = reply["result"]["content"][0]["text"].as_str().expect("the rail in words").to_string();
    for decision in &read.decisions {
        let number = decision.number.expect("numbered");
        let line = words
            .lines()
            .position(|line| line.trim_start().starts_with(&format!("{number} · ")))
            .unwrap_or_else(|| panic!("decision {number} is not in the words: {words}"));
        let answers = words
            .lines()
            .nth(line + 1)
            .and_then(|next| next.trim().strip_prefix("answers: "))
            .unwrap_or_else(|| panic!("decision {number} says no answers: {words}"));
        let said: Vec<&str> = answers.split(" | ").collect();
        assert_eq!(said, decision.answers, "the words give decision {number} other answers");
        // And a call answers it, with the first answer the words give.
        let call = json!({"name": "answer", "arguments": {"decision": number, "answer": said[0]}});
        let answered = ask(&mut store, &mut world, &defs, "tools/call", call);
        assert_eq!(answered["result"]["isError"], json!(false), "decision {number} was not answerable: {answered}");
        assert_eq!(answered["result"]["structuredContent"]["recorded"], json!(true));
    }
}

/// A view the page does not have, and an object the instance does not hold,
/// are refused in words with nothing drawn (322).
#[test]
fn a_view_the_page_lacks_is_refused() {
    let (mut store, world, defs) = a_rail();
    let read = page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("the page reads");
    assert!(page::view(&read, "map", None).is_err());
    assert!(page::view(&read, "object", None).is_err());
    assert!(page::view(&read, "object", Some("bolt/atlas/none")).is_err());
    let board = page::view(&read, "board", None).expect("the board");
    assert!(board.regions.contains_key("lane-construction") && board.said.contains("Construction"));
    let status = page::view(&read, "status", None).expect("the status view");
    assert!(status.regions.contains_key("hosts") && status.said.contains("as of commit"));
    let object = page::view(&read, "object", Some(BOLT)).expect("the object");
    assert!(object.regions["dk-b"].contains("data-opened=\"true\""));
    assert!(object.said.contains("answers:"), "{}", object.said);
}
