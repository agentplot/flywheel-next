//! README.md's `Run the loop on your laptop`, played as it is written.
//!
//! The commands are read out of the README itself, so the walkthrough and what
//! runs cannot drift: a line the section gains is a line this test runs. The
//! paths are the operator's `~/flywheel` rewritten to a scratch directory and
//! `cargo run -q --` rewritten to the binary beside this test; `git` lines are
//! the operator's to run and are left alone; and the sessions binding is the
//! operator's, because this test is not in a Herdr pane and the section says
//! the operator is then the session — so the chore's commit and its report
//! are this test's, made the way an agent makes them (204, 205, 207, 222, 93b,
//! D10a, D11).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
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
    command.starts_with("git ") || command.starts_with("export ") || command.starts_with("mkdir")
}

/// The walkthrough's text with the operator's paths and binding rewritten.
fn rewritten(command: &str, home: &Path) -> String {
    command
        .replace("cargo run -q --", &binary().display().to_string())
        .replace("~/flywheel", &home.display().to_string())
        .replace("--sessions herdr", "--sessions operator")
}

/// One command of the walkthrough, with the operator's paths rewritten and
/// run from a directory: the leading assignments are the environment they are.
fn play(command: &str, home: &Path, from: &Path) -> std::process::Output {
    let text = rewritten(command, home);
    let words: Vec<String> = text.split_whitespace().map(|w| w.trim_matches('"').to_string()).collect();
    let mut env: Vec<(String, String)> = vec![("FLYWHEEL_APP_KEY".into(), "local".into())];
    let mut at = 0;
    while let Some((name, value)) = words[at].split_once('=') {
        if name.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
            env.push((name.to_string(), value.to_string()));
            at += 1;
            continue;
        }
        break;
    }
    let mut run = Command::new(&words[at]);
    run.args(&words[at + 1..]).current_dir(from);
    for (name, value) in env {
        run.env(name, value);
    }
    run.output().unwrap_or_else(|e| panic!("running `{text}`: {e}"))
}

