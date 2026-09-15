//! The page within its budget: an instance holding the round's counts and one
//! holding ten times each, seeded through the scenario tool, served, and loaded
//! in the headless browser throttled to a mid-range phone over an ordinary
//! mobile connection (310a, S235, 95, `surfaces.yaml` budget, D15, D17).
//!
//! What is asserted is what the browser received and drew, read from the page's
//! own timing entries: when it first painted, when the rail's first card took a
//! press, every response's size as it came over the wire, and the first view's
//! HTML before compression. The numbers are 310a's, and a page whose cost grows
//! with the instance rather than with what is on screen fails at the larger
//! size.

use super::driver;

use flywheel_domain::signals::{self, Capture, Signal};
use flywheel_scenario::bindings::FilesWorld;
use flywheel_scenario::conformance::{drive, Suite};
use headless_chrome::protocol::cdp::{Emulation, Input, Network};
use headless_chrome::Tab;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Duration;

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// What an instance holds of the four things 310a scales.
#[derive(Debug, Clone, Copy)]
struct Counts {
    signals: usize,
    captures: usize,
    intents: usize,
    facts: usize,
}

impl Counts {
    fn times(self, n: usize) -> Counts {
        Counts {
            signals: self.signals * n,
            captures: self.captures * n,
            intents: self.intents * n,
            facts: self.facts * n,
        }
    }
}

/// The round's scratch instance as the page was measured on it (blueprints
/// gaps 110, 310a): the willdan signals read in as fourteen captures, a
/// transcript read, three findings and a typed note — 264 signals curation had
/// not read, over nineteen captures — the intents the round proposed and
/// closed, and the facts its sessions left.
const ROUND: Counts = Counts { signals: 264, captures: 19, intents: 8, facts: 60 };

/// What stands on the first view at both sizes: the intents proposed, each a
/// decision on the rail, which is never paged (15, S235), and the notes the
/// Inception lane draws. Ten times the instance is the same first view over ten
/// times the material, so what the larger instance adds is the material a first
/// view has no call to draw: signals waiting in the tray and the captures
/// holding them, the sessions' facts, and intents closed before today (310a).
const PROPOSED: usize = 4;
const NOTES: usize = 2;

/// 310a's numbers.
const FIRST_PAINT_MS: f64 = 1000.0;
const FIRST_PRESS_MS: f64 = 1500.0;
const LOAD_BYTES: u64 = 200_000;
const CACHED_LOAD_BYTES: u64 = 60_000;
const FIRST_VIEW_HTML_BYTES: u64 = 100_000;
const UPDATE_BYTES: u64 = 8_000;

/// How many times each load is made, the paint and the press taken as the
/// median: a throttled browser's timings move by tens of milliseconds between
/// two loads of one page, and a tenth is what growth is held to.
const LOADS: usize = 3;

/// How long a throttled load, a dock page or an update is waited for. A page
/// many times over its budget takes minutes on this connection, and what it
/// measures is the finding, so the wait is long and never the budget.
const WAIT: Duration = Duration::from_secs(300);

