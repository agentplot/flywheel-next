//! A view in a member's client: the page's bundle in a frame, handed a tool's
//! result the way a client hands it, and asking for what it opens through the
//! client (293a, 322, 326, S230, D18).
//!
//! The harness is the client's half of the user-interface extension and no
//! more: it frames the bundle it read from the served protocol, answers the
//! view's hello, hands it a result, and holds each call the view sends until
//! this test carries it to the served protocol and brings the reply back. What
//! is asserted is what the frame drew, read out of the browser (310).

use super::{driver, phone};

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};

/// The instance the driver's page is served as (`driver::Served::page`).
const INSTANCE: &str = "willdan";

/// One message of the protocol, posted at the instance's address the way a
/// client posts it (319).
fn protocol(address: SocketAddr, message: Value) -> Value {
    let payload = message.to_string();
    let mut socket = TcpStream::connect(address).expect("the served address");
    let head = format!(
        "POST /{INSTANCE} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\
         Content-Type: application/json\r\nAccept: application/json, text/event-stream\r\n\
         Content-Length: {}\r\n\r\n",
        payload.len()
    );
    socket.write_all(head.as_bytes()).expect("the request");
    socket.write_all(payload.as_bytes()).expect("the body");
    let mut answer = String::new();
    socket.read_to_string(&mut answer).expect("the reply");
    let body = answer.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or_default();
    serde_json::from_str(body).unwrap_or_else(|e| panic!("the reply {answer:?}: {e}"))
}

fn request(method: &str, params: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
}

/// The client's half: a frame holding the bundle, the view's hello answered,
/// the result handed over once the view says it is ready, and every call the
/// view makes held in `window.calls` for the test to carry.
fn harness(bundle: &str, result: &Value) -> String {
    // A script's text ends at the first `</`, so what is embedded says `<\/`.
    let embed = |value: &Value| value.to_string().replace("</", "<\\/");
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><title>a client</title></head>
<body style="margin:0;background:#E9EBEF;font:14px system-ui,sans-serif">
<div style="max-width:780px;margin:28px auto;background:#fff;border-radius:14px;padding:14px;box-shadow:0 1px 4px rgba(0,0,0,.12)">
<p style="margin:2px 6px 12px;color:#444">show me the rail</p>
<iframe id="view" sandbox="allow-scripts allow-same-origin allow-forms" style="width:100%;height:240px;border:0;display:block"></iframe>
</div>
<script>
const BUNDLE = {bundle};
const RESULT = {result};
window.calls = [];
const frame = document.getElementById('view');
const send = (m) => frame.contentWindow.postMessage(m, '*');
window.reply = (id, result) => send({{jsonrpc: '2.0', id, result}});
addEventListener('message', (e) => {{
  if (e.source !== frame.contentWindow) return;
  const m = e.data; if (!m || m.jsonrpc !== '2.0') return;
  if (m.method === 'ui/initialize') window.reply(m.id, {{protocolVersion: '2026-01-26', hostInfo: {{name: 'a client', version: '1'}}, hostCapabilities: {{}}, hostContext: {{theme: 'light'}}}});
  else if (m.method === 'ui/notifications/initialized') send({{jsonrpc: '2.0', method: 'ui/notifications/tool-result', params: RESULT}});
  else if (m.method === 'ui/notifications/size-changed') frame.style.height = Math.max(160, m.params.height) + 'px';
  else if (m.id !== undefined) window.calls.push(m);
}});
frame.srcdoc = BUNDLE;
</script></body></html>"#,
        bundle = embed(&json!(bundle)),
        result = embed(result),
    )
}

/// The harness, served over loopback to every request for as long as the test
/// runs, so the frame and the page around it share an origin the test can read.
fn serve(html: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port for the harness");
    let url = format!("http://{}/", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for mut socket in listener.incoming().flatten() {
            let mut head = [0u8; 4096];
            let _ = socket.read(&mut head);
            let reply = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{html}",
                html.len()
            );
            let _ = socket.write_all(reply.as_bytes());
        }
    });
    url
}

/// An expression over the frame's document, `doc`, evaluated in the browser.
fn in_frame(tab: &headless_chrome::Tab, expression: &str) -> Value {
    let script = format!(
        "(() => {{ const doc = document.getElementById('view').contentDocument; \
          if (!doc || !doc.body) return null; return {expression}; }})()"
    );
    tab.evaluate(&script, false)
        .expect("the browser evaluates")
        .value
        .unwrap_or(Value::Null)
}

