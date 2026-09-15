//! The catalogue is the one write path (193, 193a, 311; audit 6).
//!
//! A real host serves its page; what is asserted is what a client receives
//! over the socket and what the state repository and the blueprints hold after.

use chrono::{DateTime, Duration, TimeZone, Utc};
use flywheel::host::{Bindings, Host};
use flywheel_atoms::{Records, Scope};
use flywheel_domain::derived::Declaration;
use flywheel_store_git::store::sandbox;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

fn at(minute: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap() + Duration::minutes(minute)
}

fn base(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "flywheel-catalogue-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn speak(address: std::net::SocketAddr, request: &str) -> String {
    let mut socket = std::net::TcpStream::connect(address).expect("the page answers");
    socket.write_all(request.as_bytes()).expect("the request is sent");
    let mut text = String::new();
    socket.read_to_string(&mut text).expect("the page replies");
    text
}

fn body_of(reply: &str) -> &str {
    reply.split_once("\r\n\r\n").map(|(_, body)| body).unwrap_or(reply)
}

/// A client's call: JSON in, JSON out.
fn post(address: std::net::SocketAddr, path: &str, body: &Value) -> Value {
    let body = body.to_string();
    let reply = speak(
        address,
        &format!(
            "POST {path} HTTP/1.1\r\nHost: mac-mini.example\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        ),
    );
    let text = body_of(&reply);
    serde_json::from_str(text).unwrap_or_else(|e| panic!("the body {text:?}: {e}"))
}

fn get(address: std::net::SocketAddr, path: &str) -> String {
    speak(
        address,
        &format!("GET {path} HTTP/1.1\r\nHost: mac-mini.example\r\nConnection: close\r\n\r\n"),
    )
}

/// The model's catalogue rows: `profiles/surfaces.yaml` `tools.catalogue`, by
/// name, each with the arguments it names (193, D15).
fn models_rows() -> BTreeMap<String, Vec<String>> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../definitions/profiles/surfaces.yaml");
    let text = std::fs::read_to_string(path).expect("the mirrored surfaces profile");
    let read: Value = serde_yaml::from_str(&text).expect("surfaces.yaml parses");
    read["tools"]["catalogue"]
        .as_object()
        .expect("tools.catalogue is a map")
        .iter()
        .map(|(name, row)| {
            let args = row["args"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_default();
            (name.clone(), args)
        })
        .collect()
}

/// Every operation the page writes is a tool the catalogue names and a client
/// reaches by the same name; the catalogue offers no command the model's lacks;
/// and every transition 4 grants that this phase carries is in it (4, 193,
/// 193a, 311; audit 6).
///
/// The curator's surface was the one page-only write — `POST /api/curate`
/// wrote moves and a session exit as no tool. It is the `curate` tool now, so
/// here a client makes the same delivery over `/api/tools/curate` and the
/// session's exit and the signals' moves are what the page's form would leave.
#[test]
fn every_write_goes_through_the_catalogue() {
    let dir = base("one-path");
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
    host.sinks.address = "http://mac-mini.example/willdan".into();
    let defs = host.defs.clone();
    flywheel_domain::commands::put_new(
        &mut host.store,
        &defs,
        "curation/willdan",
        "curation",
        None,
        [
            ("threshold".to_string(), json!(2)),
            ("cadence".to_string(), json!(flywheel_domain::cadence::DEFAULT)),
        ]
        .into_iter()
        .collect(),
        now,
    )
    .unwrap();

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

    // The served catalogue against the model's rows: the same names with the
    // same arguments, and nothing the model lacks (193a).
    let served: Value = serde_json::from_str(body_of(&get(address, "/api/tools"))).unwrap();
    let rows = models_rows();
    let mut names = vec![];
    for tool in served["tools"].as_array().expect("a list of tools") {
        let name = tool["name"].as_str().unwrap_or_default().to_string();
        let args: Vec<String> = tool["args"]
            .as_array()
            .expect("an argument schema")
            .iter()
            .filter_map(|a| a["name"].as_str().or(a.as_str()).map(String::from))
            .collect();
        let row = rows
            .get(&name)
            .unwrap_or_else(|| panic!("`{name}` is served and is no row of the model's catalogue (193a)"));
        assert_eq!(&args, row, "`{name}` names different arguments from the model's row");
        names.push(name);
    }
    // What 4 grants, as far as this phase goes: send-back waits on stages with
    // construction (A.5, proposal: phase 2).
    for granted in ["drop", "hold", "release", "retire", "takeover", "finish", "close"] {
        assert!(names.iter().any(|n| n == granted), "4 grants `{granted}` and the catalogue lacks it: {names:?}");
    }
    assert!(names.iter().any(|n| n == "curate"), "running curation is no tool: {names:?}");

    // Two captures through the tool the page's box calls (19, 194).
    for text in ["the rows lose their numbers on the second page", "the second page drops the row numbers again"] {
        let answered = post(address, "/api/tools/capture", &json!({"args": {"text": text, "source": "page"}}));
        assert_eq!(answered["recorded"], json!(true), "{answered}");
    }
    // The tick charges curation; the session it starts is the operator's to run
    // (110, 93b).
    let session = {
        let mut held = host.lock().unwrap();
        held.set_now(at(1));
        held.sweep().unwrap();
        held.set_now(at(2));
        held.sweep().unwrap();
        let objects = held.store.list_records(&Scope::All).unwrap();
        flywheel_surface::page::curating(&objects).expect("a curation session is charged")
    };
    let signals: Vec<String> = {
        let mut held = host.lock().unwrap();
        held.store.with_world(|_, world| {
            flywheel_domain::signals::unmoved(&flywheel_domain::signals::Blueprints(world))
                .into_iter()
                .map(|s| s.id)
                .collect()
        })
    };
    assert_eq!(signals.len(), 2, "one signal per capture (19)");

    // The operator's moves, as a client of the catalogue delivers them: the
    // same tool the page's form calls (193, 311).
    let intent = "intent/rows-lose-numbers";
    let moves: Vec<Value> = signals
        .iter()
        .map(|s| json!({"signal": s, "move": "join", "target": intent}))
        .collect();
    let curated = post(
        address,
        "/api/tools/curate",
        &json!({"args": {"session": session, "moves": moves}}),
    );
    assert_eq!(curated["recorded"], json!(true), "{curated}");
    assert_eq!(curated["id"], json!(session), "{curated}");

    // What it left is the session's exit naming its deliverable — the entry
    // `flywheel exit done --deliverable move` writes — and one standing move per
    // signal in the blueprints, for `record_moves` to apply on the next tick
    // (67, 80, 107, 116).
    let mut held = host.lock().unwrap();
    let exit = held
        .store
        .thread(&session)
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == "exit")
        .next_back()
        .expect("the session reported an exit");
    assert_eq!(exit.fields.get("exit"), Some(&json!("done")));
    assert_eq!(exit.fields.get("deliverables"), Some(&json!(["move"])));
    assert_eq!(exit.by.as_deref(), Some("chuck"), "the operators list's entry gave it (153, 253a)");
    held.store.with_world(|_, world| {
        for signal in &signals {
            let standing = flywheel_domain::signals::standing_move(world, signal)
                .unwrap()
                .unwrap_or_else(|| panic!("`{signal}` has no standing move"));
            assert_eq!(standing.target, format!("join {intent}"));
        }
    });
}
