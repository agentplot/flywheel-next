//! README.md's `Run the loop on your laptop`, played as it is written.
//!
//! The commands are read out of the README itself, so the walkthrough and what
//! runs cannot drift: a line the section gains is a line this test runs. The
//! paths are the operator's `~/flywheel` rewritten to a scratch directory and
//! `cargo run -q --` rewritten to the binary beside this test; nothing else is
//! changed, and `git` lines are the operator's to run and are left alone
//! (204, 205, 207, 222, D10a, D11).

use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Command;

/// The `flywheel` binary beside the test binary.
fn binary() -> PathBuf {
    let deps = std::env::current_exe().expect("the test binary has a path");
    let target = deps.parent().and_then(|p| p.parent()).expect("target/debug");
    target.join("flywheel")
}

fn readme() -> String {
    let path = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../README.md"));
    std::fs::read_to_string(&path).expect("the README is readable")
}

/// Every command of the walkthrough's shell blocks, joined over its line
/// continuations, in the order the section gives them.
fn commands(readme: &str) -> Vec<String> {
    let section = readme
        .split_once("## Run the loop on your laptop")
        .expect("the README carries the walkthrough")
        .1
        .split_once("## The conformance suite")
        .expect("the walkthrough ends where the suite begins")
        .0;
    let mut out: Vec<String> = Vec::new();
    let mut inside = false;
    let mut held = String::new();
    for line in section.lines() {
        if line.starts_with("```") {
            inside = line.starts_with("```sh");
            continue;
        }
        if !inside {
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        held.push(' ');
        held.push_str(trimmed.trim_end_matches('\\'));
        if !trimmed.ends_with('\\') {
            out.push(held.trim().to_string());
            held.clear();
        }
    }
    out
}

/// What the walkthrough asks the operator to run themselves, which this test
/// does not: git is theirs, and the export is the environment it sets below.
fn is_the_operators(command: &str) -> bool {
    command.starts_with("git ") || command.starts_with("export ")
}

/// One command of the walkthrough, with the operator's paths rewritten.
fn play(command: &str, home: &PathBuf, session_state: &str) -> std::process::Output {
    let text = command
        .replace("cargo run -q --", &binary().display().to_string())
        .replace("~/flywheel", &home.display().to_string())
        .replace("http://your-laptop.local", "http://laptop.example");
    // The words as the shell would split them, with the leading assignments
    // taken as the environment they are.
    let words: Vec<String> = text.split_whitespace().map(|w| w.trim_matches('"').to_string()).collect();
    let mut env: Vec<(String, String)> = vec![("FLYWHEEL_APP_KEY".into(), "local".into())];
    let mut at = 0;
    while let Some((name, value)) = words[at].split_once('=') {
        if name.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
            env.push((name.to_string(), value.replace("$STATE", session_state)));
            at += 1;
            continue;
        }
        break;
    }
    let mut run = Command::new(&words[at]);
    run.args(&words[at + 1..]);
    for (name, value) in env {
        run.env(name, value);
    }
    run.output().unwrap_or_else(|e| panic!("running `{text}`: {e}"))
}

