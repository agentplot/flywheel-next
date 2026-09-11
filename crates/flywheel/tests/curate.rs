//! A decision, reachable from a fresh instance with nothing written by hand
//! (14.2, D16).
//!
//! Captures land through the page's box, cross curation's threshold and charge
//! its session; the operator is that session (93b), so the page carries the
//! curator's surface; the moves submitted there are the session's delivery and
//! its exit; `record_moves` and `propose_intents` run on the next tick; and the
//! proposed intent's decision stands on the served rail with a number, where
//! the answer given is a commit a second reader of the state repository finds
//! (110, 116, 153, 154, 160).

use chrono::{DateTime, Duration, TimeZone, Utc};
use flywheel::host::{Bindings, Host};
use flywheel_atoms::Records;
use flywheel_domain::derived::Declaration;
use flywheel_store_git::store::sandbox;
use serde_json::json;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

fn at(minute: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap() + Duration::minutes(minute)
}

fn base(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "flywheel-curate-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// One request, spoken over a socket, so what is asserted is what a client
/// receives and not what a handler hands itself.
fn speak(address: std::net::SocketAddr, request: &str) -> String {
    let mut socket = std::net::TcpStream::connect(address).expect("the page answers");
    socket.write_all(request.as_bytes()).expect("the request is sent");
    let mut text = String::new();
    socket.read_to_string(&mut text).expect("the page replies");
    text.split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or(text)
}

/// One of the page's own controls, used. A control is a plain form with no
/// script behind it, so what comes back is a See Other to the page the operator
/// was on and never a body they are stranded on (310, 311). What the control
/// did is read from the store, which is where it wrote.
fn control(address: std::net::SocketAddr, path: &str, body: &str) -> String {
    let answered = whole(
        address,
        &format!(
            "POST {path} HTTP/1.1\r\nHost: mac-mini.example\r\n\
             Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        ),
    );
    let first = answered.lines().next().unwrap_or_default().to_string();
    assert!(
        first.contains("303"),
        "the control did not send the operator back to the page: {answered}"
    );
    answered
        .lines()
        .find_map(|line| line.strip_prefix("location: ").or_else(|| line.strip_prefix("Location: ")))
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// The whole reply, status line and headers included.
fn whole(address: std::net::SocketAddr, request: &str) -> String {
    let mut socket = std::net::TcpStream::connect(address).expect("the page answers");
    socket.write_all(request.as_bytes()).expect("the request is sent");
    let mut text = String::new();
    socket.read_to_string(&mut text).expect("the page replies");
    text
}

fn get(address: std::net::SocketAddr) -> String {
    speak(
        address,
        "GET / HTTP/1.1\r\nHost: mac-mini.example\r\nConnection: close\r\n\r\n",
    )
}

/// A form field's value, percent-encoded the way a browser sends one. Only the
/// characters a signal id and an intent id carry need it.
fn encode(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' | '/' => c.to_string(),
            ' ' => "+".to_string(),
            other => format!("%{:02X}", other as u32),
        })
        .collect()
}

/// Captures cross the threshold, curation is charged, the operator moves the
/// signals on the page, and the next tick puts a numbered decision on the
/// served rail whose answer is a commit a second reader finds (110, 116, 153,
/// 154, D16).
#[test]
fn curated_from_the_page_raises_a_decision() {
    let dir = base("raises");
    let now = at(0);
    let git = sandbox(&dir, "mac-mini", now).unwrap();
    let mut host = Host::over(
        "mac-mini",
        "willdan",
        flywheel_domain::set::load().unwrap(),
        git,
        Bindings {
            world: "host".into(),
            workspace: "recorded".into(),
            sessions: "operator".into(),
        },
        Declaration {
            repositories: vec!["atlas".into()],
            types: vec![],
            kinds: vec!["all".into()],
        },
        now,
    );
    host.sinks.address = "http://mac-mini.example/willdan".into();

    // The instance's curation machine, with a threshold two captures cross
    // (`curation.yaml` record.threshold, 110).
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
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
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

    // Two captures through the page's own box: each is captured whole and
    // writes one signal of kind ask (19, 194).
    for text in [
        "the rows lose their numbers on the second page",
        "the second page drops the row numbers again",
    ] {
        let back = control(
            address,
            "/api/tools/capture",
            &format!("text={}&source=page", encode(text)),
        );
        assert!(
            !back.contains("refused="),
            "the capture box refused what was typed in it: {back}"
        );
    }

    // The tick charges curation: the count crossed the threshold, and the
    // session it starts is the operator's to run (110, 93b).
    {
        let mut held = host.lock().unwrap();
        held.set_now(at(1));
        held.sweep().unwrap();
        held.set_now(at(2));
        held.sweep().unwrap();
    }
    let page = get(address);
    assert!(
        page.contains("id=\"curate-box\""),
        "the page carries no curator's surface: {page}"
    );

    // The two unmoved signals, each with the standing moves as controls: never
    // a word read out of what the signal says (107, 116, 194).
    let signals: Vec<String> = {
        let mut held = host.lock().unwrap();
        held.store.with_world(|_, world| {
            flywheel_domain::signals::unmoved(&flywheel_domain::signals::Blueprints(world))
                .into_iter()
                .map(|s| s.id)
                .collect()
        })
    };
    assert_eq!(signals.len(), 2, "the box wrote one signal per capture (19)");
    for signal in &signals {
        assert!(
            page.contains(&format!("data-signal=\"{signal}\"")),
            "`{signal}` is not on the curator's surface: {page}"
        );
        assert!(
            page.contains(&format!("name=\"move.{signal}\"")),
            "`{signal}` carries no move control"
        );
    }
    for word in flywheel_domain::signals::MOVES {
        assert!(
            page.contains(&format!("<option value=\"{word}\">{word}</option>")),
            "the surface offers no `{word}` (107, 116)"
        );
    }

    // The operator joins them both into one proposed intent. That submit is the
    // curation session's delivery and its exit, through the same record
    // `flywheel exit done --deliverable move` writes (67, 93b).
    let intent = "intent/rows-lose-numbers";
    let body = signals
        .iter()
        .map(|s| {
            format!(
                "move.{0}=join&target.{0}={1}",
                encode(s),
                encode(intent)
            )
        })
        .collect::<Vec<_>>()
        .join("&");
    let back = control(address, "/api/curate", &body);
    assert!(
        !back.contains("refused="),
        "the curator's submit was refused: {back}"
    );
    // The session the moves were delivered under: the one the README's
    // walkthrough names (regions::session_stem, session.yaml id).
    let session = "curation/willdan/main/1";

    // The exit is a thread entry on the session, naming what it delivered (67,
    // 80).
    {
        let held = host.lock().unwrap();
        let exit = held
            .store
            .thread(session)
            .unwrap()
            .into_iter()
            .filter(|e| e.kind == "exit")
            .next_back()
            .expect("the session reported an exit");
        assert_eq!(exit.fields.get("exit"), Some(&json!("done")));
        assert_eq!(exit.fields.get("deliverables"), Some(&json!(["move"])));
        assert_eq!(exit.by.as_deref(), Some("chuck"));
    }

    // The next tick applies them: `record_moves` gives every judged signal its
    // one standing move and `propose_intents` makes the intent its joins name
    // (107, 109, 110, 116).
    let number = {
        let mut held = host.lock().unwrap();
        let mut found = None;
        for minute in 3..9 {
            held.set_now(at(minute));
            held.sweep().unwrap();
            let defs = held.defs.clone();
            found = flywheel_domain::commands::rail(&mut held.store, &defs)
                .unwrap()
                .iter()
                .find(|d| d.object == intent)
                .and_then(|d| d.number);
            if found.is_some() {
                break;
            }
        }
        found.expect("the joined signals became a proposed intent with a decision on the rail")
    };

    // It cites the two signals it was joined from, which is its weight (109).
    {
        let held = host.lock().unwrap();
        let proposed = Records::get(&held.store, intent)
            .unwrap()
            .expect("the proposed intent");
        assert_eq!(proposed.record.get("signals_count"), Some(&json!(2)));
    }

    // And it is on the served rail, numbered (15, 310).
    let page = get(address);
    assert!(
        page.contains(&format!("data-number=\"{number}\"")),
        "the decision curation raised is not on the served rail: {page}"
    );

    // The answer given on the page, through the one tool the reply grammar
    // calls, is a commit a second reader of the state repository finds with no
    // host running (193, 153, 132, 160).
    let back = control(
        address,
        "/api/tools/answer",
        &format!("decision={number}&answer=yes"),
    );
    assert!(!back.contains("refused="), "the answer was refused: {back}");
    // What the control wrote, read where it wrote it: the response naming the
    // number the operator answered (153, 193).
    let id = {
        let held = host.lock().unwrap();
        held.store
            .list_records(&flywheel_atoms::Scope::Machine("response".into()))
            .unwrap()
            .into_iter()
            .find(|o| o.record.get("decision").and_then(|v| v.as_u64()) == Some(number as u64))
            .map(|o| o.id.trim_start_matches("response/").to_string())
            .expect("the answer was recorded as a response")
    };

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

    let _ = std::fs::remove_dir_all(&dir);
}
