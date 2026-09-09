//! The phone pass: the page as the operator meets it, at 390px (314).
//!
//! The page is served from the process that just ticked, so what the driver
//! reads is what the tick wrote (310); the browser is a test dependency of this
//! crate and of no shipped crate (D15).

mod driver;
mod phase;

use flywheel_scenario::conformance::{drive, Suite};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// A run with a decision standing, and the number the register gave it.
fn a_standing_decision() -> (flywheel_scenario::Runtime, u32) {
    let path = root().join("conformance/scenarios/S01.yaml");
    let (scenario, _) = flywheel_atoms::conformance::load(&path).expect("S01 loads");
    let suite = Suite::for_scenario(&path).expect("the suite opens");
    let defs = flywheel_engine::load::load_dir(&root().join("definitions")).expect("the machines load");
    let mut runtime = drive::seed(defs, &scenario, &suite).expect("S01 seeds");
    // One tick derives the elaboration's decision and the register numbers it;
    // the scenario's own response step is not played, so it still stands.
    runtime.tick();
    let number = runtime
        .decisions()
        .iter()
        .find_map(|d| d.number)
        .expect("a decision stands after the first tick");
    (runtime, number)
}

/// The driver opens the rail at 390×844 and taps an answer: the control is
/// where a finger can reach it, the tap goes through the tool catalogue, and the
/// decision it answered is gone from the page that follows (311, 193, 314).
#[test]
fn driver_taps_at_390px() {
    if !driver::available() {
        eprintln!(
            "skipped: no browser to drive. The 390px pass needs a Chromium or Chrome installed \
             (D15); the driver is a test dependency and downloads nothing."
        );
        return;
    }
    let (runtime, number) = a_standing_decision();
    let served = driver::Served::page(runtime.store, runtime.defs, "chuck")
        .expect("the page is served from the process that just ticked");
    let driver = driver::Driver::open().expect("the headless browser starts");

    let tab = driver
        .visit(&served.url, driver::PHONE)
        .expect("the rail opens at the phone's viewport");
    assert_eq!(
        driver::viewport_width(tab).expect("the document reports its width"),
        driver::PHONE.0 as i64,
        "the page laid itself out at 390px, which is what the bundle's media query reads (307)"
    );

    // The decision is found by the number the register gave it (15).
    let card = format!("article.decision[data-number=\"{number}\"]");
    let answer = format!("{card} button[data-answer=\"yes\"]");
    let measured = driver::measure(tab, &answer)
        .expect("the control is measured")
        .expect("the answer control is on the page");
    assert!(
        measured.tappable(),
        "the answer is reachable by tap at 390px, with nothing behind a hover or a keyboard \
         (311): {measured:?}"
    );

    let holding = tab.get_url();
    tab.wait_for_element(&answer)
        .expect("the answer control is on the page")
        .click()
        .expect("the control takes a tap");
    let landed = driver::wait_for_url(tab, &holding).expect("the tap is posted");
    assert!(
        landed.ends_with("/api/tools/answer"),
        "the tap posts to the tool catalogue, which is the one write path (193): {landed}"
    );
    let recorded = tab.get_content().expect("what the tool answered");
    assert!(
        recorded.contains("recorded"),
        "the call was recorded once, as the reply grammar's would be (153, 193): {recorded}"
    );

    // And a reload is the page again, rendered from the same read: the bundle
    // holds no client state a reload loses (310). What the response does to the
    // decision is the next tick's, not the page's — the page records it and the
    // machinery applies it (129, 137).
    let tab = driver
        .visit(&served.url, driver::PHONE)
        .expect("the rail opens again");
    let after = tab.get_content().expect("the page after the answer");
    assert!(
        after.contains(&format!("data-number=\"{number}\"")),
        "the page is rendered from state on every request and nothing was lost"
    );
    assert!(
        driver::measure(tab, &answer)
            .expect("the control is measured")
            .is_some_and(|m| m.tappable()),
        "and the control is where it was"
    );
}


// ---- 11.8 the pass over every scenario carrying a response

