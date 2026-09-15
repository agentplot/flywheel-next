//! The page keeps itself current: what moved on the host appears on the open
//! page as the regions that changed, with the decision in hand kept (S221, S235,
//! 21.8).

use super::driver;

use flywheel_scenario::bindings::FilesWorld;
use flywheel_scenario::conformance::{drive, Suite};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// An instance with three proposed intents standing on the rail, served.
fn served() -> driver::Served {
    let objects: Vec<Value> = (0..3)
        .map(|n| json!({"id": format!("intent/subject-{n}"), "machine": "intent", "state": {"life": "proposed"}, "record": {"title": format!("subject {n}")}}))
        .collect();
    let scenario = serde_json::from_value(json!({
        "scenario": "current", "title": "the page keeps itself current", "profiles": ["git-only"],
        "satisfies": [310], "given": {"objects": objects}, "when": [],
    }))
    .expect("a scenario");
    let suite = Suite::open(&root().join("conformance")).expect("the suite");
    let defs = flywheel_engine::load::load_dir(&root().join("definitions")).expect("the machines load");
    let mut runtime = drive::seed(defs, &scenario, &suite).expect("the instance seeds");
    let base = std::env::temp_dir().join(format!("flywheel-current-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let host = runtime.store.me();
    runtime.store.bind_state_repository(&base, &[host]).expect("the state repository");
    let standing = flywheel_domain::commands::rail_read(&runtime.store, &runtime.defs).expect("the rail");
    let mut register = flywheel_domain::commands::register(&runtime.store).expect("the register");
    if register.number_all(&standing) {
        let ids: Vec<String> = standing.iter().map(|d| d.id.clone()).collect();
        flywheel_domain::commands::set_register(&mut runtime.store, &register, &ids).expect("numbered");
    }
    driver::Served::page_over(runtime.store, FilesWorld::new(), runtime.defs, "chuck").expect("the page serves")
}

fn js(tab: &headless_chrome::Tab, expression: &str) -> Value {
    tab.evaluate(expression, false).expect("the browser evaluates").value.unwrap_or(Value::Null)
}

/// A capture posted to the host from elsewhere appears on the open page once the
/// host says its store moved, as the regions that changed and nothing else: the
/// decision in hand stays in hand, the page is not loaded again, and the rail's
/// cards are the same elements they were (S221, S235, 21.8).
#[test]
fn a_capture_posted_elsewhere_appears_with_the_hand_kept() {
    if !driver::available() {
        eprintln!("skipped: no browser to drive; the open page needs a Chromium or Chrome (D15)");
        return;
    }
    let served = served();
    let browser = driver::Driver::open().expect("a browser");
    let tab = browser.visit(&format!("{}willdan/", served.url), driver::DESKTOP).expect("the page");
    driver::eventually(|| (js(tab, "document.querySelectorAll('#rail .card.decision').length") == json!(3)).then_some(()))
        .expect("three decisions stand on the rail");
    // The hand moves to the second decision, and the page marks itself so a load
    // of it again would show.
    js(tab, "document.dispatchEvent(new KeyboardEvent('keydown', {key: 'j', bubbles: true})); window.__loaded = 'once'; true");
    let in_hand = js(tab, "document.querySelector('#rail .card.decision.focused').getAttribute('data-number')");
    let second = js(tab, "document.querySelectorAll('#rail .card.decision')[1].getAttribute('data-number')");
    assert_eq!(in_hand, second, "the hand did not move to the second decision");
    js(tab, "window.__card = document.querySelector('#rail .card.decision.focused'); true");

    // A capture from elsewhere: a client of the host's catalogue, not this page.
    let address = served.url.trim_start_matches("http://").trim_end_matches('/').to_string();
    let payload = json!({"args": {"text": "the order total is wrong on the second page", "source": "console"}}).to_string();
    let mut socket = std::net::TcpStream::connect(&address).expect("the host");
    write!(
        socket,
        "POST /api/tools/capture HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
        payload.len()
    )
    .expect("the capture");
    let mut reply = String::new();
    socket.read_to_string(&mut reply).expect("the reply");
    assert!(reply.starts_with("HTTP/1.1 200"), "the capture was not taken: {reply}");
    // The loop beside a host raises the generation when its store moved (S221).
    served.changed.send_modify(|generation| *generation += 1);

    driver::eventually(|| {
        (js(tab, "document.getElementById('lane-inception').textContent.includes('the order total is wrong')") == json!(true)).then_some(())
    })
    .expect("the capture never appeared on the open page");
    assert_eq!(js(tab, "window.__loaded"), json!("once"), "the page was loaded again");
    assert_eq!(
        js(tab, "document.querySelector('#rail .card.decision.focused').getAttribute('data-number')"),
        in_hand,
        "the decision in hand was lost"
    );
    assert_eq!(js(tab, "window.__card === document.querySelector('#rail .card.decision.focused')"), json!(true), "the rail was drawn again though it did not move");
}
