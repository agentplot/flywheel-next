use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use flywheel::console;
use flywheel::init;
use flywheel::report::{self, Report, Reported, SESSION_ENV};
use flywheel_scenario::{conformance, scenario, Runtime, Store};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "flywheel", about = "flywheel next — prototype: seed a scenario, tick the machinery, answer decisions, serve the rail")]
struct Cli {
    /// Machine definitions directory.
    #[arg(long, global = true, default_value = "definitions")]
    defs: PathBuf,
    /// The store file.
    #[arg(long, global = true, default_value = "state/store.json")]
    state: PathBuf,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// The release's set version, and the version of every core machine in it.
    Version {
        /// List the core machines the binary carries and their versions (224).
        #[arg(long)]
        definitions: bool,
    },
    /// What a host does. A host runs the set the binary carries and no other:
    /// `--definitions` is the scenario runner's alone (D2, 223).
    Host {
        /// With no subcommand, the host runs: one long-lived process, the
        /// notify-tick and the 60-second sweep, fetching before every tick
        /// (D7, 231).
        #[command(subcommand)]
        cmd: Option<HostCmd>,
        /// The manifest this host reads itself from (183).
        #[arg(long, global = true, default_value = "flywheel.yaml")]
        manifest: PathBuf,
        /// Which host this is; several run on one computer, told apart by id
        /// alone (232).
        #[arg(long, global = true, default_value = "local")]
        name: String,
        /// Where this host keeps its clones, in place of the manifest's own.
        /// Several hosts run on one computer, each under a root of its own
        /// (205, 232).
        #[arg(long, global = true)]
        root: Option<PathBuf>,
        /// Refused, wherever it is written. A host runs the definitions in the
        /// binary (223, D2).
        #[arg(long, global = true)]
        definitions: Option<PathBuf>,
    },
    /// Bring an instance and its first host into existence: the blueprints, the
    /// state repository, the App's installation recorded, the first host
    /// registered. Every step proves itself, so running it again changes
    /// nothing and a half-finished bootstrap is finished here (204).
    Init {
        /// The operator's name for the instance; it need not be a git-host
        /// organization (219).
        #[arg(long)]
        instance: String,
        /// The first host's name.
        #[arg(long, default_value = "local")]
        host: String,
        /// Where this host keeps its clones (205).
        #[arg(long)]
        root: PathBuf,
        /// The git host: where the instance's repositories live.
        #[arg(long)]
        git_host: PathBuf,
        /// The App's id. Its key is the operator's to place (207a).
        #[arg(long, default_value = "0")]
        app: String,
        /// The environment variable the operator put the App's key in.
        #[arg(long, default_value = "FLYWHEEL_APP_KEY")]
        app_key_from: String,
        /// Where the manifest is written.
        #[arg(long, default_value = "flywheel.yaml")]
        manifest: PathBuf,
    },
    /// Load the machine definitions and report what was read.
    Defs,
    /// Seed the store from a scenario file (replaces the store).
    Seed { scenario: PathBuf, /// Also run the scenario's `when` steps.
        #[arg(long)] drive: bool },
    /// Run ticks until nothing fires (or N ticks).
    Tick { #[arg(default_value = "0")] n: usize },
    /// Answer a numbered decision.
    Respond { number: u32, answer: Vec<String> },
    /// Dictate on an object.
    Dictate { object: String, answer: Vec<String> },
    /// Print the rail.
    Rail,
    /// Print the last N log lines.
    Log { #[arg(default_value = "30")] n: usize },
    /// Serve the page: one bundle, the rail, the capture box and the status
    /// view, at the host's private-network address (307, D11).
    Serve {
        #[arg(long, default_value = "4242")] port: u16,
        /// The host's address, with the instance in the path — the
        /// private-network name its router gives it (205a, D10a).
        #[arg(long, default_value = "http://localhost/flywheel")] address: String,
        /// The instance's operators list; while it holds one entry the page is
        /// served with no sign-in (236a, 253a).
        #[arg(long = "operator", default_value = "operator")] operators: Vec<String>,
    },
    /// The conformance suite: load the definitions, seed the stores, play the
    /// steps against the real engine and assert the `then` clauses (94).
    Scenario {
        #[command(subcommand)]
        cmd: ScenarioCmd,
    },
    /// An adapter: one keyed capture per source event, and nothing else (111,
    /// 115, 215). `flywheel capture meeting <file>` is the one this release
    /// ships beside the page's box and the chat forward (D13).
    Capture {
        /// The adapter: `meeting`.
        kind: String,
        /// What it enumerates — for a meeting, the transcript's path in the raw
        /// store, which is cited and never copied in (111).
        source: String,
        /// Who captured it; the operators list's entry by default (153).
        #[arg(long, default_value = "operator")]
        by: String,
    },
    /// Dump an object's configuration and record.
    Obj { id: String },
    /// Evaluate one evidence name for an object (and optional region path).
    Ev { object: String, name: String, #[arg(default_value = "life")] region: String },

    // ---- what a session reports (65, 67). The operator runs these in phase 1
    // (93b); the scripted stand-in and the phase-2 runner run the same binary.
    /// Report an exit: done, blocked or stalled.
    Exit {
        kind: String,
        /// What `done` delivered; repeatable.
        #[arg(long = "deliverable")]
        deliverables: Vec<String>,
        /// What `blocked` is blocked on.
        #[arg(long)]
        question: Option<String>,
        /// Anything else the report carries.
        #[arg(long)]
        text: Option<String>,
        #[arg(long, env = SESSION_ENV, default_value = "")]
        session: String,
    },
    /// Offer a finding or a chore, pointing at its document.
    Offer {
        kind: String,
        #[arg(long)]
        document: String,
        #[arg(long, env = SESSION_ENV, default_value = "")]
        session: String,
    },
    /// Say something on the session's thread that is not an exit.
    Note {
        text: Vec<String>,
        #[arg(long, env = SESSION_ENV, default_value = "")]
        session: String,
    },
    /// Refuse the work the session was given (43).
    Refuse {
        reason: Vec<String>,
        #[arg(long, env = SESSION_ENV, default_value = "")]
        session: String,
    },
}

/// Write one report through the store and print what it was. A refused report
/// is recorded and the command exits non-zero, so nothing is dropped and the
/// caller learns (66, 80).
fn do_report(cli: &Cli, session: &str, report: &Report) -> Result<i32> {
    let mut store = flywheel_scenario::load(&cli.state)
        .with_context(|| "no store; run `flywheel seed <scenario>` first")?;
    let by = std::env::var("USER").unwrap_or_else(|_| "operator".into());
    let at = chrono::Utc::now();
    let outcome = report::write_report(&mut store, session, &by, at, report)?;
    flywheel_scenario::save(&store, &cli.state)?;
    Ok(match outcome {
        Reported::Accepted(entry) => {
            println!("{} recorded on {session}", entry.kind);
            0
        }
        Reported::Refused { reason, .. } => {
            eprintln!("refused: {reason}");
            eprintln!("recorded on {session} as invalid, with the raw report");
            1
        }
    })
}

/// What a host is asked to do. Group 5 fills these in; the binding rule they
/// share is here from the start (D2).
#[derive(Subcommand, Clone)]
enum HostCmd {
    /// Report what this host is, what set it runs, and what it refused.
    Doctor {
        /// A blueprints checkout to read the instance's own types from (57, 85).
        #[arg(long)]
        blueprints: Option<PathBuf>,
        /// Check this host's layout against the manifest too (205, 222).
        #[arg(long)]
        layout: bool,
    },
    /// Clone what the manifest names under this host's root, and check the
    /// layout (205, 222).
    Join,
    /// Run the loop, and say what each pass did rather than staying silent.
    Run {
        /// Stop after this many passes; zero runs until the process is stopped.
        #[arg(long, default_value = "0")]
        passes: usize,
        /// Take the clock and the sweep from the caller, one command per line
        /// on the input, rather than from a timer. What the conformance runner
        /// starts a host with under `--hosts real`, so a two-host run is
        /// deterministic (D15, D7).
        #[arg(long)]
        driven: bool,
    },
}

#[derive(Subcommand)]
enum ScenarioCmd {
    /// Run scenario files or a directory of them.
    Run {
        paths: Vec<PathBuf>,
        /// The `StateStore` binding.
        #[arg(long, default_value = "stand-in")]
        profile: String,
        /// Every host in the scenario is a process of its own.
        #[arg(long = "hosts", value_parser = ["real"])]
        hosts: Option<String>,
        /// Load the machine files from a directory instead of the embedded set.
        #[arg(long)]
        definitions: Option<PathBuf>,
        /// Render each run as a trace; the directory defaults to
        /// `target/flywheel-trace/<profile>/`.
        #[arg(long, num_args = 0..=1, default_missing_value = "")]
        trace: Option<String>,
    },
}

fn open(cli: &Cli) -> Result<Runtime> {
    let defs = flywheel_engine::load::load_dir(&cli.defs)?;
    let store = flywheel_scenario::load(&cli.state).with_context(|| "no store; run `flywheel seed <scenario>` first")?;
    Ok(Runtime::new(defs, store))
}

/// One tick as this host runs it: the stand-in's own world moves first — the
/// heartbeats, the scripted sessions, the tethered processes — and then the
/// engine's pass goes through the store's operations (D7, 125). Group 6 gives
/// this a home of its own in `flywheel host`.
fn host_tick(rt: &mut Runtime) -> Result<usize> {
    use flywheel_atoms::Scope;
    rt.store.tick += 1;
    let now = rt.store.now;
    let hosts: Vec<String> = rt
        .store
        .objects
        .values()
        .filter(|o| o.machine == "host" && o.record.get("alive").and_then(|v| v.as_bool()).unwrap_or(true))
        .map(|o| o.id.clone())
        .collect();
    for h in hosts {
        rt.store.set_given(&h, "host.last_seen", serde_json::json!(now.to_rfc3339()));
    }
    rt.store.play_scripts();
    rt.store.play_services();

    let defs = rt.defs.clone();
    let ticked = console::tick(
        &mut rt.store,
        &defs,
        &Scope::All,
        |store, object, region, effect| flywheel_scenario::world::perform(&defs, store, object, region, effect),
        |store, fired, tail| {
            store.log(
                "transition",
                &fired.object,
                format!(
                    "{}: {} → {}{}",
                    fired.region,
                    fired.from,
                    fired.to,
                    fired.note.as_ref().map(|n| format!(" — {n}")).unwrap_or_default()
                ),
            );
            store.tail.extend(tail);
        },
    )?;
    rt.store.now = rt.store.now + chrono::Duration::seconds(rt.store.tick_seconds);
    Ok(ticked.transitions)
}

/// Tick until nothing fires, so one response cascades as far as it can.
fn settle(rt: &mut Runtime, max: usize) -> Result<usize> {
    let mut total = 0;
    for _ in 0..max {
        let n = host_tick(rt)?;
        total += n;
        if n == 0 { break; }
    }
    Ok(total)
}

fn print_rail(rt: &mut Runtime) -> Result<()> {
    // Through the trait surface: the objects from `list`, the numbers from the
    // rail record (15, 131).
    let d = console::rail(&mut rt.store, &rt.defs)?;
    let count = d.iter().filter(|x| x.group != "attention").count();
    println!("DECISIONS · {} · tick {} · {} decisions", rt.store.now.format("%Y-%m-%d %H:%M"), rt.store.tick, count);
    for g in ["approve", "decide", "answer", "attention"] {
        let rows: Vec<_> = d.iter().filter(|x| x.group == g).collect();
        if rows.is_empty() { continue; }
        println!("  {}", g.to_uppercase());
        for x in rows {
            println!("    {:>4}  {:<22} {:<40} → {}", x.number.unwrap_or(0), x.kind, x.object, x.answers.join(" · "));
        }
    }
    if !rt.store.tail.is_empty() {
        println!("  SINCE");
        for t in rt.store.tail.iter().rev().take(8) {
            println!("        {:<8} {:<40} {}", t.kind, t.object, t.at.format("%H:%M"));
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match &cli.cmd {
        Cmd::Defs => {
            let defs = flywheel_engine::load::load_dir(&cli.defs)?;
            println!("machines: {}", defs.machines.len());
            for (name, m) in &defs.machines {
                let states: usize = m.regions.values().map(|r| r.states.len()).sum();
                println!("  {name:<22} kind={:<9} object={:<16} regions={} states={}", format!("{:?}", m.kind).to_lowercase(), m.object.clone().unwrap_or_default(), m.regions.len(), states);
            }
            println!("evidence atoms: {}  effects: {}", defs.atoms.evidence.len(), defs.atoms.effects.len());
        }
        Cmd::Init {
            instance,
            host,
            root,
            git_host,
            app,
            app_key_from,
            manifest,
        } => {
            let report = init::run(init::Init {
                instance: instance.clone(),
                host: host.clone(),
                root: root.clone(),
                git_host: git_host.clone(),
                app: app.clone(),
                app_key_from: app_key_from.clone(),
                manifest: manifest.clone(),
                state: cli.state.clone(),
            })?;
            for line in &report.lines {
                println!("{line}");
            }
        }
        Cmd::Version { definitions } => {
            println!("flywheel {}", env!("CARGO_PKG_VERSION"));
            println!(
                "definition set {} · digest {:016x}",
                flywheel_domain::set::SET_VERSION,
                flywheel_domain::set::digest()
            );
            if *definitions {
                for (name, version) in flywheel_domain::set::versions()? {
                    println!("  {name:<22} version {version}");
                }
            }
        }
        Cmd::Host { cmd, definitions, manifest, name, root } => {
            // A host runs the set the binary carries. The directory load is the
            // scenario runner's, and a host that took one could not prove which
            // set it ran (223, 224, D2).
            if let Some(dir) = definitions {
                anyhow::bail!(
                    "`--definitions {}` is refused: a host runs the definitions in the binary, \
                     so that the set it ran is provable from the bytes (223, 224, D2). \
                     The override is the scenario runner's: `flywheel scenario run --definitions`",
                    dir.display()
                );
            }
            match cmd.clone().unwrap_or(HostCmd::Run { passes: 0, driven: false }) {
                HostCmd::Join => {
                    let read = flywheel::host::manifest_with_root(manifest, name, root.as_deref())?;
                    let mut world = flywheel_world_host::HostWorld::open(read, name)?;
                    let joined = flywheel_world_host::join::join(&mut world)?;
                    for cloned in &joined.cloned {
                        println!("cloned {cloned}");
                    }
                    if joined.cloned.is_empty() {
                        println!("nothing to clone: the layout is already what the manifest says");
                    }
                    // A host refuses to start on a hand-made layout, and says
                    // what differs (205, 222). The doctor runs at join and at
                    // every tick.
                    let differences = flywheel_world_host::join::doctor(&world);
                    for difference in &differences {
                        println!("differs: {difference}");
                    }
                    if !differences.is_empty() {
                        anyhow::bail!("this host will not start on a layout it did not make (205, 222)");
                    }
                }
                HostCmd::Doctor { blueprints, layout } => {
                    println!(
                        "definition set {} · digest {:016x} · {} core machines",
                        flywheel_domain::set::SET_VERSION,
                        flywheel_domain::set::digest(),
                        flywheel_domain::set::versions()?.len()
                    );
                    if layout {
                        let read = flywheel::host::manifest_with_root(manifest, name, root.as_deref())?;
                        let world = flywheel_world_host::HostWorld::open(read, name)?;
                        let differences = flywheel_world_host::join::doctor(&world);
                        match differences.is_empty() {
                            true => println!("layout: as the manifest says"),
                            false => {
                                for difference in &differences {
                                    println!("differs: {difference}");
                                }
                                anyhow::bail!(
                                    "this host will not start on a layout it did not make (205, 222)"
                                );
                            }
                        }
                    }
                    if let Some(dir) = &blueprints {
                        let loaded = flywheel_domain::blueprints::load_over_core(dir)?;
                        let core = flywheel_domain::set::load()?;
                        // A pinned `name@version` is the same machine under a
                        // second key, not a type of its own.
                        let own = loaded
                            .defs
                            .machines
                            .keys()
                            .filter(|k| !k.contains('@') && !core.machines.contains_key(*k))
                            .count();
                        println!("blueprints {} · {own} types of its own", dir.display());
                        // A refusal is reported, never swallowed: what the
                        // binary carries goes on running and the operator is
                        // told what was not read (223, 79-82).
                        for refusal in &loaded.refusals {
                            println!("refused: {refusal}");
                        }
                    }
                }
                HostCmd::Run { passes, driven } => {
                    let mut host =
                        flywheel::host::Host::open(manifest, name, root.as_deref(), chrono::Utc::now())?;
                    if driven {
                        // The clock and the sweep come from the caller; nothing
                        // here keeps time (D15, D7).
                        host.record_bindings();
                        let stdin = std::io::stdin();
                        return flywheel::driven::drive(
                            &mut host,
                            stdin.lock(),
                            std::io::stdout(),
                        );
                    }
                    println!(
                        "host {} · instance {} · world {} · workspace {} · sessions {}",
                        host.name,
                        host.instance,
                        host.bindings.world,
                        host.bindings.workspace,
                        host.bindings.sessions
                    );
                    host.record_bindings();
                    // One long-lived process: the notify poll, and the sweep
                    // every 60 seconds whatever the poll says (D6, D7, 231).
                    let mut pass = 0usize;
                    loop {
                        host.set_now(chrono::Utc::now());
                        match host.once() {
                            Ok(fired) => {
                                if fired > 0 {
                                    println!("{fired} transitions");
                                }
                            }
                            // A problem with the machinery is reported and made
                            // no work of; the loop goes on (81).
                            Err(e) => {
                                host.report_problem(&format!("host/{}", host.name), &format!("{e:#}"));
                                eprintln!("problem: {e:#}");
                            }
                        }
                        pass += 1;
                        if passes > 0 && pass >= passes {
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(
                            flywheel::host::POLL as u64,
                        ))
                        .await;
                    }
                }
            }
        }
        Cmd::Seed { scenario: path, drive } => {
            let defs = flywheel_engine::load::load_dir(&cli.defs)?;
            let sc = scenario::load(path)?;
            let mut rt = scenario::seed(defs, &sc);
            // The described world is the stand-in's (93); the objects and the
            // register reach the store through its own operations (125, 131).
            let objects: Vec<_> = rt.store.objects.values().cloned().collect();
            let start = console::register(&rt.store)?.next_number;
            let defs = rt.defs.clone();
            console::seed(&mut rt.store, &defs, &objects, Some(start))?;
            if *drive { scenario::drive(&mut rt, &sc); }
            flywheel_scenario::save(&rt.store, &cli.state)?;
            println!("seeded {} · {} objects", sc.scenario, rt.store.objects.len());
            print_rail(&mut rt)?;
        }
        Cmd::Tick { n } => {
            let mut rt = open(&cli)?;
            let fired = if *n == 0 { settle(&mut rt, 50)? } else { let mut total = 0; for _ in 0..*n { total += host_tick(&mut rt)?; } total };
            flywheel_scenario::save(&rt.store, &cli.state)?;
            println!("{fired} transitions");
            print_rail(&mut rt)?;
        }
        Cmd::Respond { number, answer } => {
            let mut rt = open(&cli)?;
            let (_, journal) = console::respond(&mut rt.store, &rt.defs, *number, &answer.join(" "), "cli")?;
            for n in journal { rt.store.log(&n.kind, &n.object, n.text); }
            let fired = settle(&mut rt, 50)?;
            flywheel_scenario::save(&rt.store, &cli.state)?;
            println!("{fired} transitions");
            print_rail(&mut rt)?;
        }
        Cmd::Dictate { object, answer } => {
            let mut rt = open(&cli)?;
            let (_, journal) = console::dictate(&mut rt.store, &rt.defs, object, &answer.join(" "), "cli")?;
            for n in journal { rt.store.log(&n.kind, &n.object, n.text); }
            let fired = settle(&mut rt, 50)?;
            flywheel_scenario::save(&rt.store, &cli.state)?;
            println!("{fired} transitions");
            print_rail(&mut rt)?;
        }
        Cmd::Rail => { let mut rt = open(&cli)?; print_rail(&mut rt)?; }
        Cmd::Log { n } => {
            let rt = open(&cli)?;
            for l in rt.store.log.iter().rev().take(*n).collect::<Vec<_>>().into_iter().rev() {
                println!("t{:<4} {:<10} {:<40} {}", l.tick, l.kind, l.object, l.text);
            }
        }
        Cmd::Obj { id } => {
            let rt = open(&cli)?;
            match rt.store.objects.get(id) { Some(o) => println!("{}", serde_json::to_string_pretty(o)?), None => println!("no such object") }
        }
        Cmd::Ev { object, name, region } => {
            use flywheel_engine::runtime::EvidenceSource;
            let rt = open(&cli)?;
            println!("{:?}", rt.store.evidence(object, region, name));
        }
        Cmd::Exit { kind, deliverables, question, text, session } => {
            let r = Report::Exit { kind: kind.clone(), deliverables: deliverables.clone(), question: question.clone(), text: text.clone() };
            std::process::exit(do_report(&cli, session, &r)?);
        }
        Cmd::Offer { kind, document, session } => {
            let r = Report::Offer { kind: kind.clone(), document: document.clone() };
            std::process::exit(do_report(&cli, session, &r)?);
        }
        Cmd::Note { text, session } => {
            let r = Report::Note { text: text.join(" ") };
            std::process::exit(do_report(&cli, session, &r)?);
        }
        Cmd::Refuse { reason, session } => {
            let r = Report::Refuse { reason: reason.join(" ") };
            std::process::exit(do_report(&cli, session, &r)?);
        }
        // An adapter runs unattended and makes no judgment: one keyed capture
        // per source event, with a pointer to material it never copies in
        // (111, 115, 215).
        Cmd::Capture { kind, source, by } => {
            let mut rt = open(&cli)?;
            let defs = rt.defs.clone();
            let at = rt.store.now;
            let enumerated = flywheel_scenario::bindings::with_files(&mut rt.store, |store, world| {
                flywheel_domain::adapters::run(
                    store,
                    world,
                    &defs,
                    &format!("{kind} {source}"),
                    &by,
                    at,
                )
            })?;
            flywheel_scenario::save(&rt.store, &cli.state)?;
            for key in &enumerated.keys {
                println!("{key}");
            }
            println!(
                "{} capture(s) written, {} signal(s): an enumerator reads nothing into signals (115)",
                enumerated.captures_written, enumerated.signals_written
            );
        }
        Cmd::Scenario { cmd } => {
            let ScenarioCmd::Run { paths, profile, hosts, definitions, trace } = cmd;
            let options = conformance::RunOptions {
                profile: conformance::Profile::parse(profile)?,
                hosts_real: hosts.as_deref() == Some("real"),
                definitions: definitions.clone().or(Some(cli.defs.clone())),
                trace: trace.as_ref().filter(|t| !t.is_empty()).map(PathBuf::from),
                tracing: trace.is_some(),
                interval: chrono::Duration::seconds(60),
            };
            let paths = if paths.is_empty() { vec![PathBuf::from("conformance")] } else { paths.clone() };
            let report = conformance::run(&paths, &options)?;
            print!("{}", report.render());
            std::process::exit(report.exit_code());
        }
        Cmd::Serve { port, address, operators } => {
            let rt = open(&cli)?;
            let served = flywheel_surface::http::Served::over(
                rt.store,
                Box::new(flywheel_scenario::bindings::FilesWorld::new()),
                rt.defs,
                operators,
                address,
            );
            // The two addresses of 46 and 245: the host's own, and the port the
            // operator at the machine uses. Nothing else is bound.
            println!("page at {address}, and on localhost:{port} for this machine");
            flywheel_surface::http::serve(served, &format!("127.0.0.1:{port}")).await?;
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn _unused(_: Store) {}
