use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use flywheel::init;
use flywheel::report::{self, Report, Reported, SESSION_ENV};
use flywheel_scenario::conformance;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "flywheel", about = "flywheel next — prototype: seed a scenario, tick the machinery, answer decisions, serve the rail")]
struct Cli {
    /// Machine definitions directory.
    #[arg(long, global = true, default_value = "definitions")]
    defs: PathBuf,
    /// This host's checkout of the state repository, which every record
    /// operation goes through (92, 125). A session's work order names it in the
    /// environment, so a report inside a place needs no flag (67, 89).
    #[arg(long, global = true, env = flywheel_scenario::sessions::STATE_ENV,
          default_value = "state/flywheel-state")]
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
        /// Serve the page beside the loop, on this port: one bundle, the rail,
        /// the capture box and the status view, rendered from this host's own
        /// state store on every request (D10a, D11, 307). The host binds its
        /// private-network address and this port for the operator at the
        /// machine, and nothing else (46, 155, 245).
        #[arg(long)]
        serve: Option<u16>,
        /// The instance's operators list; while it holds one entry the page is
        /// served with no sign-in (236a, 253a).
        #[arg(long = "operator", default_value = "operator")]
        operators: Vec<String>,
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
        /// This host's one address: what this computer is called on the
        /// operator's private network, never a localhost port, because every
        /// link a delivery carries is written at it (191, 205a, D10a).
        #[arg(long, default_value = "http://localhost")]
        address: String,
        /// Where the manifest is written.
        #[arg(long, default_value = "flywheel.yaml")]
        manifest: PathBuf,
    },
    /// Load the machine definitions and report what was read.
    Defs,
    /// Render the exact prompt a session would be handed, with no session
    /// started: the closed set of inputs of 89 for one session type, one
    /// instruction version and one scenario's job (90, 124).
    RenderOrder {
        /// The session type; a unit type names its stage, `default/build`.
        session_type: String,
        /// The instruction set version to render against. The set the binary
        /// carries is the release's; another is read with `--instructions`
        /// (123).
        instruction_version: u32,
        /// The scenario the job is taken from.
        scenario: PathBuf,
        /// Read the instruction set from a directory instead of the set the
        /// binary carries, which is how a prompt is rendered against a version
        /// the release does not ship (123, 124).
        #[arg(long)]
        instructions: Option<PathBuf>,
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
        /// The manifest this host reads itself from (183).
        #[arg(long, default_value = "flywheel.yaml")]
        manifest: PathBuf,
        /// Which host's bindings the capture is written through (232).
        #[arg(long, default_value = "local")]
        name: String,
        /// Where this host keeps its clones, in place of the manifest's own
        /// (205, 232).
        #[arg(long)]
        root: Option<PathBuf>,
    },

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
        /// Which host's checkout the report is written through (232).
        #[arg(long, default_value = "local")]
        host: String,
    },
    /// Offer a finding or a chore, pointing at its document.
    Offer {
        kind: String,
        #[arg(long)]
        document: String,
        #[arg(long, env = SESSION_ENV, default_value = "")]
        session: String,
        /// Which host's checkout the report is written through (232).
        #[arg(long, default_value = "local")]
        host: String,
    },
    /// Say something on the session's thread that is not an exit.
    Note {
        text: Vec<String>,
        #[arg(long, env = SESSION_ENV, default_value = "")]
        session: String,
        /// Which host's checkout the report is written through (232).
        #[arg(long, default_value = "local")]
        host: String,
    },
    /// Refuse the work the session was given (43).
    Refuse {
        reason: Vec<String>,
        #[arg(long, env = SESSION_ENV, default_value = "")]
        session: String,
        /// Which host's checkout the report is written through (232).
        #[arg(long, default_value = "local")]
        host: String,
    },
}