/// The instance described as the scenario tool describes one (95): its objects,
/// and the signal material the blueprints hold under the machinery's prefix
/// (111, 203).
fn described(counts: Counts) -> (flywheel_atoms::conformance::Scenario, FilesWorld) {
    let mut world = FilesWorld::new();
    let mut objects: Vec<Value> = vec![json!({
        "id": "curation/willdan", "machine": "curation",
        "state": {"run": "idle"}, "record": {"threshold": 12}
    })];
    let meetings = counts.captures - NOTES;
    let read_in = counts.signals - NOTES;
    let mut cited: Vec<String> = Vec::new();
    for n in 0..counts.captures {
        let note = n >= meetings;
        let key = match note {
            false => format!("meeting/2026-08-{:02}/willdan-weekly-{n}", 1 + n % 28),
            true => format!("console/2026-09-01/note-{n}"),
        };
        let capture = Capture {
            key: key.clone(),
            source: if note { "console" } else { "meeting" }.to_string(),
            event_at: format!("2026-08-{:02}", 1 + n % 28),
            captured_by: "chuck".into(),
            raw: if note { String::new() } else { format!("file:///raw/{key}.vtt") },
        };
        signals::write_capture(&mut world, &capture).expect("the capture's record");
        let capture_id = signals::object_of(&key);
        objects.push(json!({
            "id": capture_id, "machine": "capture", "state": {"reading": "read"},
            "record": {
                "source": capture.source, "event_key": key, "event_at": capture.event_at,
                "captured_by": capture.captured_by, "raw": capture.raw
            }
        }));
        let held = match note {
            true => 1,
            false => read_in / meetings + usize::from(n < read_in % meetings),
        };
        for ordinal in 1..=held as u64 {
            let signal = Signal {
                id: signals::signal_object(&key, ordinal),
                capture: capture_id.clone(),
                kind: ["ask", "constraint", "question", "reaction"][ordinal as usize % 4].into(),
                asserted_by: ["dana", "chuck", "priya"][ordinal as usize % 3].into(),
                subject_tags: vec!["checkout".into(), "payments".into()],
                assertion: format!(
                    "The retry schedule for declined cards should back off rather than try again at \
                     once, point {ordinal} of {key}"
                ),
                excerpt: "so what we saw on Friday is the card tried twice in a second and the bank \
                          flagged it, and then nothing worked for the rest of the day"
                    .into(),
                position: format!("00:{:02}:{:02}", ordinal % 60, (ordinal * 7) % 60),
                argues_with: vec![],
            };
            signals::write_signal(&mut world, &key, ordinal, &signal).expect("the signal's record");
            if cited.len() < PROPOSED * 3 && !note {
                cited.push(signal.id.clone());
            }
            objects.push(json!({
                "id": signal.id, "machine": "signal", "parent": capture_id,
                "state": {"move": "unmoved"}, "record": signal.fields()
            }));
        }
    }
    for n in 0..counts.intents {
        let proposed = n < PROPOSED;
        let id = format!("intent/willdan-subject-{n}");
        objects.push(match proposed {
            true => json!({
                "id": id, "machine": "intent", "state": {"life": "proposed"},
                "record": {
                    "title": format!("declined cards are retried too soon, subject {n}"),
                    "signals": cited.iter().skip(n * 3).take(3).collect::<Vec<_>>()
                }
            }),
            false => json!({
                "id": id, "machine": "intent", "state": {"life": "closed"},
                "record": {"title": format!("an earlier subject, closed, {n}")}
            }),
        });
    }
    for n in 0..counts.facts {
        let capture = signals::object_of(&format!(
            "meeting/2026-08-{:02}/willdan-weekly-{}",
            1 + (n % meetings) % 28,
            n % meetings
        ));
        objects.push(json!({
            "id": format!("fact/session/{capture}/capture-reader/{}", 1 + n / meetings),
            "machine": "fact", "state": {},
            "record": {"host": "local", "runner": "herdr", "started_at": "2026-09-01T08:00:00Z"}
        }));
    }
    let scenario = serde_json::from_value(json!({
        "scenario": "budget",
        "title": "the page within its budget",
        "profiles": ["git-only"],
        "satisfies": [310],
        "given": {
            "objects": objects,
            "hosts": [{"name": "local", "bound": 4}],
            "files": world.files.clone(),
        },
        "when": [],
    }))
    .expect("the described instance is a scenario");
    (scenario, world)
}

