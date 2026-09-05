use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use flywheel_scenario::{scenario, Runtime, Store};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "flywheel", about = "flywheel next — prototype: seed a scenario, tick the machinery, answer decisions, serve the plan")]
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
    /// Print the plan.
    Plan,
    /// Print the last N log lines.
    Log { #[arg(default_value = "30")] n: usize },
    /// Serve the plan page.
    Serve { #[arg(long, default_value = "4242")] port: u16 },
    /// Dump an object's configuration and record.
    Obj { id: String },
    /// Evaluate one evidence name for an object (and optional region path).
    Ev { object: String, name: String, #[arg(default_value = "life")] region: String },
}

fn open(cli: &Cli) -> Result<Runtime> {
    let defs = flywheel_engine::load::load_dir(&cli.defs)?;
    let store = flywheel_scenario::load(&cli.state).with_context(|| "no store; run `flywheel seed <scenario>` first")?;
    Ok(Runtime::new(defs, store))
}

fn print_plan(rt: &mut Runtime) {
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
            print_plan(&mut rt);
        }
        Cmd::Tick { n } => {
            let mut rt = open(&cli)?;
            let fired = if *n == 0 { rt.settle(50) } else { (0..*n).map(|_| rt.tick()).sum() };
            flywheel_scenario::save(&rt.store, &cli.state)?;
            println!("{fired} transitions");
            print_plan(&mut rt);
        }
        Cmd::Respond { number, answer } => {
            let mut rt = open(&cli)?;
            rt.respond(*number, &answer.join(" "), "cli");
            let fired = rt.settle(50);
            flywheel_scenario::save(&rt.store, &cli.state)?;
            println!("{fired} transitions");
            print_plan(&mut rt);
        }
        Cmd::Dictate { object, answer } => {
            let mut rt = open(&cli)?;
            rt.dictate(object, &answer.join(" "), "cli");
            let fired = rt.settle(50);
            flywheel_scenario::save(&rt.store, &cli.state)?;
            println!("{fired} transitions");
            print_plan(&mut rt);
        }
        Cmd::Plan => { let mut rt = open(&cli)?; print_plan(&mut rt); }
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
        Cmd::Serve { port } => {
            let rt = open(&cli)?;
            flywheel_surface::serve(rt, cli.state.clone(), *port).await?;
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn _unused(_: Store) {}
