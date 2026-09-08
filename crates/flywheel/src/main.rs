use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
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
    /// Serve the page.
    Serve { #[arg(long, default_value = "4242")] port: u16 },
    /// The conformance suite: load the definitions, seed the stores, play the
    /// steps against the real engine and assert the `then` clauses (94).
    Scenario {
        #[command(subcommand)]
        cmd: ScenarioCmd,
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

fn print_rail(rt: &mut Runtime) {
    let d = rt.decisions();
    let count = d.iter().filter(|x| x.group != "attention").count();
    println!("PLAN · {} · tick {} · {} decisions", rt.store.now.format("%Y-%m-%d %H:%M"), rt.store.tick, count);
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
        Cmd::Seed { scenario: path, drive } => {
            let defs = flywheel_engine::load::load_dir(&cli.defs)?;
            let sc = scenario::load(path)?;
            let mut rt = scenario::seed(defs, &sc);
            if *drive { scenario::drive(&mut rt, &sc); }
            flywheel_scenario::save(&rt.store, &cli.state)?;
            println!("seeded {} · {} objects", sc.scenario, rt.store.objects.len());
            print_rail(&mut rt);
        }
        Cmd::Tick { n } => {
            let mut rt = open(&cli)?;
            let fired = if *n == 0 { rt.settle(50) } else { (0..*n).map(|_| rt.tick()).sum() };
            flywheel_scenario::save(&rt.store, &cli.state)?;
            println!("{fired} transitions");
            print_rail(&mut rt);
        }
        Cmd::Respond { number, answer } => {
            let mut rt = open(&cli)?;
            rt.respond(*number, &answer.join(" "), "cli");
            let fired = rt.settle(50);
            flywheel_scenario::save(&rt.store, &cli.state)?;
            println!("{fired} transitions");
            print_rail(&mut rt);
        }
        Cmd::Dictate { object, answer } => {
            let mut rt = open(&cli)?;
            rt.dictate(object, &answer.join(" "), "cli");
            let fired = rt.settle(50);
            flywheel_scenario::save(&rt.store, &cli.state)?;
            println!("{fired} transitions");
            print_rail(&mut rt);
        }
        Cmd::Rail => { let mut rt = open(&cli)?; print_rail(&mut rt); }
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
        Cmd::Serve { port } => {
            let rt = open(&cli)?;
            flywheel_surface::serve(rt, cli.state.clone(), *port).await?;
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn _unused(_: Store) {}