/// The described instance in a state repository of its own, its decisions
/// numbered as a tick numbers them, and served (92, 15, 310).
fn served(counts: Counts, name: &str) -> driver::Served {
    let (scenario, world) = described(counts);
    let suite = Suite::open(&root().join("conformance")).expect("the suite");
    let defs = flywheel_engine::load::load_dir(&root().join("definitions")).expect("the machines load");
    let mut runtime = drive::seed(defs, &scenario, &suite).expect("the instance seeds");
    // The intents closed before today finished days ago, so what finished
    // today is the notes alone (S9).
    let before = runtime.store.now - chrono::Duration::days(3);
    for object in runtime.store.objects.values_mut() {
        if object.machine == "intent" && object.config.get("life").map(String::as_str) == Some("closed") {
            object.entered_at.values_mut().for_each(|at| *at = before);
        }
    }
    let base = std::env::temp_dir().join(format!("flywheel-budget-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let host = runtime.store.me();
    runtime.store.bind_state_repository(&base, &[host]).expect("the state repository");
    let standing = flywheel_domain::commands::rail_read(&runtime.store, &runtime.defs).expect("the rail");
    let mut register = flywheel_domain::commands::register(&runtime.store).expect("the register");
    if register.number_all(&standing) {
        let ids: Vec<String> = standing.iter().map(|d| d.id.clone()).collect();
        flywheel_domain::commands::set_register(&mut runtime.store, &register, &ids).expect("numbered");
    }
    driver::Served::page_over(runtime.store, world, runtime.defs, "chuck").expect("the page serves")
}

/// The page answers at its address before a browser is sent there, so a page
/// that is not served says why rather than failing as a navigation.
fn answers(url: &str) {
    use std::io::{Read, Write};
    let rest = url.trim_start_matches("http://");
    let (host, path) = rest.split_once('/').expect("a path");
    let mut socket = std::net::TcpStream::connect(host).expect("the served address");
    write!(socket, "GET /{path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").expect("the request");
    let mut reply = String::new();
    socket.read_to_string(&mut reply).expect("the reply");
    let status = reply.lines().next().unwrap_or_default();
    assert!(
        status.contains(" 200 "),
        "{url} answers `{status}`: {}",
        reply.split_once("\r\n\r\n").map(|(_, b)| b).unwrap_or_default().chars().take(400).collect::<String>()
    );
}

/// One load of the first view, as the browser took it.
#[derive(Debug, Clone, Default)]
struct Load {
    /// First contentful paint, from the request.
    paint: f64,
    /// When the press on the rail's first card was handled, from the request.
    press: f64,
    /// Every response the load took, as it came over the wire.
    bytes: u64,
    /// The first view's HTML before compression.
    html: u64,
    responses: Vec<(String, u64)>,
}

/// What one instance measured.
struct Measured {
    name: String,
    counts: Counts,
    cold: Load,
    cached: Load,
    dock: u64,
    tray: u64,
    update: u64,
}

fn js(tab: &Tab, expression: &str) -> Option<Value> {
    tab.evaluate(expression, false).ok().and_then(|r| r.value)
}

fn js_await(tab: &Tab, expression: &str) -> Option<Value> {
    tab.evaluate(expression, true).ok().and_then(|r| r.value)
}

/// A mid-range phone over an ordinary mobile connection: a 4× CPU slowdown,
/// 5 Mbps down, 1 Mbps up, 150 ms round trip (`surfaces.yaml` budget).
fn throttle(tab: &Tab) {
    tab.call_method(Network::Enable {
        max_total_buffer_size: None,
        max_resource_buffer_size: None,
        max_post_data_size: None,
        report_direct_socket_traffic: None,
        enable_durable_messages: None,
    })
    .expect("the network domain");
    tab.call_method(Emulation::SetCPUThrottlingRate { rate: 4.0 }).expect("the CPU slowdown");
    tab.call_method(Network::EmulateNetworkConditions {
        offline: false,
        latency: 150.0,
        download_throughput: 5_000_000.0 / 8.0,
        upload_throughput: 1_000_000.0 / 8.0,
        connection_Type: None,
        packet_loss: None,
        packet_queue_length: None,
        packet_reordering: None,
    })
    .expect("the connection");
}

/// A press at a point of the page, as a finger or a pointer makes one.
fn press(tab: &Tab, x: f64, y: f64) {
    for kind in [
        Input::DispatchMouseEventTypeOption::MousePressed,
        Input::DispatchMouseEventTypeOption::MouseReleased,
    ] {
        tab.call_method(Input::DispatchMouseEvent {
            Type: kind,
            x,
            y,
            modifiers: None,
            timestamp: None,
            button: Some(Input::MouseButton::Left),
            buttons: Some(1),
            click_count: Some(1),
            force: None,
            tangential_pressure: None,
            tilt_x: None,
            tilt_y: None,
            twist: None,
            delta_x: None,
            delta_y: None,
            pointer_Type: None,
        })
        .expect("the press");
    }
}

/// Load the first view once, pressing the rail's first card the moment the page
/// has drawn it and taken it in hand.
fn load(tab: &Tab, url: &str, cold: bool) -> Load {
    tab.navigate_to("about:blank").expect("a blank page");
    driver::within(WAIT, || (js(tab, "location.href") == Some(json!("about:blank"))).then_some(()))
        .expect("the blank page");
    if cold {
        tab.call_method(Network::ClearBrowserCache(None)).expect("an empty cache");
    }
    tab.navigate_to(url).expect("the page");
    // The card in hand is the page's script having run over the rail: a press
    // before that reaches nothing that answers it (S219).
    let target = format!(
        "(() => {{ if (location.href !== {url:?}) return null; \
          const c = document.querySelector('#rail .card.decision.focused'); if (!c) return null; \
          const r = c.getBoundingClientRect(); if (!r.width) return null; \
          return JSON.stringify([r.left + 5, r.top + 4]); }})()"
    );
    let at: Vec<f64> = driver::within(WAIT, || {
        js(tab, &target).and_then(|v| v.as_str().map(String::from)).and_then(|s| serde_json::from_str(&s).ok())
    })
    .unwrap_or_else(|| panic!("the rail's first card had not stood in hand {} s after asking for {url}", WAIT.as_secs()));
    press(tab, at[0], at[1]);
    driver::within(WAIT, || (js(tab, "document.readyState") == Some(json!("complete"))).then_some(()))
        .unwrap_or_else(|| panic!("{url} had not loaded {} s after it was asked for", WAIT.as_secs()));
    let read = js_await(
        tab,
        "(async () => { \
           const one = (type) => new Promise(ok => { \
             new PerformanceObserver(list => ok(list.getEntries()[0])).observe({type, buffered: true}); \
             setTimeout(() => ok(null), 5000); }); \
           const paint = (await one('paint') && performance.getEntriesByName('first-contentful-paint')[0]) || null; \
           const input = await one('first-input'); \
           await new Promise(ok => document.readyState === 'complete' ? ok() : addEventListener('load', ok)); \
           const nav = performance.getEntriesByType('navigation')[0]; \
           const responses = performance.getEntriesByType('resource') \
             .filter(r => r.startTime <= nav.loadEventEnd) \
             .map(r => [r.name, r.transferSize]); \
           return JSON.stringify({ paint: paint && paint.startTime, press: input && input.processingEnd, \
             html: nav.decodedBodySize, transfer: nav.transferSize, responses }); })()",
    )
    .and_then(|v| v.as_str().map(String::from))
    .and_then(|s| serde_json::from_str::<Value>(&s).ok())
    .unwrap_or_else(|| panic!("the page at {url} said nothing of its timing"));
    let paint = read["paint"].as_f64().unwrap_or_else(|| panic!("no first contentful paint at {url}: {read}"));
    let pressed = read["press"].as_f64().unwrap_or_else(|| panic!("the press on the first card was never handled at {url}: {read}"));
    let responses: Vec<(String, u64)> = read["responses"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| (r[0].as_str().unwrap_or_default().to_string(), r[1].as_u64().unwrap_or(0)))
        .collect();
    Load {
        paint,
        press: pressed,
        bytes: read["transfer"].as_u64().unwrap_or(0) + responses.iter().map(|(_, b)| b).sum::<u64>(),
        html: read["html"].as_u64().unwrap_or(0),
        responses,
    }
}

/// The loads of one kind, with the paint and the press their medians.
fn loads(tab: &Tab, url: &str, cold: bool) -> Load {
    let mut all: Vec<Load> = (0..LOADS).map(|_| load(tab, url, cold)).collect();
    let median = |mut values: Vec<f64>| {
        values.sort_by(|a, b| a.total_cmp(b));
        values[values.len() / 2]
    };
    let paint = median(all.iter().map(|l| l.paint).collect());
    let press = median(all.iter().map(|l| l.press).collect());
    let mut last = all.pop().expect("a load");
    last.paint = paint;
    last.press = press;
    last
}

/// What the responses the page fetched since a mark weighed, once it has
/// stopped fetching.
fn fetched_since_mark(tab: &Tab) -> u64 {
    let expression = "JSON.stringify(performance.getEntriesByType('resource') \
                      .filter(r => r.startTime >= window.__since).map(r => r.transferSize))";
    let read = || -> Vec<u64> {
        js(tab, expression)
            .and_then(|v| v.as_str().map(String::from))
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    };
    // Settled once three looks a tenth of a second apart see the same.
    let mut seen: Vec<Vec<u64>> = Vec::new();
    driver::within(WAIT, || {
        seen.push(read());
        let n = seen.len();
        (n >= 3 && seen[n - 1] == seen[n - 2] && seen[n - 2] == seen[n - 3] && !js(tab, "!!document.querySelector('form.busy')").is_some_and(|v| v == json!(true)))
            .then(|| seen[n - 1].iter().sum())
    })
    .expect("the page stopped fetching")
}

/// Open something in the dock the way a click on it does, and weigh what it
/// fetched.
fn opened(tab: &Tab, click: &str, surface: &str) -> u64 {
    let clicked = js(tab, &format!("(() => {{ window.__since = performance.now(); const el = {click}; if (!el) return false; el.click(); return true; }})()"));
    assert_eq!(clicked, Some(json!(true)), "nothing to open with `{click}`");
    let shown = format!(
        "(() => {{ const s = document.getElementById({surface:?}); \
          return !!s && getComputedStyle(s).display !== 'none' && !s.hasAttribute('data-loading'); }})()"
    );
    driver::within(WAIT, || (js(tab, &shown) == Some(json!(true))).then_some(()))
        .unwrap_or_else(|| panic!("`{surface}` never opened"));
    fetched_since_mark(tab)
}

/// Measure one served instance: the first view with nothing cached and with the
/// fonts cached, then one dock page, the tray, and the update one answer causes.
fn measure(tab: &Tab, counts: Counts, name: &str) -> Measured {
    let served = served(counts, name);
    let url = format!("{}willdan/", served.url);
    answers(&url);
    let cold = loads(tab, &url, true);
    let cached = loads(tab, &url, false);
    let object = js(
        tab,
        "(() => { const el = [...document.querySelectorAll('#board .lane [data-machine][id]')][0]; return el ? el.id : null; })()",
    )
    .and_then(|v| v.as_str().map(String::from))
    .expect("an object on the board");
    let dock = opened(
        tab,
        &format!("document.getElementById({object:?})"),
        &format!("dock-{object}"),
    );
    let tray = opened(tab, "document.querySelector('a.curation')", "dock-tray");
    let answered = js(
        tab,
        "(() => { window.__since = performance.now(); \
          const all = [...document.querySelectorAll('#rail .card.decision form button[data-answer]')]; \
          const b = all.find(b => b.getAttribute('data-answer') === 'yes') || all[0]; \
          if (!b) return null; b.click(); return b.getAttribute('data-answer'); })()",
    );
    assert!(answered.is_some_and(|a| a.is_string()), "no answer to give on the rail");
    // The post goes busy at once; what it and anything after it fetched is the
    // update the answer caused (S221).
    let update = fetched_since_mark(tab);
    drop(served);
    Measured { name: name.to_string(), counts, cold, cached, dock, tray, update }
}

fn kb(bytes: u64) -> String {
    format!("{:.1} KB", bytes as f64 / 1000.0)
}

fn secs(ms: f64) -> String {
    format!("{:.2} s", ms / 1000.0)
}

fn said(m: &Measured) -> String {
    format!(
        "{} ({} signals · {} captures · {} intents · {} facts): first paint {} · first press {} · \
         a load {} ({} with the fonts cached, paint {}) · first view HTML {} · a dock page {} · \
         the tray {} · the update after an answer {}\n    responses: {}",
        m.name,
        m.counts.signals,
        m.counts.captures,
        m.counts.intents,
        m.counts.facts,
        secs(m.cold.paint),
        secs(m.cold.press),
        kb(m.cold.bytes),
        kb(m.cached.bytes),
        secs(m.cached.paint),
        kb(m.cold.html),
        kb(m.dock),
        kb(m.tray),
        kb(m.update),
        m.cold
            .responses
            .iter()
            .map(|(name, bytes)| format!("{} {}", name.rsplit('/').next().unwrap_or(name), kb(*bytes)))
            .collect::<Vec<_>>()
            .join(" · ")
    )
}

/// Each of 310a's numbers this instance is over.
fn over_budget(m: &Measured, over: &mut Vec<String>) {
    let at = &m.name;
    if m.cold.paint > FIRST_PAINT_MS {
        over.push(format!("{at}: first paint {} is over 1.0 s", secs(m.cold.paint)));
    }
    if m.cold.press > FIRST_PRESS_MS {
        over.push(format!("{at}: the first card took a press at {}, over 1.5 s", secs(m.cold.press)));
    }
    if m.cold.bytes > LOAD_BYTES {
        over.push(format!("{at}: a load with nothing cached is {}, over 200 KB", kb(m.cold.bytes)));
    }
    if m.cached.bytes > CACHED_LOAD_BYTES {
        over.push(format!("{at}: a load with the fonts cached is {}, over 60 KB", kb(m.cached.bytes)));
    }
    if m.cold.html > FIRST_VIEW_HTML_BYTES {
        over.push(format!("{at}: the first view's HTML is {}, over 100 KB", kb(m.cold.html)));
    }
    if m.update > UPDATE_BYTES {
        over.push(format!("{at}: the update after an answer is {}, over 8 KB", kb(m.update)));
    }
}

/// Whether the larger instance costs more than what is on screen explains.
fn grew(small: &Measured, large: &Measured, over: &mut Vec<String>) {
    if large.cold.paint > small.cold.paint * 1.1 {
        over.push(format!(
            "at ten times the instance first paint is {} against {}, more than a tenth over",
            secs(large.cold.paint),
            secs(small.cold.paint)
        ));
    }
    if large.cold.bytes as f64 > small.cold.bytes as f64 * 1.1 {
        over.push(format!(
            "at ten times the instance a load is {} against {}, more than a tenth over",
            kb(large.cold.bytes),
            kb(small.cold.bytes)
        ));
    }
    if large.update > small.update {
        over.push(format!(
            "at ten times the instance the update after an answer weighs {} against {}",
            kb(large.update),
            kb(small.update)
        ));
    }
}

/// The page keeps 310a's budget at the round's counts and at ten times each,
/// measured as the surfaces profile states (310a, S235).
#[test]
fn the_page_keeps_its_budget() {
    if !driver::available() {
        eprintln!("skipped: no browser to drive; the budget is measured in a Chromium or Chrome (D15)");
        return;
    }
    let browser = driver::Driver::open().expect("a browser");
    let tab = browser.visit("about:blank", driver::PHONE).expect("a blank page").clone();
    throttle(&tab);
    let small = measure(&tab, ROUND, "the round's counts");
    let large = measure(&tab, ROUND.times(10), "ten times each");
    let mut over = Vec::new();
    over_budget(&small, &mut over);
    over_budget(&large, &mut over);
    grew(&small, &large, &mut over);
    let report = format!("{}\n{}", said(&small), said(&large));
    eprintln!("{report}");
    assert!(
        over.is_empty(),
        "the page is over its budget (310a):\n  {}\n\nmeasured:\n{report}",
        over.join("\n  ")
    );
}