/// The walkthrough, run as written: the instance and its host come into being,
/// the layout is what the manifest says, the page answers at the operator's own
/// port, and a capture typed into the box is a record the status view shows.
#[test]
fn readme_walkthrough_runs() {
    let home = std::env::temp_dir().join(format!(
        "flywheel-walkthrough-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(home.join("git-host")).expect("the git host is a directory");

    let readme = readme();
    let commands = commands(&readme);
    assert!(
        commands.iter().any(|c| c.contains("init ")),
        "the walkthrough begins with init: {commands:?}"
    );
    let state = home
        .join("hosts/laptop/willdan/flywheel-state")
        .display()
        .to_string();

    let mut served: Option<std::process::Child> = None;
    for command in &commands {
        if is_the_operators(command) || command.starts_with("mkdir") {
            continue;
        }
        // The host with `--serve` never returns: it is the loop. It is started
        // here and stopped at the end, and the page is asked for meanwhile.
        if command.contains("--serve") {
            let text = command
                .replace("cargo run -q --", &binary().display().to_string())
                .replace("~/flywheel", &home.display().to_string());
            let words: Vec<&str> = text.split_whitespace().collect();
            served = Some(
                Command::new(words[0])
                    .args(&words[1..])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .expect("the host starts"),
            );
            continue;
        }
        // The report needs a session the run made, which the walkthrough names
        // by the id the status view showed; a run of this test has its own.
        if command.contains("exit done") {
            continue;
        }
        let out = play(command, &home, &state);
        assert!(
            out.status.success(),
            "`{command}` failed: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let mut host = served.expect("the walkthrough serves the page");
    let page = wait_for_the_page();
    assert!(
        page.contains("id=\"capture-box\""),
        "the page the walkthrough opens carries the capture box: {page}"
    );
    // The captures, typed into the box as the section says. A dozen, which is
    // the shipped threshold curation is charged at (110).
    for n in 1..=12 {
        let answered = post(
            "/api/tools/capture",
            &format!("text=the rows lose their numbers on page {n}&source=page"),
        );
        assert!(answered.contains("\"recorded\":true"), "{answered}");
    }

    // The curator's surface, once the tick has charged curation (110, 93b).
    let page = wait_for("id=\"curate-box\"");
    let signals = signals_on(&page);
    assert!(
        signals.len() >= 2,
        "the curator's surface shows the unmoved signals: {page}"
    );
    for word in ["attach", "join", "route", "challenge", "drop"] {
        assert!(
            page.contains(&format!("<option value=\"{word}\">{word}</option>")),
            "the surface offers no `{word}` (107, 116)"
        );
    }

    // Two of them joined into one intent, submitted: the session's delivery and
    // its exit in one submit (67, 93b, 110).
    let intent = "intent/rows-lose-numbers";
    let body = signals[..2]
        .iter()
        .map(|s| format!("move.{s}=join&target.{s}={intent}"))
        .collect::<Vec<_>>()
        .join("&");
    let curated = post("/api/curate", &body);
    assert!(curated.contains("\"recorded\":true"), "{curated}");

    // The decision the joins raise, on the served rail with its number (15,
    // 109, 110).
    let page = wait_for(&format!("data-object=\"{intent}\""));
    let number = number_of(&page, intent)
        .unwrap_or_else(|| panic!("the proposed intent carries no number: {page}"));

    // And the answer, through the same tool a numbered chat reply calls (193).
    let answered = post("/api/tools/answer", &format!("decision={number}&answer=yes"));
    assert!(answered.contains("\"recorded\":true"), "{answered}");
    let _ = host.kill();
    let _ = host.wait();

    // Each of them is a record on the shared line, readable with nothing
    // running, and none of them was written by hand (132, 160, 167).
    for under in ["capture", "signal", "intent", "response"] {
        let held = std::fs::read_dir(PathBuf::from(&state).join("objects").join(under))
            .unwrap_or_else(|e| panic!("`objects/{under}` on the shared line: {e}"))
            .count();
        assert!(held >= 1, "nothing under `objects/{under}` on the shared line");
    }
    let _ = std::fs::remove_dir_all(&home);
}

/// The signal ids the curator's surface offers a move on.
fn signals_on(page: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for at in page.match_indices("data-signal=\"") {
        let rest = &page[at.0 + 13..];
        let Some(end) = rest.find('"') else { continue };
        let id = rest[..end].to_string();
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// The number the register gave the decision standing on one object (15).
fn number_of(page: &str, object: &str) -> Option<u32> {
    let card = page
        .split("<article class=\"card decision")
        .find(|card| card.contains(&format!("data-object=\"{object}\"")))?;
    let at = card.find("data-number=\"")? + 13;
    let rest = &card[at..];
    rest[..rest.find('"')?].parse().ok()
}

/// The page, once it says what the walkthrough's next step needs it to. A tick
/// is what moves the state, so the page is asked again until it has.
fn wait_for(shown: &str) -> String {
    for _ in 0..120 {
        if let Ok(text) = speak("GET / HTTP/1.1\r\nHost: localhost:4242\r\nConnection: close\r\n\r\n")
        {
            if text.contains(shown) {
                return text;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    panic!("the page never showed `{shown}`");
}

/// The page, once the host is serving it. A host makes its repositories and its
/// first tick before it listens, so the first request may be early.
fn wait_for_the_page() -> String {
    for _ in 0..120 {
        if let Ok(text) = speak("GET / HTTP/1.1\r\nHost: localhost:4242\r\nConnection: close\r\n\r\n") {
            if text.contains("<!doctype html>") {
                return text;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    panic!("the page never answered at the operator's own port");
}

fn post(path: &str, body: &str) -> String {
    speak(&format!(
        "POST {path} HTTP/1.1\r\nHost: localhost:4242\r\n\
         Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n{body}",
        body.len()
    ))
    .expect("the page takes the call")
}

fn speak(request: &str) -> std::io::Result<String> {
    let mut socket = std::net::TcpStream::connect("127.0.0.1:4242")?;
    socket.write_all(request.as_bytes())?;
    let mut text = String::new();
    socket.read_to_string(&mut text)?;
    Ok(text
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or(text))
}
