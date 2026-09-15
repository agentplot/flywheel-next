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
    // And nothing else: an instance no scenario was applied into has no tour,
    // so the page carries no overlay and asks for itself no again. The overlay
    // is the product's own onboarding and is turned on by a fact in the store,
    // never by a flag, so an ordinary instance has never had one
    // (`design/flywheel-next/scenarios/storefront.md`).
    assert!(!page.contains("class=\"tour"), "an instance with no scenario has no overlay");
    assert!(!page.contains("http-equiv=\"refresh\""), "and nothing refreshes it");

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
        body.contains("id=\"dock-bolt/atlas/plan-rows\" data-kind=\"bolt\" data-answerable=\"true\" data-opened=\"true\""),
        "the object's own surface is not the one the link opened: {body}"
    );
    assert!(
        body.contains("data-opened=\"false\""),
        "every other surface stays shut: {body}"
    );
    // And it opened with the answer controls in reach, which is what the link
    // is for: the surface's own footer carries the bolt's answers, so the
    // operator the notification reached answers where they landed and is not
    // sent back to the rail to hunt for the card (308, S27).
    let surface = body
        .split("id=\"dock-bolt/atlas/plan-rows\"")
        .nth(1)
        .and_then(|rest| rest.split("</article>").next())
        .expect("the object's surface");
    assert!(
        surface.contains("class=\"dk-f\"") && surface.contains("data-answerable=\"true\""),
        "the surface the link opened carries no answers at its foot (308, S27): {surface}"
    );
    assert!(
        surface.contains(&format!("action=\"/api/tools/{}\"", flywheel_surface::catalogue::ANSWER)),
        "the answers on the surface do not post to the catalogue (193): {surface}"
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
    // The eight the description raises, in the order the register numbered
    // them, which is the order the chat prints (15).
    //
    // The mockup drew the intent's two proposed elaborations as cards beside
    // it, and that is the one thing here that is the mockup's rather than the
    // model's: requirement 10's first decision kind is "a proposed intent,
    // with its proposed elaborations", so while the intent is itself proposed
    // they ride on its card and take no number. Until the intent is a subject
    // at all there is nothing to decide about how to understand it, and two
    // cards the operator could not answer are two the rail should not have
    // held.
    assert_eq!(
        standing,
        vec![
            (412, "intent-proposed".into(), "intent/atlas-provider-limits".into()),
            (413, "claim-moved".into(), "bolt/atlas/plan-rows".into()),
            (414, "unit-proposed".into(), "unit/atlas/status-writer".into()),
            (415, "question".into(), "unit/atlas/rail-tail/wi-1".into()),
            (416, "unit-proposed".into(), "unit/atlas/retry-jitter".into()),
            (417, "unit-proposed".into(), "unit/atlas/chores-1".into()),
            (418, "unit-proposed".into(), "unit/new-repo/baseline-1".into()),
            // An elaboration proposed on an intent already open is a decision
            // of the operator's own, and is numbered like any other (21, 27).
            (419, "elaboration-proposed".into(), "elaboration/loop-granularity/e5".into()),
        ],
        "the seeded host raises what the scenario runner raises in process, and numbers it the same"
    );
    // And the intent's one card names what rides on it, so the operator reads
    // the subject and the work proposed to understand it as the one thing they
    // are being asked about (10).
    let intent = rail
        .iter()
        .find(|d| d.object == "intent/atlas-provider-limits")
        .expect("the proposed intent stands");
    let mut carried = intent.folds.clone();
    carried.sort();
    assert_eq!(
        carried,
        vec![
            "elaboration/atlas-provider-limits/prototype".to_string(),
            "elaboration/atlas-provider-limits/research".to_string(),
            "intent/atlas-provider-limits".to_string(),
        ],
        "the card does not name the elaborations proposed with it"
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
    // The rail is numbered by the tick, never by a request: a read that
    // renumbered would be a commit on the shared line made on the request path
    // by a host that may not hold the rail's lease (15, D12). `flywheel host
    // seed` derives it once for the same reason, so the page the operator opens
    // carries the numbers rather than waiting on a loop nobody started.
    {
        let defs = host.defs.clone();
        flywheel_domain::commands::rail(&mut host.store, &defs).expect("the rail is numbered");
    }

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
        &form_on(&page, 417, "yes").0,
        &form_on(&page, 417, "yes").1,
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

/// An answer given on the page is applied on the pass that answer caused, and
/// it puts nothing back on the rail (13, 130, 137, 153).
///
/// Two things had to be true for this and neither was. The store delivered a
/// response to the object it named and never to the object its *number* named,
/// and an answer names a number and nothing else — so the guard on the intent
/// never saw the yes and the decision stood. And the response machine read
/// `response.decision_present` against a standing set nobody filled, so every
/// answer the operator gave came back as its own attention line. Four answers
/// made six cards; 13 says one response is enough and the operator never
/// nudges.
#[test]
fn an_answer_applies_on_the_pass_it_caused_and_puts_nothing_back_on_the_rail() {
    let dir = base("applies");
    let now = at(0);
    let git = sandbox(&dir, "laptop", now).unwrap();
    let mut host = Host::over(
        "laptop",
        "willdan",
        flywheel_domain::set::load().unwrap(),
        git,
        Bindings { world: "host".into(), workspace: "recorded".into(), sessions: "operator".into() },
        // One laptop, the instance's only host: it takes everything (149).
        Declaration { repositories: vec!["all".into()], types: vec![], kinds: vec!["all".into()] },
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

    // Settle it, so what follows is the answer's doing and not the first
    // sweep's.
    {
        host.set_now(at(1));
        host.sweep().unwrap();
    }
    let intent = "intent/atlas-provider-limits";
    let number = {
        let defs = host.defs.clone();
        flywheel_domain::commands::rail(&mut host.store, &defs)
            .unwrap()
            .into_iter()
            .find(|d| d.object == intent)
            .and_then(|d| d.number)
            .expect("the intent's proposal stands, numbered")
    };

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

    // The tap.
    let page = speak(
        address,
        "GET / HTTP/1.1\r\nHost: laptop.example\r\nConnection: close\r\n\r\n",
    );
    let (action, body) = form_on(&page, number, "yes");
    post_form(address, &action, &body, "/");

    // One pass, a second after the sweep that settled it, so the sweep is not
    // due: what moves the intent is the notify the answer itself caused (130,
    // D6, D7).
    {
        let mut held = host.lock().unwrap();
        held.set_now(at(1) + Duration::seconds(1));
        assert!(
            held.notified().unwrap().iter().any(|id| id == intent),
            "the answer did not notify the object it answers, so the rail waits out the sweep"
        );
        held.once().unwrap();
    }

    // The intent opened, and it carries the response that opened it (137).
    {
        let held = host.lock().unwrap();
        let opened = Records::get(&held.store, intent).unwrap().expect("the intent");
        assert_eq!(
            opened.config.get("life").map(String::as_str),
            Some("open"),
            "the answer was recorded and never applied: {:?}",
            opened.config
        );
        assert_eq!(
            opened.applied_responses.len(),
            1,
            "the response it applied is not listed on it"
        );
    }

    // And nothing came back: the decision is gone and no attention line stands
    // in its place (13, 6).
    {
        let mut held = host.lock().unwrap();
        let defs = held.defs.clone();
        let rail = flywheel_domain::commands::rail(&mut held.store, &defs).unwrap();
        assert!(
            !rail.iter().any(|d| d.number == Some(number)),
            "the decision the operator answered is still standing"
        );
        let raised: Vec<&str> = rail
            .iter()
            .filter(|d| d.kind == "response-unapplicable")
            .map(|d| d.object.as_str())
            .filter(|object| *object != "response/lost-1")
            .collect();
        assert!(
            raised.is_empty(),
            "answering put {raised:?} back on the rail as attention; one response is enough (13)"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// "yes all" sends one `answer` per approve decision, in number order, each
/// recorded on its own, and it answers the numbers the control named and no
/// others (11, S2, S7, 17.5).
///
/// The page is rendered in one request and the control is used in the next. A
/// sweep over a fresh read would answer a decision that arrived between the
/// two — one the operator never saw — so the numbers travel with the tap.
#[test]
fn yes_all_answers_the_numbers_it_named_and_never_a_batch() {
    let dir = base("yes-all");
    let now = at(0);
    let git = sandbox(&dir, "laptop", now).unwrap();
    let mut host = Host::over(
        "laptop",
        "willdan",
        flywheel_domain::set::load().unwrap(),
        git,
        Bindings { world: "host".into(), workspace: "recorded".into(), sessions: "operator".into() },
        Declaration { repositories: vec!["all".into()], types: vec![], kinds: vec!["all".into()] },
        now,
    );
    host.sinks.address = "http://laptop.example/willdan".into();
    flywheel::seed::from_scenario(
        &mut host.store.git,
        &workspace().join("scenarios/rail-mockup.yaml"),
        now,
        &host.declaration.clone(),
    )
    .expect("the scenario's given state");
    {
        let defs = host.defs.clone();
        flywheel_domain::commands::rail(&mut host.store, &defs).expect("the rail is numbered");
    }

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

    // The control as it rendered, with the numbers it says it will answer.
    let page = speak(
        address,
        "GET / HTTP/1.1\r\nHost: laptop.example\r\nConnection: close\r\n\r\n",
    );
    let control = page
        .split("<form ")
        .find(|block| block.contains("id=\"yesall\""))
        .expect("the header carries no `yes all` control (S2)");
    let named: Vec<u32> = control
        .split_once("name=\"numbers\" value=\"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(said, _)| said.split_whitespace().filter_map(|n| n.parse().ok()).collect())
        .expect("the control carries the numbers it will answer");
    assert!(named.len() > 2, "the mockup's rail has an approve group to sweep: {named:?}");
    assert!(named.windows(2).all(|pair| pair[0] < pair[1]), "in number order: {named:?}");

    // A decision of another kind stands and is not among them (S7).
    let standing: BTreeMap<u32, String> = {
        let mut held = host.lock().unwrap();
        let defs = held.defs.clone();
        flywheel_domain::commands::rail(&mut held.store, &defs)
            .unwrap()
            .into_iter()
            .filter_map(|d| d.number.map(|n| (n, d.group)))
            .collect()
    };
    for (number, group) in &standing {
        assert_eq!(
            named.contains(number),
            group == "approve",
            "decision {number} of the {group} group"
        );
    }

    // The tap, with the numbers it named.
    let body = format!(
        "numbers={}",
        named.iter().map(u32::to_string).collect::<Vec<_>>().join("+")
    );
    let answered = post_form(address, "/api/answer-all", &body, "/");
    assert!(
        answered.lines().next().unwrap_or_default().contains("303"),
        "a control on the page lands the operator back on a page (310, 311): {answered}"
    );

    // One response per decision, each on its own, none of them a batch (S7).
    {
        let held = host.lock().unwrap();
        let given: Vec<flywheel_engine::Object> = held
            .store
            .list_records(&Scope::Machine("response".into()))
            .unwrap();
        let answered: BTreeMap<u32, String> = given
            .iter()
            .filter_map(|o| {
                let number = o.record.get("decision")?.as_u64()? as u32;
                Some((number, o.record.get("answer")?.as_str()?.to_string()))
            })
            .collect();
        for number in &named {
            assert_eq!(
                answered.get(number).map(String::as_str),
                Some("yes"),
                "decision {number} was named and has no response of its own"
            );
        }
        for (number, _) in standing.iter().filter(|(_, group)| *group != "approve") {
            assert!(
                !answered.contains_key(number),
                "decision {number} was swept in and it is not an approve (S7)"
            );
        }
        // One response per decision the control named, and no more. The
        // scenario seeds a response of its own, so what is counted is the
        // sweep's own.
        assert_eq!(
            answered.keys().filter(|number| named.contains(number)).count(),
            named.len(),
            "one response per decision the control named, and no more"
        );
    }

    // And a number the control never named is not answered by it, however the
    // rail has moved since: what the operator saw is what they said yes to.
    let unseen = standing
        .keys()
        .find(|number| !named.contains(number))
        .copied()
        .expect("a decision of another kind stands");
    let body = format!("numbers={unseen}");
    post_form(address, "/api/answer-all", &body, "/");
    {
        let held = host.lock().unwrap();
        let answered = held
            .store
            .list_records(&Scope::Machine("response".into()))
            .unwrap()
            .into_iter()
            .any(|o| o.record.get("decision").and_then(|v| v.as_u64()) == Some(unseen as u64));
        assert!(
            !answered,
            "`yes all` answered {unseen}, which is not in the approve group (S7)"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// One message of the model context protocol, posted at the instance's address
/// the way a member's own client posts it (319).
fn client_call(address: std::net::SocketAddr, message: serde_json::Value) -> serde_json::Value {
    let body = message.to_string();
    let reply = speak(
        address,
        &format!(
            "POST /willdan HTTP/1.1\r\nHost: mac-mini.example\r\nContent-Type: application/json\r\n\
             Accept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    );
    serde_json::from_str(&reply).unwrap_or_else(|e| panic!("the reply {reply:?}: {e}"))
}

fn tool_call(id: u64, name: &str, arguments: serde_json::Value, delivery: Option<&str>) -> serde_json::Value {
    let mut params = json!({"name": name, "arguments": arguments});
    if let Some(delivery) = delivery {
        params["_meta"] = json!({"flywheel/delivery": delivery});
    }
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": params})
}

/// A host that has swept once over a bolt whose close is offered, serving its
/// page and the protocol beside the loop, with the number the decision took.
struct Serving {
    dir: std::path::PathBuf,
    host: Arc<Mutex<Host>>,
    address: std::net::SocketAddr,
    _runtime: tokio::runtime::Runtime,
    number: u32,
}

fn a_serving_host(name: &str) -> Serving {
    let dir = base(name);
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
    let defs = host.defs.clone();
    let number = flywheel_domain::commands::rail(&mut host.store, &defs)
        .unwrap()
        .iter()
        .find(|d| d.object == "bolt/atlas/plan-rows")
        .and_then(|d| d.number)
        .expect("the running host numbered the decision");
    let host = Arc::new(Mutex::new(host));
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let served = flywheel::serve::page_of(&host, 4242, &["chuck".to_string()]);
    let address = runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = flywheel_surface::http::serve_on(served, listener).await;
        });
        address
    });
    Serving { dir, host, address, _runtime: runtime, number }
}

/// What a client's call must leave as it was: every object's lease, the run
/// record a tick writes, and every object but the responses a call records.
fn untouched(host: &Host) -> (Vec<String>, usize, Vec<String>) {
    let objects: Vec<_> = host
        .store
        .list(&Scope::All)
        .unwrap()
        .objects
        .into_iter()
        .filter(|o| o.machine != "response")
        .collect();
    let leases = objects
        .iter()
        .map(|o| format!("{} {:?}", o.id, host.store.leases(&o.id).unwrap()))
        .collect();
    let run = host.store.git.run_record().unwrap().len();
    let moved = objects
        .iter()
        .map(|o| format!("{} {:?} {:?}", o.id, o.config, o.applied_responses))
        .collect();
    (leases, run, moved)
}

/// A call over the client's transport is a read under the caller's identity or
/// a response recorded, and never a tick: it takes no lease, writes nothing a
/// tick writes and moves no object; the answer it records is the loop's to
/// apply on its next tick (291, 325, 231, 137).
#[test]
fn a_call_over_the_client_transport_takes_no_lease_and_runs_no_tick() {
    let serving = a_serving_host("client-no-tick");
    let before = untouched(&serving.host.lock().unwrap());

    let hello = client_call(
        serving.address,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}}}),
    );
    assert_eq!(hello["result"]["serverInfo"]["name"], json!("flywheel"), "{hello}");
    for (id, name, arguments) in [
        (2, "rail", json!({})),
        (3, "status", json!({})),
        (4, "object", json!({"object": "bolt/atlas/plan-rows"})),
    ] {
        let read = client_call(serving.address, tool_call(id, name, arguments, None));
        assert_eq!(read["result"]["isError"], json!(false), "{read}");
    }
    let uri = flywheel_surface::catalogue::view_address("rail");
    let bundle = client_call(serving.address, json!({"jsonrpc": "2.0", "id": 5, "method": "resources/read", "params": {"uri": uri}}));
    assert!(bundle["result"]["contents"][0]["text"].is_string(), "{bundle}");
    let answered = client_call(
        serving.address,
        tool_call(6, "answer", json!({"decision": serving.number, "answer": "yes"}), Some("tap-0a1b2c3d")),
    );
    assert_eq!(answered["result"]["structuredContent"]["recorded"], json!(true), "{answered}");

    let after = untouched(&serving.host.lock().unwrap());
    assert_eq!(before.0, after.0, "a client's call took or changed a lease");
    assert_eq!(before.1, after.1, "a client's call wrote the run record, which a tick does");
    assert_eq!(before.2, after.2, "a client's call moved an object, which a tick does");

    // The answer is a response on the shared line, and the loop applies it.
    let reader = sandbox(&serving.dir, "reader", at(0)).unwrap();
    assert!(reader.get("response/client-tap-0a1b2c3d").unwrap().is_some(), "the answer is not on the shared line");
    {
        let mut held = serving.host.lock().unwrap();
        held.set_now(at(1));
        held.sweep().unwrap();
        let applied = held.store.list(&Scope::All).unwrap().objects;
        assert!(
            applied.iter().any(|o| o.applied_responses.iter().any(|r| r == "client-tap-0a1b2c3d")),
            "the loop did not apply the client's answer"
        );
    }
    let _ = std::fs::remove_dir_all(&serving.dir);
}

/// A client that disappears mid-call loses nothing: half a call is not taken, a
/// whole call whose reply nobody read is taken at most once, the host serves
/// on with no lease or run record touched, and the client coming back with the
/// same delivery has it taken once (325, 217f, 137).
#[test]
fn losing_a_client_loses_nothing() {
    use std::io::Write as _;
    let serving = a_serving_host("client-lost");
    let before = untouched(&serving.host.lock().unwrap());

    // Gone half-way through sending a call.
    {
        let mut socket = std::net::TcpStream::connect(serving.address).unwrap();
        let _ = socket.write_all(
            b"POST /willdan HTTP/1.1\r\nHost: mac-mini.example\r\nContent-Type: application/json\r\n\
              Content-Length: 400\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"answer\"",
        );
    }
    // Gone once the call was sent, before its reply came.
    let gone = tool_call(2, "answer", json!({"decision": serving.number, "answer": "yes"}), Some("tap-gone-9f8e"));
    {
        let body = gone.to_string();
        let mut socket = std::net::TcpStream::connect(serving.address).unwrap();
        let _ = socket.write_all(
            format!(
                "POST /willdan HTTP/1.1\r\nHost: mac-mini.example\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        );
    }

    // The host serves on: nothing it holds waits on a client.
    let rail = client_call(serving.address, tool_call(3, "rail", json!({}), None));
    assert_eq!(rail["result"]["isError"], json!(false), "{rail}");

    // The client comes back and sends the same delivery.
    let back = client_call(serving.address, gone);
    assert_eq!(back["result"]["isError"], json!(false), "{back}");
    let responses = serving.host.lock().unwrap().store.list(&Scope::Machine("response".into())).unwrap().objects;
    assert_eq!(responses.len(), 1, "a lost client's call was taken twice, or half a call was taken: {responses:?}");
    assert_eq!(responses[0].id, "response/client-tap-gone-9f8e");

    let after = untouched(&serving.host.lock().unwrap());
    assert_eq!(before.0, after.0, "a lost client left a lease behind");
    assert_eq!(before.1, after.1);
    assert_eq!(before.2, after.2);

    // And the loop runs on and applies the one answer, once.
    let mut held = serving.host.lock().unwrap();
    held.set_now(at(1));
    held.sweep().unwrap();
    let applied: usize = held
        .store
        .list(&Scope::All)
        .unwrap()
        .objects
        .iter()
        .map(|o| o.applied_responses.iter().filter(|r| *r == "client-tap-gone-9f8e").count())
        .sum();
    assert_eq!(applied, 1, "the lost client's answer applied {applied} times");
    drop(held);
    let _ = std::fs::remove_dir_all(&serving.dir);
}

/// Every tracked file on the state repository's shared line, by path, as a
/// reader with no host running finds it (160, 167).
fn shared_line(dir: &std::path::Path) -> BTreeMap<String, String> {
    use flywheel_world_host::git::{ls_tree, show, Repo};
    let state = Repo::at(dir.join("flywheel-state.git"));
    ls_tree(&state, "main", "")
        .expect("the shared line lists")
        .into_iter()
        .map(|path| {
            let text = show(&state, "main", &path).expect("a read").unwrap_or_default();
            (path, text)
        })
        .collect()
}

/// One message, posted as a client posts it, and the body that came back —
/// nothing, for a notification.
fn client_says(address: std::net::SocketAddr, message: &serde_json::Value) -> String {
    let body = message.to_string();
    speak(
        address,
        &format!(
            "POST /willdan HTTP/1.1\r\nHost: mac-mini.example\r\nContent-Type: application/json\r\n\
             Accept: application/json, text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    )
}

/// The conversation is the client's. After a session in which a member's
/// client said hello, carried what the member typed in messages of its own,
/// read the rail and the status view and made two calls, the state repository
/// holds those two calls and nothing of the prose, and nothing it holds was
/// derived from anything else (324, 136, 194a).
#[test]
fn the_record_after_a_client_session_holds_calls_and_nothing_else() {
    let serving = a_serving_host("client-session");
    let before = shared_line(&serving.dir);
    let prose = [
        "can you check what needs me before I head out at five",
        "the plan-rows bolt looks finished to me so land it",
        "thanks, and keep the tail one where it is until Monday",
    ];
    let session = [
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": "2025-06-18", "clientInfo": {"name": "a member's client", "version": "1"},
            "capabilities": {"extensions": {"io.modelcontextprotocol/ui": {"mimeTypes": ["text/html;profile=mcp-app"]}}}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
        // What the member typed, carried in the client's own messages, none of
        // which is a call of the catalogue.
        json!({"jsonrpc": "2.0", "id": 2, "method": "completion/complete", "params": {
            "ref": {"type": "ref/prompt", "name": "rail"}, "argument": {"name": "question", "value": prose[0]}}}),
        json!({"jsonrpc": "2.0", "method": "notifications/cancelled", "params": {"requestId": 2, "reason": prose[1]}}),
        json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}),
        json!({"jsonrpc": "2.0", "id": 4, "method": "tools/list"}),
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {
            "name": "rail", "arguments": {}, "_meta": {"conversation": prose[0]}}}),
        json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "status"}}),
        // The two calls it was asked to make, each with the member's words
        // riding alongside in the call's own metadata.
        json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {
            "name": "answer", "arguments": {"decision": serving.number, "answer": "yes"},
            "_meta": {"flywheel/delivery": "tap-session-1", "said": prose[1]}}}),
        json!({"jsonrpc": "2.0", "id": 8, "method": "tools/call", "params": {
            "name": "hold", "arguments": {"object": "bolt/atlas/plan-rows"}, "_meta": {"said": prose[2]}}}),
    ];
    for message in &session {
        let reply = client_says(serving.address, message);
        match message.get("id") {
            Some(_) => assert!(reply.contains("\"jsonrpc\""), "{message} was not answered: {reply}"),
            None => assert!(reply.trim().is_empty(), "a notification was answered: {reply}"),
        }
    }

    let after = shared_line(&serving.dir);
    for (path, text) in &after {
        for said in prose {
            assert!(!text.contains(said), "`{path}` keeps what the member said to their client: {said}");
        }
    }
    // The calls, and only the calls, are responses on record.
    let reader = sandbox(&serving.dir, "reader", at(0)).unwrap();
    let mut responses: Vec<String> = reader
        .list(&Scope::Machine("response".into()))
        .unwrap()
        .objects
        .into_iter()
        .map(|o| o.id)
        .collect();
    responses.sort();
    assert_eq!(responses, ["response/client-1", "response/client-tap-session-1"]);
    // And everything the session changed on the shared line is one of them.
    let changed: Vec<&String> = after
        .iter()
        .filter(|(path, text)| before.get(*path) != Some(*text))
        .map(|(path, _)| path)
        .collect();
    assert!(!changed.is_empty(), "the calls wrote nothing");
    for path in changed {
        let text = &after[path];
        assert!(
            path.contains("response") || text.contains("client-1") || text.contains("client-tap-session-1"),
            "`{path}` changed in a client session and is no call's record: {text}"
        );
    }
    let _ = std::fs::remove_dir_all(&serving.dir);
}