/// Write one report through the state repository and print what it was. A
/// refused report is recorded and the command exits non-zero, so nothing is
/// dropped and the caller learns (66, 80).
fn do_report(cli: &Cli, host: &str, session: &str, report: &Report) -> Result<i32> {
    let mut store = open_state(&cli.state, host)?;
    let by = std::env::var("USER").unwrap_or_else(|_| "operator".into());
    let at = chrono::Utc::now();
    let outcome = report::write_report(&mut store, session, &by, at, report)?;
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
        #[arg(long, default_value = "git-only")]
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

/// This host's checkout of the state repository, opened where the operator
/// or the work order pointed. A directory that is no checkout is refused by
/// name rather than cloned into: a report writes through the repository the
/// host already made (92, 205).
fn open_state(root: &Path, host: &str) -> Result<flywheel_store_git::GitStore> {
    if !root.join(".git").is_dir() {
        anyhow::bail!(
            "{} is no checkout of the state repository: pass --state or set {}, \
             which a session's work order names (67, 89)",
            root.display(),
            flywheel_scenario::sessions::STATE_ENV
        );
    }
    flywheel_store_git::GitStore::open(root, root, host, chrono::Utc::now())
        .with_context(|| format!("opening the state repository at {}", root.display()))
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
            address,
            manifest,
        } => {
            let report = init::run(init::Init {
                instance: instance.clone(),
                host: host.clone(),
                root: root.clone(),
                git_host: git_host.clone(),
                app: app.clone(),
                app_key_from: app_key_from.clone(),
                // The command line reads it from where the operator placed it
                // (207a).
                app_key: None,
                address: address.clone(),
                manifest: manifest.clone(),
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
        Cmd::Host { cmd, definitions, manifest, name, root, serve, operators } => {
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
                    // The loop and the page are one process over one store: the
                    // page shows what the last tick wrote, and an answer given
                    // on it is a commit the next tick reads (D11, 132, 153).
                    let host = std::sync::Arc::new(std::sync::Mutex::new(host));
                    // What a local cause wakes, so the loop takes its next pass
                    // at once rather than sleeping out the poll (130, D6).
                    let mut woken: Option<std::sync::Arc<tokio::sync::Notify>> = None;
                    if let Some(port) = serve {
                        let served = flywheel::serve::page_of(&host, *port, operators);
                        woken = Some(served.woken.clone());
                        for listener in flywheel::serve::listeners(&served, *port).await? {
                            println!("page at http://{}/", listener.local_addr()?);
                            let served = served.clone();
                            tokio::spawn(async move {
                                if let Err(e) =
                                    flywheel_surface::http::serve_on(served, listener).await
                                {
                                    eprintln!("the page stopped serving: {e:#}");
                                }
                            });
                        }
                    }
                    // One long-lived process: the notify poll, and the sweep
                    // every 60 seconds whatever the poll says (D6, D7, 231).
                    let mut pass = 0usize;
                    loop {
                        {
                            let mut held = host.lock().expect("the running host is poisoned");
                            held.set_now(chrono::Utc::now());
                            match held.once() {
                                Ok(fired) => {
                                    if fired > 0 {
                                        println!("{fired} transitions");
                                    }
                                }
                                // A problem with the machinery is reported and
                                // made no work of; the loop goes on (81).
                                Err(e) => {
                                    let name = held.name.clone();
                                    held.report_problem(&format!("host/{name}"), &format!("{e:#}"));
                                    eprintln!("problem: {e:#}");
                                }
                            }
                        }
                        pass += 1;
                        if passes > 0 && pass >= passes {
                            break;
                        }
                        // The poll is the floor for another host's writes; a
                        // page response, a chat message or a session's report
                        // reaching this process does not wait for it (130, D6).
                        let poll =
                            tokio::time::sleep(std::time::Duration::from_secs(flywheel::host::POLL as u64));
                        match &woken {
                            Some(woken) => tokio::select! {
                                _ = poll => {}
                                _ = woken.notified() => {}
                            },
                            None => poll.await,
                        }
                    }
                }
            }
        }
        Cmd::Exit { kind, deliverables, question, text, session, host } => {
            let r = Report::Exit { kind: kind.clone(), deliverables: deliverables.clone(), question: question.clone(), text: text.clone() };
            std::process::exit(do_report(&cli, host, session, &r)?);
        }
        Cmd::Offer { kind, document, session, host } => {
            let r = Report::Offer { kind: kind.clone(), document: document.clone() };
            std::process::exit(do_report(&cli, host, session, &r)?);
        }
        Cmd::Note { text, session, host } => {
            let r = Report::Note { text: text.join(" ") };
            std::process::exit(do_report(&cli, host, session, &r)?);
        }
        Cmd::Refuse { reason, session, host } => {
            let r = Report::Refuse { reason: reason.join(" ") };
            std::process::exit(do_report(&cli, host, session, &r)?);
        }
        // An adapter runs unattended and makes no judgment: one keyed capture
        // per source event, with a pointer to material it never copies in
        // (111, 115, 215).
        Cmd::Capture { kind, source, by, manifest, name, root } => {
            // Through this host's own bindings: the capture is one write to the
            // state repository and one to the blueprints under the machinery's
            // prefix (111, 203, D8).
            let mut host =
                flywheel::host::Host::open(manifest, name, root.as_deref(), chrono::Utc::now())?;
            let defs = host.defs.clone();
            let at = host.now();
            let enumerated = host.store.with_world(|store, world| {
                flywheel_domain::adapters::run(
                    store,
                    world,
                    &defs,
                    &format!("{kind} {source}"),
                    by,
                    at,
                )
            })?;
            for key in &enumerated.keys {
                println!("{key}");
            }
            println!(
                "{} capture(s) written, {} signal(s): an enumerator reads nothing into signals (115)",
                enumerated.captures_written, enumerated.signals_written
            );
        }
        Cmd::RenderOrder { session_type, instruction_version, scenario, instructions } => {
            print!(
                "{}",
                flywheel::render_order::render_order(
                    session_type,
                    *instruction_version,
                    scenario,
                    instructions.as_deref()
                )?
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
                keep_places: false,
            };
            let paths = if paths.is_empty() { vec![PathBuf::from("conformance")] } else { paths.clone() };
            let report = conformance::run(&paths, &options)?;
            print!("{}", report.render());
            std::process::exit(report.exit_code());
        }
    }
    Ok(())
}
