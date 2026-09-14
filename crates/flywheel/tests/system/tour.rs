//! Stepping a scenario from the page (`design/flywheel-next/scenarios/
//! storefront.md`).
//!
//! Third tier by subject and by what it needs: a real instance made by
//! `flywheel init`, a real host serving over it, a real state repository, and
//! a session's exit going through the real binary. What it proves is the
//! product's own onboarding — the operator clicks and the board moves — and
//! not one crate's code (D17, 93).

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// The `flywheel` binary beside the test binary: a session's exit is reported
/// through the command, and a test is not it (D8, 93).
fn binary() -> PathBuf {
    let deps = std::env::current_exe().expect("the test binary has a path");
    let target = deps.parent().and_then(|p| p.parent()).expect("target/debug");
    target.join("flywheel")
}

fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "flywheel-tour-{name}-{}-{:?}",
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
    text
}

fn get(address: std::net::SocketAddr) -> String {
    speak(
        address,
        "GET / HTTP/1.1\r\nHost: local.example\r\nConnection: close\r\n\r\n",
    )
}

/// The overlay's strip, as the page rendered it, or nothing where there is
/// none.
fn overlay(page: &str) -> Option<String> {
    let at = page.find("<div class=\"tour")?;
    let rest = &page[at..];
    let end = rest.find("</div>").map(|e| e + "</div>".len())?;
    Some(rest[..end].to_string())
}

/// An instance a scenario was applied into onboards whoever is looking at it,
/// one action at a time, and stops being a tour when the last action is
/// played.
///
/// The overlay is the product's own and not a mode: what puts it on the page
/// is a fact in the state store, so this asserts the whole of the rule — it is
/// there while an action is left, it is gone when none is, and the click that
/// advances it is a form the page carries with no script behind it (310, 311).
#[test]
fn the_page_steps_the_scenario_it_stands_in() {
    std::env::set_var(flywheel_scenario::sessions::BINARY_ENV, binary());
    let into = dir("steps");
    let scenario = root().join("scenarios/storefront");

    // The instance, made the way an operator makes one and left standing
    // before the first action: nothing has happened in it yet.
    let applied = flywheel::apply::apply(
        &scenario,
        &into,
        Some(0),
        chrono::Utc::now(),
        chrono::Duration::seconds(flywheel::apply::PER_ACTION),
    )
    .expect("the scenario is applied into a fresh instance");
    assert_eq!(applied.through, 0);
    let actions = applied.actions;
    assert!(actions >= 15, "the storefront scenario carries its arc");

    let mut host = flywheel::host::Host::open(
        &applied.manifest,
        "local",
        None,
        chrono::Utc::now(),
    )
    .expect("the host opens over the instance the apply made");
    host.sinks.address = "http://local.example/storefront".into();
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

    // Before anything: the overlay is there, because an action is left.
    let strip = overlay(&get(address)).expect("an instance with actions left carries the overlay");
    assert!(strip.contains(&format!("of <b>{actions}</b>")), "{strip}");
    assert!(
        strip.contains("action=\"/tour/next\""),
        "the control is a form the page carries, with no script behind it (310, 311): {strip}"
    );

    // Every action, one click each. The click writes that the action is owed;
    // the loop plays it, which is what this stands in for — one pass, exactly
    // as the running host takes one.
    let post = "POST /tour/next HTTP/1.1\r\nHost: local.example\r\n\
                Content-Length: 0\r\nConnection: close\r\n\r\n";
    for step in 1..=actions {
        // Whether this one holds a beat: a session's delivery does and
        // nothing else does, because the machinery has already stalled where
        // the session would be working.
        let a_session = {
            let held = host.lock().unwrap();
            flywheel_domain::tour::read(&held.store.git, "storefront")
                .expect("the fact the overlay is read from")
                .next_is_a_session
        };
        let answer = speak(address, post);
        assert!(
            answer.starts_with("HTTP/1.1 303"),
            "a click is answered with somewhere to go, never a body (310): {answer}"
        );
        // The click writes that the action is owed and plays nothing itself:
        // the request the operator waits on never runs a cascade (D11).
        {
            let held = host.lock().unwrap();
            let tour = flywheel_domain::tour::read(&held.store.git, "storefront").unwrap();
            assert_eq!(tour.played, step - 1, "nothing is played by the click itself");
            assert!(tour.working(), "the action the operator asked for is owed");
        }
        let page = get(address);
        let strip = overlay(&page).expect("the overlay is there while an action is left");
        assert!(
            strip.contains("the agent is working"),
            "what is owed is shown as owed: {strip}"
        );
        assert!(
            page.contains("http-equiv=\"refresh\""),
            "the document asks for itself again when the beat is over (310)"
        );

        // The machinery plays it, the way the loop does: one pass, at the
        // moment the beat is out. A session's is held for the beat and
        // nothing else is.
        let played = {
            let mut held = host.lock().unwrap();
            let owed = flywheel_domain::tour::read(&held.store.git, "storefront")
                .unwrap()
                .due_at
                .expect("the moment the operator asked for");
            if a_session {
                held.set_now(owed - chrono::Duration::milliseconds(1));
                let early = held.now();
                assert!(
                    !flywheel::tour::play_due(&mut held, early).expect("nothing yet"),
                    "a session's delivery is held for the beat before it appears"
                );
            }
            held.set_now(owed);
            flywheel::tour::play_due(&mut held, owed).expect("the action plays")
        };
        assert!(played, "the loop played the action the operator asked for");

        let held = host.lock().unwrap();
        let tour = flywheel_domain::tour::read(&held.store.git, "storefront").unwrap();
        assert_eq!(tour.played, step);
        assert!(!tour.working(), "nothing is owed once it is played");
    }

    // Played out: there is nothing left to say, so there is no overlay. The
    // page is the page, which is what every instance an operator made for
    // themselves has always had.
    let page = get(address);
    assert_eq!(overlay(&page), None, "the tour is over and the strip is gone");
    assert!(
        !page.contains("http-equiv=\"refresh\""),
        "and nothing is asking for itself again"
    );
    // It is the same instance a whole apply reaches: the operator's clicks, one
    // per action, and `--through` at the last stand at the same moment.
    assert!(
        page.contains("elaboration/storefront-declines/finding-1"),
        "the prototype the tour built is on the board"
    );
}