/// The `flywheel` binary beside the test binary; a scenario whose steps report
/// through the command needs it, and a test binary is not it (D8).
fn flywheel_binary_beside_the_test() -> PathBuf {
    let deps = std::env::current_exe().expect("the test binary has a path");
    let target = deps.parent().and_then(|p| p.parent()).expect("target/debug");
    let binary = target.join("flywheel");
    if !binary.exists() {
        let out = std::process::Command::new(env!("CARGO"))
            .args(["build", "--quiet", "-p", "flywheel"])
            .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .output()
            .expect("building the flywheel binary");
        assert!(
            binary.exists(),
            "no flywheel binary at {}: {}",
            binary.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    binary
}

/// The set is read off the scenarios and no list of it is kept by hand: a
/// scenario that carries a response is in the pass, one that does not is out,
/// and adding a response step to a scenario adds it (314, D15).
#[test]
fn phone_set_is_selected_not_listed() {
    let dir = root().join("conformance/scenarios");
    let selected: Vec<String> = flywheel_scenario::conformance::phone_set(&dir)
        .expect("the set is read off the files")
        .iter()
        .map(|p| p.file_stem().expect("a name").to_string_lossy().to_string())
        .collect();
    assert!(!selected.is_empty(), "some scenario carries a response");

    // Every scenario, and whether the data says it carries a response: the set
    // is exactly those and nothing is named anywhere.
    for entry in std::fs::read_dir(&dir).expect("the scenarios are readable").flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|x| x != "yaml") {
            continue;
        }
        let (scenario, _) = flywheel_atoms::conformance::load(&path).expect("a scenario loads");
        let name = path.file_stem().expect("a name").to_string_lossy().to_string();
        assert_eq!(
            selected.contains(&name),
            scenario.carries_response(),
            "`{name}` is in the 390px set exactly when it carries a response"
        );
    }

    // And the rule reads the data rather than the name: a scenario with no
    // response step joins the set the moment it gains one.
    let (mut without, _) = flywheel_atoms::conformance::load(&dir.join("S16.yaml"))
        .expect("S16 loads");
    without.when.retain(|step| !step.contains_key("response"));
    assert!(!without.carries_response());
    without
        .when
        .push([("response".to_string(), serde_json::json!({"number": 1}))].into_iter().collect());
    assert!(without.carries_response());
}

/// One scenario's pass at one viewport: play it up to the response it carries,
/// serve the page from the process that just ticked, and give that response
/// through the page instead (314).
fn pass(path: &std::path::Path, viewport: (u32, u32)) -> Result<(), String> {
    let (mut scenario, _) =
        flywheel_atoms::conformance::load(path).map_err(|e| format!("loading: {e:#}"))?;
    let at = scenario
        .when
        .iter()
        .position(|step| step.contains_key("response"))
        .expect("the set holds only scenarios carrying a response");
    let step: flywheel_atoms::conformance::ResponseStep =
        serde_json::from_value(scenario.when[at]["response"].clone())
            .map_err(|e| format!("the response step: {e}"))?;
    // Everything before the response, and then the response given through the
    // page rather than through the runner (193, 314).
    scenario.when.truncate(at);
    let suite = Suite::for_scenario(path).map_err(|e| format!("the suite: {e:#}"))?;
    let options = flywheel_scenario::conformance::RunOptions {
        definitions: Some(root().join("definitions")),
        ..Default::default()
    };
    let mut run = drive::play(&scenario, path, &suite, &options)
        .map_err(|e| format!("playing the steps before the response: {e:#}"))?;

    // The decision the step names, found by the number the register gave it
    // (15). A scenario answering by number alone names it directly.
    let standing = run.runtime.decisions();
    let found = match (&step.decision, step.number) {
        (Some(id), _) => run.runtime.standing_decision(id),
        (None, Some(number)) => standing.iter().find(|d| d.number == Some(number)).cloned(),
        _ => None,
    };
    let Some(decision) = found else {
        return Err(format!(
            "the decision `{:?}` the response names stands nowhere after step {at}; standing: {:?}",
            step.decision,
            standing.iter().map(|d| &d.object).collect::<Vec<_>>()
        ));
    };
    let number = decision
        .number
        .ok_or_else(|| "the decision stands but the register gave it no number".to_string())?;

    let served = driver::Served::page(run.runtime.store, run.runtime.defs, "chuck")
        .map_err(|e| format!("serving the page: {e:#}"))?;
    let driver = driver::Driver::open().map_err(|e| format!("the browser: {e:#}"))?;
    let tab = driver
        .visit(&served.url, viewport)
        .map_err(|e| format!("opening the rail: {e:#}"))?;

    // The number, and every answer, reachable by tap with nothing behind a
    // hover or a keyboard (311). No scenario needs a selector of its own: the
    // number the register gave is what finds the card.
    let card = format!("article.decision[data-number=\"{number}\"]");
    if driver::measure(tab, &format!("{card} .number")).map_err(|e| e.to_string())?.is_none() {
        return Err(format!("no decision numbered {number} on the page"));
    }
    for answer in &decision.answers {
        let selector = format!("{card} button[data-answer=\"{}\"]", escape(answer));
        let measured = driver::measure(tab, &selector)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("no control for answer `{answer}`"))?;
        if !measured.tappable() {
            return Err(format!(
                "the answer `{answer}` is not reachable by tap at {}px: {measured:?}",
                viewport.0
            ));
        }
    }

    // Give the scenario's own answer through the page. It posts to the one tool
    // the chat's numbered reply grammar calls (193, 194).
    let taken = decision
        .answers
        .iter()
        .find(|a| *a == &step.answer)
        .or_else(|| decision.answers.first())
        .ok_or_else(|| "the decision offers no answer".to_string())?;
    let selector = format!("{card} button[data-answer=\"{}\"]", escape(taken));
    let holding = tab.get_url();
    tab.wait_for_element(&selector)
        .map_err(|e| format!("the control: {e:#}"))?
        .click()
        .map_err(|e| format!("the tap: {e:#}"))?;
    let landed = driver::wait_for_url(tab, &holding).map_err(|e| e.to_string())?;
    let expected = format!("/api/tools/{}", flywheel_surface::catalogue::ANSWER);
    if !landed.ends_with(&expected) {
        return Err(format!("the tap posted to {landed}, not to {expected} (193)"));
    }
    let answered = tab.get_content().map_err(|e| e.to_string())?;
    if !answered.contains("recorded") {
        return Err(format!("the call was not recorded: {answered}"));
    }

    // And a reload shows it recorded, with who gave it and when (153, 154, 310).
    let tab = driver
        .visit(&served.url, viewport)
        .map_err(|e| format!("reloading: {e:#}"))?;
    let after = tab.get_content().map_err(|e| e.to_string())?;
    for shown in ["class=\"answered\"", "data-given-by=\"chuck\"", "data-given-at=\""] {
        if !after.contains(shown) {
            return Err(format!("a reload does not show `{shown}` (153, 154, 310)"));
        }
    }
    Ok(())
}