/// Wait until an expression over the frame's document is true.
fn until(tab: &headless_chrome::Tab, expression: &str) {
    let by = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < by {
        if in_frame(tab, expression) == json!(true) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!(
        "the frame never came to `{expression}`; it reads {}",
        in_frame(tab, "doc.body.innerText")
    );
}

/// The calls the view has sent through the client since the last look.
fn calls(tab: &headless_chrome::Tab) -> Vec<Value> {
    let held = tab
        .evaluate("JSON.stringify(window.calls.splice(0))", false)
        .expect("the browser evaluates")
        .value;
    serde_json::from_str(held.as_ref().and_then(Value::as_str).unwrap_or("[]")).expect("the calls")
}

/// A served page with a decision standing, and the address its protocol is at.
fn a_served_rail() -> (driver::Served, SocketAddr) {
    let (runtime, _) = phone::a_standing_decision();
    let served = driver::Served::page(runtime.store, runtime.defs, "chuck").expect("the page serves");
    let address: SocketAddr = served
        .url
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .parse()
        .expect("a loopback address");
    (served, address)
}

/// A client holding the bundle an earlier binary served renders a result this
/// binary returned: the frame says it is out of date and draws none of the
/// state (326, S230).
#[test]
fn a_view_of_another_version_shows_it_is_out_of_date_in_a_browser() {
    if !driver::available() {
        eprintln!("skipped: no browser to drive; the client's frame needs a Chromium or Chrome installed (D15)");
        return;
    }
    let (_served, address) = a_served_rail();
    let version = flywheel_surface::page::VERSION;
    let rail = protocol(address, request("tools/call", json!({"name": "rail"})))["result"].clone();
    assert_eq!(rail["structuredContent"]["version"], json!(version));
    let object = rail["structuredContent"]["regions"]["rail"].as_str().expect("the rail's region").to_string();
    let read = protocol(address, request("resources/read", json!({"uri": format!("ui://flywheel/{version}/rail")})));
    let bundle = read["result"]["contents"][0]["text"].as_str().expect("the bundle");
    let earlier = bundle.replace(
        &format!("<meta name=\"flywheel-version\" content=\"{version}\">"),
        "<meta name=\"flywheel-version\" content=\"0.0.1\">",
    );
    assert_ne!(earlier, bundle, "the version change was not driven");

    let browser = driver::Driver::open().expect("a browser");
    let tab = browser.visit(&serve(harness(&earlier, &rail)), driver::DESKTOP).expect("the client");
    until(tab, "doc.getElementById('view-stale') && doc.getElementById('view-stale').hidden === false");
    let said = in_frame(tab, "doc.body.innerText");
    assert!(
        said.as_str().is_some_and(|s| s.contains("this view is out of date · fetch it again")),
        "{said}"
    );
    assert_eq!(in_frame(tab, "doc.getElementById('rail').innerHTML"), json!(""), "state was drawn");
    assert_eq!(in_frame(tab, "doc.querySelectorAll('.card').length"), json!(0));
    assert!(object.contains("card decision"), "the result did carry the rail: {object}");
}

/// Carry one call the view sent to the served protocol and hand the reply back
/// to the view, as the client does.
fn carry(tab: &headless_chrome::Tab, address: SocketAddr, call: &Value) -> Value {
    let reply = protocol(address, json!({"jsonrpc": "2.0", "id": 9, "method": "tools/call", "params": call["params"]}));
    let script = format!("window.reply({}, {})", call["id"], reply["result"]);
    tab.evaluate(&script, false).expect("the reply is handed back");
    reply["result"].clone()
}

/// The next call the view sends through the client.
fn next_call(tab: &headless_chrome::Tab) -> Value {
    for _ in 0..200 {
        if let Some(call) = calls(tab).into_iter().next() {
            return call;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("the view sent no call through the client");
}

/// A tap on an answer inside the rendered rail goes busy at once, and is sent
/// through the client as the call its form names with a delivery of its own;
/// the view then asks for the rail again and shows the answer given (311, 321,
/// 323, 137, S230).
#[test]
fn a_tap_in_a_view_is_a_call_through_the_client_in_a_browser() {
    if !driver::available() {
        eprintln!("skipped: no browser to drive; the client's frame needs a Chromium or Chrome installed (D15)");
        return;
    }
    let (_served, address) = a_served_rail();
    let version = flywheel_surface::page::VERSION;
    let rail = protocol(address, request("tools/call", json!({"name": "rail"})))["result"].clone();
    let read = protocol(address, request("resources/read", json!({"uri": format!("ui://flywheel/{version}/rail")})));
    let bundle = read["result"]["contents"][0]["text"].as_str().expect("the bundle").to_string();

    let browser = driver::Driver::open().expect("a browser");
    let tab = browser.visit(&serve(harness(&bundle, &rail)), driver::DESKTOP).expect("the client");
    until(tab, "doc.querySelector('#rail .card.decision form.answer button') !== null");
    let number = in_frame(tab, "doc.querySelector('#rail .card.decision').getAttribute('data-number')");
    let number: u64 = number.as_str().and_then(|n| n.parse().ok()).expect("the card's number");
    let answer = in_frame(tab, "doc.querySelector('#rail .card.decision form.answer button').getAttribute('data-answer')");
    in_frame(tab, "(doc.querySelector('#rail .card.decision form.answer button').click(), true)");

    // The tap, sent through the client, and the control busy while it goes.
    let tap = next_call(tab);
    assert_eq!(in_frame(tab, "doc.querySelector('#rail form.busy') !== null"), json!(true), "the control did not answer at once");
    assert_eq!(tap["method"], json!("tools/call"));
    assert_eq!(tap["params"]["name"], json!("answer"));
    assert_eq!(tap["params"]["arguments"]["decision"], json!(number));
    assert_eq!(tap["params"]["arguments"]["answer"], answer);
    let delivery = tap["params"]["_meta"]["flywheel/delivery"].as_str().expect("a delivery of its own");
    assert!(delivery.starts_with("tap-"), "{delivery}");
    let done = carry(tab, address, &tap);
    assert_eq!(done["structuredContent"]["recorded"], json!(true), "{done}");

    // The view asks for the rail again, and draws the answer given.
    let again = next_call(tab);
    assert_eq!(again["params"]["name"], json!("rail"));
    carry(tab, address, &again);
    until(tab, &format!("doc.querySelector('#rail .card.decision[data-number=\"{number}\"]').textContent.includes('chuck')"));
    assert_eq!(in_frame(tab, "doc.querySelector('#rail form.busy') === null"), json!(true));

    // Delivered twice by the client, it is taken once (137).
    let twice = protocol(address, json!({"jsonrpc": "2.0", "id": 10, "method": "tools/call", "params": tap["params"]}));
    assert_eq!(twice["result"]["structuredContent"]["recorded"], json!(false), "{twice}");
}

/// The same result in this binary's bundle draws the rail the page draws, and
/// an object tapped in it opens as its own view, asked for through the client
/// (293a, 308, 322, S230).
#[test]
fn a_view_draws_the_rail_it_is_handed_in_a_browser() {
    if !driver::available() {
        eprintln!("skipped: no browser to drive; the client's frame needs a Chromium or Chrome installed (D15)");
        return;
    }
    let (_served, address) = a_served_rail();
    let version = flywheel_surface::page::VERSION;
    let rail = protocol(address, request("tools/call", json!({"name": "rail"})))["result"].clone();
    let read = protocol(address, request("resources/read", json!({"uri": format!("ui://flywheel/{version}/rail")})));
    let bundle = read["result"]["contents"][0]["text"].as_str().expect("the bundle").to_string();

    let browser = driver::Driver::open().expect("a browser");
    let tab = browser.visit(&serve(harness(&bundle, &rail)), driver::DESKTOP).expect("the client");
    until(tab, "doc.querySelector('#rail .card.decision') !== null");
    assert_eq!(in_frame(tab, "doc.body.getAttribute('data-view')"), json!("rail"));
    assert_eq!(in_frame(tab, "doc.getElementById('view-stale').hidden"), json!(true));
    // Only the rail: the page's header and board are not drawn in a client.
    assert_eq!(in_frame(tab, "getComputedStyle(doc.querySelector('.top')).display"), json!("none"));
    assert_eq!(in_frame(tab, "getComputedStyle(doc.getElementById('board')).display"), json!("none"));
    // And the client sizes its frame to the whole rail: the view said the
    // height it needs, so its last card is inside the frame and not cut off.
    until(
        tab,
        "(() => { const cards = doc.querySelectorAll('#rail .card'); const last = cards[cards.length - 1]; \
         return last.getBoundingClientRect().bottom <= document.getElementById('view').getBoundingClientRect().height; })()",
    );

    // The object on the card, tapped: one `object` call through the client.
    let object = in_frame(tab, "doc.querySelector('#rail .card.decision').getAttribute('data-object')");
    let object = object.as_str().expect("the card names its object").to_string();
    assert!(in_frame(tab, "(doc.querySelector('#rail .card.decision a.object').click(), true)") == json!(true));
    let mut asked = Vec::new();
    for _ in 0..200 {
        asked.extend(calls(tab));
        if !asked.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(asked.len(), 1, "{asked:?}");
    assert_eq!(asked[0]["method"], json!("tools/call"));
    assert_eq!(asked[0]["params"]["name"], json!("object"));
    assert_eq!(asked[0]["params"]["arguments"]["object"], json!(object));
    let detail = protocol(address, json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": asked[0]["params"]}));
    let script = format!("window.reply({}, {})", asked[0]["id"], detail["result"]);
    tab.evaluate(&script, false).expect("the reply is handed back");
    until(tab, "doc.body.getAttribute('data-view') === 'object'");
    assert_eq!(
        in_frame(tab, "doc.querySelector('#dk-b .surface[data-opened=\"true\"]').id"),
        json!(format!("dock-{object}"))
    );
}
