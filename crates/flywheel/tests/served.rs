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

    // The answer, through the same tool the reply grammar calls (193). A client
    // sends it as JSON and reads the record it made; the page's own control is
    // a form, and gets somewhere to go instead (310, 311).
    let body = json!({"args": {"decision": number.to_string(), "answer": "yes"}}).to_string();
    let answered = speak(
        address,
        &format!(
            "POST /api/tools/answer HTTP/1.1\r\nHost: mac-mini.example\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\
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

/// One whole response, status line and all: a link that is not served answers
/// 404, and nothing but the status line says so.
fn speak_whole(address: std::net::SocketAddr, request: &str) -> String {
    let mut socket = std::net::TcpStream::connect(address).expect("the page answers");
    socket.write_all(request.as_bytes()).expect("the request is sent");
    let mut text = String::new();
    socket.read_to_string(&mut text).expect("the page replies");
    text
}

/// A link the machinery wrote is fetched, and what comes back is the object it
/// names (308, 205a).
///
/// `links::to_object` writes `<address>/<object>` with the instance already in
/// the address, so the path a rail line, a chat rendering and a notification
/// all carry is `/<instance>/<object>`. The router serves it, the object's own
/// surface comes back open, and a link naming an object this instance does not
/// hold is refused rather than answered with the board.
#[test]
fn a_link_the_machinery_wrote_is_fetched() {
    let dir = base("link");
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
    seed(
        &mut host,
        "bolt/atlas/plan-rows",
        "bolt",
        &[("life", "open"), ("life.open.close", "offered")],
        &[("repository", json!("atlas"))],
    );
    host.sweep().unwrap();

    // The link the machinery writes for that object, not one the test spelled.
    let link = flywheel_surface::links::to_object(&host.sinks.address, "bolt/atlas/plan-rows")
        .expect("the machinery writes a link at the host's address");
    assert_eq!(link, "http://mac-mini.example/willdan/bolt/atlas/plan-rows");
    let path = link
        .split_once("//")
        .and_then(|(_, rest)| rest.split_once('/'))
        .map(|(_, path)| format!("/{path}"))
        .expect("the link has a path");

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

    // The link opens, and the surface it opens is the object's own (209, 210).
    let answered = speak_whole(
        address,
        &format!("GET {path} HTTP/1.1\r\nHost: mac-mini.example\r\nConnection: close\r\n\r\n"),
    );
    assert!(
        answered.starts_with("HTTP/1.1 200"),
        "the link the machinery wrote is not served: {}",
        answered.lines().next().unwrap_or_default()
    );
    let body = answered.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or(&answered);
    assert!(
        body.contains("id=\"dock-bolt/atlas/plan-rows\" data-kind=\"bolt\" data-answerable=\"false\" data-opened=\"true\""),
        "the object's own surface is not the one the link opened: {body}"
    );
    assert!(
        body.contains("data-opened=\"false\""),
        "every other surface stays shut: {body}"
    );

    // And the page at the address itself, which every chat rendering carries.
    let page = flywheel_surface::links::to_page(&host.lock().unwrap().sinks.address.clone()).unwrap();
    assert_eq!(page, "http://mac-mini.example/willdan");
    let answered = speak_whole(
        address,
        "GET /willdan HTTP/1.1\r\nHost: mac-mini.example\r\nConnection: close\r\n\r\n",
    );
    assert!(
        answered.starts_with("HTTP/1.1 200"),
        "the page link is not served: {}",
        answered.lines().next().unwrap_or_default()
    );

    // A link naming an object the instance does not hold is refused, and one
    // naming another instance says so: this host serves one (205a).
    let missing = speak_whole(
        address,
        "GET /willdan/bolt/atlas/nothing HTTP/1.1\r\nHost: mac-mini.example\r\nConnection: close\r\n\r\n",
    );
    assert!(
        missing.starts_with("HTTP/1.1 404"),
        "a link to no object answered {}",
        missing.lines().next().unwrap_or_default()
    );
    let elsewhere = speak_whole(
        address,
        "GET /atlas/bolt/atlas/plan-rows HTTP/1.1\r\nHost: mac-mini.example\r\nConnection: close\r\n\r\n",
    );
    assert!(
        elsewhere.starts_with("HTTP/1.1 404"),
        "a link naming another instance answered {}",
        elsewhere.lines().next().unwrap_or_default()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The repositories `scenarios/rail-mockup.yaml` describes work in.
///
/// A host seeded with that description declares them, because a described
/// instance whose host covers none of the repositories it names is an instance
/// in which every object of it waits under attention and none of the decisions
/// it was written to show can stand (149).
fn mockup_repositories() -> Vec<String> {
    vec!["atlas".into(), "switchboard".into(), "new-repo".into()]
}

/// The workspace root, where the scenarios live.
fn workspace() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

/// A host seeded from a scenario serves that scenario's rail (94, D15, 310).
///
/// `scenarios/rail-mockup.yaml` holds every decision kind once, running work
/// and a tail, and until now the only thing that could stand on it was the
/// scenario runner's in-process store — so there was no way to open a served
/// page on a described state, and a fresh instance shows the operator nothing.
/// The seed puts the described objects into the host's own state repository
/// through the store's write path, and the page at the port carries that
/// scenario's decisions, by the numbers the register gave them.
#[test]
fn a_seeded_host_serves_the_scenarios_rail() {
    let dir = base("seeded");
    let now = at(0);
    let git = sandbox(&dir, "laptop", now).unwrap();
    let mut host = Host::over(
        "laptop",
        "willdan",
        flywheel_domain::set::load().unwrap(),
        git,
        Bindings { world: "host".into(), workspace: "recorded".into(), sessions: "operator".into() },
        Declaration { repositories: mockup_repositories(), types: vec![], kinds: vec!["all".into()] },
        now,
    );
    host.sinks.address = "http://laptop.example/willdan".into();

    let seeded = flywheel::seed::from_scenario(
        &mut host.store.git,
        &workspace().join("scenarios/rail-mockup.yaml"),
        now,
        &host.declaration.clone(),
    )
    .expect("the scenario's given state goes into the state store");
    assert_eq!(seeded.register_start, Some(412), "the scenario names the register's start");
    assert!(seeded.objects >= 27, "the scenario describes a whole instance: {}", seeded.objects);

    // What the description makes stand, read the way the page reads it.
    let defs = host.defs.clone();
    let rail = flywheel_domain::commands::rail(&mut host.store, &defs).unwrap();
    let standing: Vec<(u32, String, String)> = rail
        .iter()
        .map(|d| (d.number.expect("every standing decision is numbered"), d.kind.clone(), d.object.clone()))
        .collect();
    // The nine the description raises, in the order the register numbered
    // them, which is the order the chat prints (15).
    assert_eq!(
        standing,
        vec![
            (412, "intent-proposed".into(), "intent/atlas-provider-limits".into()),
            (413, "elaboration-proposed".into(), "elaboration/atlas-provider-limits/research".into()),
            (414, "elaboration-proposed".into(), "elaboration/atlas-provider-limits/prototype".into()),
            (415, "claim-moved".into(), "bolt/atlas/plan-rows".into()),
            (416, "unit-proposed".into(), "unit/atlas/status-writer".into()),
            (417, "question".into(), "unit/atlas/rail-tail/wi-1".into()),
            (418, "unit-proposed".into(), "unit/atlas/retry-jitter".into()),
            (419, "unit-proposed".into(), "unit/atlas/chores-1".into()),
            (420, "unit-proposed".into(), "unit/new-repo/baseline-1".into()),
        ],
        "the seeded host raises what the scenario runner raises in process, and numbers it the same"
    );

    // And the page serves them: the rail the operator opens is that rail.
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
    let page = speak(
        address,
        "GET / HTTP/1.1\r\nHost: laptop.example\r\nConnection: close\r\n\r\n",
    );
    for (number, _, object) in &standing {
        assert!(
            page.contains(&format!("data-number=\"{number}\"")),
            "decision {number} is not on the served rail"
        );
        assert!(
            page.contains(object.as_str()),
            "decision {number}'s object {object} is not on the served page"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// One whole response with its headers, so a test can follow where a control
/// sent the operator.
fn post_form(address: std::net::SocketAddr, path: &str, body: &str, referrer: &str) -> String {
    let mut socket = std::net::TcpStream::connect(address).expect("the page answers");
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .expect("a bounded wait: a page that does not answer is a failure, not a hang");
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: laptop.example\r\nConnection: close\r\n\
         Referer: http://laptop.example{referrer}\r\n\
         Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(request.as_bytes()).expect("the request is sent");
    let mut text = String::new();
    socket.read_to_string(&mut text).expect("the page replies");
    text
}

/// The form the page rendered for one answer on one decision: its action and
/// the fields it carries, read out of the document rather than spelled by the
/// test, so what is posted is what a browser would post.
fn form_on(page: &str, number: u32, answer: &str) -> (String, String) {
    let card = page
        .split("<article ")
        .find(|block| block.contains(&format!("data-number=\"{number}\"")))
        .unwrap_or_else(|| panic!("decision {number} is not on the rail"));
    let form = card
        .split("<form ")
        .find(|block| block.contains(&format!("value=\"{answer}\"")))
        .unwrap_or_else(|| panic!("decision {number} carries no `{answer}` control"));
    let action = form
        .split_once("action=\"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(action, _)| action.to_string())
        .expect("the control posts somewhere");
    let mut fields: Vec<String> = Vec::new();
    for input in form.split("<input ").skip(1) {
        let value_of = |name: &str| -> Option<String> {
            input
                .split_once(&format!("{name}=\""))
                .and_then(|(_, rest)| rest.split_once('"'))
                .map(|(value, _)| value.to_string())
        };
        if let (Some(name), Some(value)) = (value_of("name"), value_of("value")) {
            fields.push(format!("{name}={value}"));
        }
    }
    (action, fields.join("&"))
}

/// A control on the rail answers, and the page the operator lands on shows it
/// (137, 153, 154, 310, 311).
///
/// The page runs no script, so a control is a plain form; a form answered with
/// a body strands the operator on it. What a browser gets is 303 See Other back
/// to the page it was on, and the page that comes back carries the answer, as
/// does a reload of it afterwards. A caller that is not a browser still reads
/// the record it made.
#[test]
fn a_tap_on_the_seeded_rail_answers_and_the_next_render_shows_it() {
    let dir = base("tap");
    let now = at(0);
    let git = sandbox(&dir, "laptop", now).unwrap();
    let mut host = Host::over(
        "laptop",
        "willdan",
        flywheel_domain::set::load().unwrap(),
        git,
        Bindings { world: "host".into(), workspace: "recorded".into(), sessions: "operator".into() },
        Declaration { repositories: mockup_repositories(), types: vec![], kinds: vec!["all".into()] },
        now,
    );
    host.sinks.address = "http://laptop.example/willdan".into();
    flywheel::seed::from_scenario(
        &mut host.store.git,
        &workspace().join("scenarios/rail-mockup.yaml"),
        now,
        &host.declaration.clone(),
    )
    .expect("the scenario's given state goes into the state store");

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

    // The page as the operator has it, and the control as it rendered.
    let page = speak(
        address,
        "GET / HTTP/1.1\r\nHost: laptop.example\r\nConnection: close\r\n\r\n",
    );
    let (action, body) = form_on(&page, 412, "yes");
    assert_eq!(action, "/api/tools/answer");
    assert!(body.contains("decision=412") && body.contains("answer=yes"), "{body}");
    assert!(
        !page.contains("data-answer=\"yes\" data-given-by=\"chuck\""),
        "nothing is answered yet"
    );

    // The tap. What comes back is somewhere to go, not something to read.
    let answered = post_form(address, &action, &body, "/");
    let status = answered.lines().next().unwrap_or_default().to_string();
    assert!(
        status.starts_with("HTTP/1.1 303"),
        "a control on the page answered `{status}`; a form post lands the operator back on a \
         page and never on a body (310, 311)"
    );
    let location = answered
        .lines()
        .find_map(|line| line.strip_prefix("location: ").or_else(|| line.strip_prefix("Location: ")))
        .map(str::trim)
        .expect("it says where the operator goes");
    assert_eq!(location, "/", "back to the page the control was on");

    // The page the operator lands on shows the answer they gave (153, 154).
    let landed = speak(
        address,
        &format!("GET {location} HTTP/1.1\r\nHost: laptop.example\r\nConnection: close\r\n\r\n"),
    );
    assert!(
        landed.contains("data-answer=\"yes\" data-given-by=\"chuck\""),
        "the page the operator landed on does not show the answer they gave"
    );
    // And a reload shows it still: the answer is recorded, not remembered (310).
    let again = speak(
        address,
        "GET / HTTP/1.1\r\nHost: laptop.example\r\nConnection: close\r\n\r\n",
    );
    assert!(
        again.contains("data-answer=\"yes\" data-given-by=\"chuck\""),
        "a reload lost the answer"
    );

    // The object's own path serves it too, now that a link the machinery wrote
    // is a path this router answers (205a, 308).
    let on_the_object = post_form(
        address,
        &form_on(&page, 419, "yes").0,
        &form_on(&page, 419, "yes").1,
        "/willdan/unit/atlas/chores-1",
    );
    assert!(on_the_object.lines().next().unwrap_or_default().starts_with("HTTP/1.1 303"));
    assert!(
        on_the_object.contains("/willdan/unit/atlas/chores-1"),
        "the operator goes back to the object's page they answered from: {on_the_object}"
    );

    // A refusal is not a dead end: the operator lands on the page with the
    // reason on it, rather than on a body (81, 310). An answer to a number the
    // register never gave is not one — it is recorded and shown under
    // attention (6), and so is a call to a tool the catalogue does not name
    // (4) — so what is refused here is a call the catalogue does name and
    // cannot read: `drop` with no object on it (193).
    let refused = post_form(address, "/api/tools/drop", "answer=yes", "/");
    let to = refused
        .lines()
        .find_map(|line| line.strip_prefix("location: ").or_else(|| line.strip_prefix("Location: ")))
        .map(str::trim)
        .expect("a refusal says where the operator goes too");
    assert!(to.starts_with("/?refused="), "{to}");
    let with_reason = speak(
        address,
        &format!("GET {to} HTTP/1.1\r\nHost: laptop.example\r\nConnection: close\r\n\r\n"),
    );
    assert!(
        with_reason.contains("data-refused=\"true\""),
        "the reason is not on the page the operator was sent back to"
    );

    // A caller that is not a browser is unchanged: it reads the record it made.
    let body = serde_json::json!({"args": {"decision": "418", "answer": "yes"}}).to_string();
    let json = speak(
        address,
        &format!(
            "POST /api/tools/answer HTTP/1.1\r\nHost: laptop.example\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\n\
             Connection: close\r\n\r\n{body}",
            body.len()
        ),
    );
    let json: serde_json::Value =
        serde_json::from_str(&json).unwrap_or_else(|e| panic!("the body {json:?}: {e}"));
    assert_eq!(json["recorded"], serde_json::json!(true), "{json}");
    let _ = std::fs::remove_dir_all(&dir);
}