/// The walkthrough, run as written: the instance tracking `flywheel-next` and
/// its host come into being, the layout is what the manifest says, the page
/// answers at the operator's own port, a capture typed into the box becomes a
/// unit on a bolt with one control, the session's report passes the stage,
/// the close is answered, and the bolt lands on the git host's `main`.
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
    assert!(
        commands.iter().any(|c| c.contains("--repository flywheel-next")),
        "the instance tracks this repository: {commands:?}"
    );
    let report = commands
        .iter()
        .find(|c| c.contains("exit done"))
        .expect("the walkthrough shows the session's report line")
        .clone();

    let mut served: Option<std::process::Child> = None;
    for command in &commands {
        if is_the_operators(command) || command.contains("exit done") {
            continue;
        }
        // The host with `--serve` never returns: it is the loop. It is started
        // here and stopped at the end, and the page is asked for meanwhile.
        //
        // Before it starts, the intervals are set to a test's: the poll and the
        // sweep are the backstop for what nothing notified, and this test is
        // the thing doing the notifying (D6, D7, 130, 231).
        if command.contains("--serve") {
            let path = home.join("flywheel.yaml");
            let mut manifest =
                flywheel_world_host::Manifest::read(&path).expect("the manifest the walkthrough wrote");
            manifest.intervals.poll = 0.05;
            manifest.intervals.sweep = 0.2;
            manifest.write(&path).expect("the intervals are set");
            let text = rewritten(command, &home);
            let words: Vec<&str> = text.split_whitespace().collect();
            served = Some(
                Command::new(words[0])
                    .args(&words[1..])
                    .env("FLYWHEEL_APP_KEY", "local")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .expect("the host starts"),
            );
            continue;
        }
        let out = play(command, &home, &home);
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

    // The capture, typed into the box as the section says (19, 111, 193).
    let answered = post(
        "/api/tools/capture",
        "text=README's crates table lacks a row for flywheel-workspace-host&source=page",
    );
    used(&answered);
    // It stands with its one control, and the dock's form names the capture.
    let page = wait_for("data-tool=\"propose-unit\"");
    let capture = value_of(&page, "name=\"capture\" value=\"")
        .unwrap_or_else(|| panic!("the unit form names no capture: {page}"));

    // The unit, with the bolt's name the section gives (34, 42, 44, 12).
    let bolt = "readme-crates-table";
    let proposed = post(
        "/api/tools/propose-unit",
        &format!("capture={capture}&type=chore&bolt={bolt}&repository=flywheel-next"),
    );
    used(&proposed);

    // The session is started on the next tick, with its work order in the
    // place: with the operator as the session, the place is where the work is
    // done by hand, and this test does it as an agent would — a commit on the
    // branch it is on, and the report line the order gives (67, 89, 93b).
    let place = wait_for_the_place(&home.join("hosts/laptop/agentplot/places/flywheel-next"));
    let order = std::fs::read_to_string(place.join(".flywheel/work-order.md")).expect("the order is in the place");
    assert!(order.contains("flywheel-workspace-host"), "the order carries the capture's words: {order}");
    std::fs::write(place.join("NOTES.md"), "the crates table names flywheel-workspace-host\n").expect("the work");
    for args in [
        vec!["add", "NOTES.md"],
        vec!["-c", "user.name=session", "-c", "user.email=session@localhost", "commit", "-q", "-m", "docs: the crates table names flywheel-workspace-host"],
    ] {
        let out = Command::new("git").args(&args).current_dir(&place).output().expect("git runs");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    let out = play(&report, &home, &place);
    assert!(
        out.status.success(),
        "`{report}` failed: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // The stage passes, the place merges, and the close is the decision on the
    // rail with its number (37, 39, 15).
    let object = format!("bolt/flywheel-next/{bolt}");
    let page = wait_for(&format!("class=\"card decision g-approve\" data-kind=\"decision\" data-number=\"1\" data-object=\"{object}\""));
    let number = number_of(&page, &object).unwrap_or_else(|| panic!("the close carries no number: {page}"));

    // And the answer, through the same tool a numbered chat reply calls (193).
    let answered = post("/api/tools/answer", &format!("decision={number}&answer=yes"));
    used(&answered);

    // The line lands: the git host's `main` carries the chore's commit and the
    // acceptance above it, and the lane shows the record (49, 160, 167).
    let bare = home.join("git-host/agentplot-flywheel-next.git");
    let log = wait_for_the_landing(&bare, "the crates table names flywheel-workspace-host");
    assert!(log.contains("acceptance"), "the landing writes the acceptance above the chore: {log}");
    let page = wait_for("class=\"record\"");
    assert!(page.contains("landed"), "the lane shows the landed record: {page}");
    let _ = host.kill();
    let _ = host.wait();

    // Each of them is a record on the shared line, readable with nothing
    // running, and none of them was written by hand (132, 160, 167).
    let state = home.join("hosts/laptop/agentplot/flywheel-state");
    for under in ["capture", "signal", "unit", "bolt", "work-item", "response"] {
        let held = std::fs::read_dir(state.join("objects").join(under))
            .unwrap_or_else(|e| panic!("`objects/{under}` on the shared line: {e}"))
            .count();
        assert!(held >= 1, "nothing under `objects/{under}` on the shared line");
    }
    let _ = std::fs::remove_dir_all(&home);
}

/// The first attribute value after a marker on the page.
fn value_of(page: &str, marker: &str) -> Option<String> {
    let at = page.find(marker)? + marker.len();
    let rest = &page[at..];
    Some(rest[..rest.find('"')?].to_string())
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

/// The one place under a repository's places, once the host has made it and
/// written the work order into it (52, 89).
fn wait_for_the_place(places: &Path) -> PathBuf {
    for _ in 0..600 {
        if let Ok(read) = std::fs::read_dir(places) {
            for entry in read.flatten() {
                let dir = entry.path();
                if dir.join(".flywheel/work-order.md").is_file() {
                    return dir;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("no place with a work order appeared under {}", places.display());
}

/// The git host's `main`, once it carries a commit with the given subject.
fn wait_for_the_landing(bare: &Path, subject: &str) -> String {
    let mut log = String::new();
    for _ in 0..600 {
        if let Ok(out) = Command::new("git").args(["log", "--oneline", "main", "-5"]).current_dir(bare).output() {
            log = String::from_utf8_lossy(&out.stdout).to_string();
            if log.contains(subject) {
                return log;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("`main` on the git host never carried `{subject}`: {log}");
}

/// The page, once it says what the walkthrough's next step needs it to. A tick
/// is what moves the state, so the page is asked again until it has.
fn wait_for(shown: &str) -> String {
    for _ in 0..600 {
        if let Ok(text) = speak("GET / HTTP/1.1\r\nHost: localhost:4242\r\nConnection: close\r\n\r\n")
        {
            if text.contains(shown) {
                return text;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("the page never showed `{shown}`");
}

/// The page, once the host is serving it. A host makes its repositories and its
/// first tick before it listens, so the first request may be early.
fn wait_for_the_page() -> String {
    for _ in 0..600 {
        if let Ok(text) = speak("GET / HTTP/1.1\r\nHost: localhost:4242\r\nConnection: close\r\n\r\n") {
            if text.contains("<!doctype html>") {
                return text;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("the page never answered at the operator's own port");
}

/// A control on the page, submitted the way a browser submits one: a form
/// body, with the whole reply kept, because what a control answers is a status
/// and a place to go and not a document (310, 311).
fn post(path: &str, body: &str) -> String {
    let mut socket = std::net::TcpStream::connect("127.0.0.1:4242").expect("the page answers");
    socket
        .write_all(
            format!(
                "POST {path} HTTP/1.1\r\nHost: localhost:4242\r\n\
                 Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .expect("the request is sent");
    let mut text = String::new();
    socket.read_to_string(&mut text).expect("the page replies");
    text
}

/// A control used: the operator is returned to the page they were on, and the
/// page they land on carries no refusal (310). What the control did is read
/// from the page and the state afterwards, never from the reply.
fn used(answered: &str) {
    assert!(
        answered.starts_with("HTTP/1.1 303 See Other"),
        "a control returns the operator to the page (310): {answered}"
    );
    assert!(!answered.contains("?refused="), "the control was refused: {answered}");
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
