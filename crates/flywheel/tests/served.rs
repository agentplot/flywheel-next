//! The page the running host serves, over its own state store (D10a, D11, 307).
//!
//! One process holds the loop and the page: what the page shows is what the
//! last tick wrote, and an answer given on the page is a commit in the state
//! repository that any reader finds with no host running (132, 141, 153, 137,
//! 160, 193).

use chrono::{DateTime, Duration, TimeZone, Utc};
use flywheel::host::{Bindings, Host};
use flywheel_atoms::{Records, Scope, StateStore};
use flywheel_domain::derived::Declaration;
use flywheel_engine::Object;
use flywheel_store_git::store::sandbox;
use serde_json::json;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

fn at(minute: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap() + Duration::minutes(minute)
}

/// Where this test's state repository and its checkouts live.
fn base(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "flywheel-served-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn seed(host: &mut Host, id: &str, machine: &str, states: &[(&str, &str)], record: &[(&str, serde_json::Value)]) {
    let mut object = Object {
        id: id.to_string(),
        machine: machine.to_string(),
        parent: None,
        config: states.iter().map(|(r, s)| (r.to_string(), s.to_string())).collect(),
        entered_at: states.iter().map(|(r, _)| (r.to_string(), host.now())).collect(),
        record: record
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect::<BTreeMap<_, _>>(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    };
    if object.config.is_empty() {
        flywheel_engine::initialise(&host.defs, &mut object, host.now());
    }
    host.store.git.seed_objects(std::slice::from_ref(&object)).unwrap();
}

/// One request, spoken over a socket, so what is asserted is what a client
/// receives and not what a handler hands itself.
fn speak(address: std::net::SocketAddr, request: &str) -> String {
    let mut socket = std::net::TcpStream::connect(address).expect("the page answers");
    socket.write_all(request.as_bytes()).expect("the request is sent");
    let mut text = String::new();
    socket.read_to_string(&mut text).expect("the page replies");
    text.split_once("\r\n\r\n").map(|(_, body)| body.to_string()).unwrap_or(text)
}

/// A decision the running host raised stands on the served rail, and an answer
/// posted through the page's own tool lands as a commit in the state
/// repository (193, 153, 137, 160, D11).
#[test]
fn served_host_answers_from_its_state_repository() {
    let dir = base("answers");
    let now = at(0);
    let git = sandbox(&dir, "mac-mini", now).unwrap();
    let mut host = Host::over(
        "mac-mini",
        "willdan",
        flywheel_domain::set::load().unwrap(),
        git,
        Bindings { world: "host".into(), workspace: "recorded".into(), sessions: "operator".into() },
        Declaration { repositories: vec!["atlas".into()], types: vec![], kinds: vec!["all".into()] },
        now,
    );
    // The host's own address: every link the page writes is at it, and it is
    // what decides whether the page may be served unsigned-in (205a, 253a, D10a).
    host.sinks.address = "http://mac-mini.example/willdan".into();
    seed(
        &mut host,
        "bolt/atlas/plan-rows",
        "bolt",
        &[("life", "open"), ("life.open.close", "offered")],
        &[("repository", json!("atlas"))],
    );

    // The loop runs, and the decision it raises takes its number (15).
    host.sweep().unwrap();
    let defs = host.defs.clone();
    let number = flywheel_domain::commands::rail(&mut host.store, &defs)
        .unwrap()
        .iter()
        .find(|d| d.object == "bolt/atlas/plan-rows")
        .and_then(|d| d.number)
        .expect("the running host numbered the decision");

    // The page, served by that same host, over that same store.
    let host = Arc::new(Mutex::new(host));
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let served = flywheel::serve::page_of(&host, 4242, &["chuck".to_string()]);
    let address = runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let served = served.clone();
        tokio::spawn(async move {
            let _ = flywheel_surface::http::serve_on(served, listener).await;
        });
        address
    });

    // The rail the page renders holds the number the host gave (15, 310).
    let page = speak(
        address,
        "GET / HTTP/1.1\r\nHost: mac-mini.example\r\nConnection: close\r\n\r\n",
    );
    assert!(
        page.contains(&format!("data-number=\"{number}\"")),
        "the decision the host raised is not on the served rail: {page}"
    );

    // The answer, through the same tool the reply grammar calls (193).
    let body = format!("decision={number}&answer=yes");
    let answered = speak(
        address,
        &format!(
            "POST /api/tools/answer HTTP/1.1\r\nHost: mac-mini.example\r\n\
             Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        ),
    );
    let answered: serde_json::Value =
        serde_json::from_str(&answered).unwrap_or_else(|e| panic!("the body {answered:?}: {e}"));
    assert_eq!(answered["recorded"], json!(true), "{answered}");
    let id = answered["id"].as_str().expect("the response has an id").to_string();

    // And it is a commit on the shared line: a second reader of the state
    // repository, with no host running, finds the record (132, 160, 167).
    let reader = sandbox(&dir, "reader", now).unwrap();
    let held = reader
        .get(&format!("response/{id}"))
        .expect("the reader reads the shared line")
        .expect("the answer landed in the state repository as a commit");
    assert_eq!(held.machine, "response");
    assert_eq!(
        held.record.get("given_by").and_then(|v| v.as_str()),
        Some("chuck"),
        "the operators list's single entry is what the response records (153, 253a)"
    );

    // The next tick of the running host reads it off its own store (137).
    {
        let mut held = host.lock().unwrap();
        held.set_now(at(1));
        held.sweep().unwrap();
        assert!(
            held.store
                .list(&Scope::All)
                .unwrap()
                .objects
                .iter()
                .any(|o| o.applied_responses.contains(&id)),
            "the response the page wrote was applied by the loop"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