/// An attribute value as a CSS selector carries it. The page writes the answer
/// escaped for HTML and the parser gives it back whole, so what a selector
/// matches is the answer itself; only the quote that ends the selector's own
/// string needs escaping.
fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Every scenario this phase accepts that carries a response runs at the
/// phone's viewport and again at the desktop's, and passes the same three
/// assertions at both (314, 311, 193, 310, 153).
#[test]
fn phone_pass_for_every_response_scenario() {
    if !driver::available() {
        eprintln!(
            "skipped: no browser to drive. The 390px pass needs a Chromium or Chrome installed \
             (D15); the driver is a test dependency and downloads nothing."
        );
        return;
    }
    std::env::set_var(
        flywheel_scenario::sessions::BINARY_ENV,
        flywheel_binary_beside_the_test(),
    );
    let dir = root().join("conformance/scenarios");
    let selected = flywheel_scenario::conformance::phone_set(&dir).expect("the set");

    // The pass runs over the set this phase accepts. A scenario the phase does
    // not accept — deferred, or needing a real workspace — is not run here for
    // the same reason it is not run at all (93a, proposal.md — Deferred).
    let mut ran: Vec<String> = Vec::new();
    let mut failures: Vec<String> = Vec::new();
    for path in &selected {
        let name = path.file_stem().expect("a name").to_string_lossy().to_string();
        if !phase::ACCEPTED.contains(&name.as_str()) {
            continue;
        }
        for viewport in [driver::PHONE, driver::DESKTOP] {
            match pass(path, viewport) {
                Ok(()) => {}
                Err(why) => failures.push(format!("{name} at {}px · {why}", viewport.0)),
            }
        }
        ran.push(name);
    }
    assert!(
        failures.is_empty(),
        "the 390px pass failed:\n{}",
        failures.join("\n")
    );
    // Every accepted scenario carrying a response ran, so none was quietly
    // left out of the pass (314).
    let expected: Vec<String> = phase::ACCEPTED
        .iter()
        .filter(|name| {
            flywheel_atoms::conformance::load(&dir.join(format!("{name}.yaml")))
                .map(|(s, _)| s.carries_response())
                .unwrap_or(false)
        })
        .map(|name| name.to_string())
        .collect();
    assert_eq!(ran, expected, "the pass ran every accepted scenario carrying a response");
    assert!(!ran.is_empty());
}
