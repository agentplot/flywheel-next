//! `flywheel scenario apply <scenario> --through <n>`: play a scenario's
//! actions into a real instance and stop, so the instance stands at that
//! moment (`design/flywheel-next/scenarios/storefront.md`).
//!
//! There is one scenario mechanism and not two. The YAML the acceptance set
//! already uses is the YAML a demo runs on: `given:` is the moment, which is
//! what `seed` already puts into a real state store, and `actions:` is what
//! happens next. A test scenario that asserts only a moment has no actions; a
//! demo with no tour copy is a long test.
//!
//! What runs here is the machinery itself. Nothing sets state: a capture
//! arrives through the adapter or the capture tool, an answer goes through the
//! response record, and a session's delivery is a file copied into the
//! repository followed by the reporting command a real session runs (67, 93,
//! 111, 125, 193). Between actions the host ticks until it settles, so the
//! instance stands wherever the machinery took it and never where a scenario
//! put it. Where an action cannot be expressed that way, it fails — a hole in
//! the machinery is the finding, never something to accommodate.
//!
//! There is no stepping backwards. To see an earlier moment, apply to an
//! earlier action number into a fresh instance.

use crate::host::Host;
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Duration, Utc};
use flywheel_atoms::conformance::{Action, CaptureAction, ResponseStep, SessionAction};
use flywheel_atoms::{Records, Scope};
use flywheel_scenario::conformance::Suite;
use std::path::{Path, PathBuf};

/// The instance a demo runs on, and where its parts are.
pub struct Instance {
    pub name: String,
    pub host: String,
    /// The directory everything sits under: the manifest, the git host's bare
    /// repositories and this host's root.
    pub under: PathBuf,
    pub manifest: PathBuf,
    pub root: PathBuf,
}

impl Instance {
    /// This host's checkout of a repository the instance tracks (205, 218).
    pub fn checkout(&self, repository: &str) -> PathBuf {
        self.root.join(&self.name).join(repository)
    }

    pub fn open(&self, at: DateTime<Utc>) -> Result<Host> {
        Host::open(&self.manifest, &self.host, None, at)
    }

    /// Put a file on a repository's shared line through this host's checkout.
    ///
    /// What a session delivers is committed where a real session's landing
    /// would leave it, because the machinery reads a repository at its line and
    /// never at a working tree (167).
    pub fn commit(&self, repository: &str, path: &str, body: &str, reason: &str) -> Result<()> {
        let read = flywheel_world_host::Manifest::read(&self.manifest)?;
        let line = read
            .all_repositories()
            .into_iter()
            .find(|(name, _)| name == repository)
            .map(|(_, r)| r.shared_line.clone())
            .ok_or_else(|| {
                anyhow!(
                    "the manifest tracks no repository `{repository}`; a delivery lands in one \
                     the instance tracks (199, 205)"
                )
            })?;
        let checkout = self.checkout(repository);
        if !checkout.join(".git").is_dir() {
            bail!(
                "this host holds no checkout of `{repository}` at {}: it joins by one command \
                 and never by hand (205)",
                checkout.display()
            );
        }
        flywheel_world_host::git::commit_file(
            &flywheel_world_host::git::Repo::at(&checkout),
            &line,
            path,
            body,
            reason,
        )
    }
}

/// The key the operator placed, as a fresh instance for a demo places it. A
/// demo's git host is a directory of bare repositories on this computer, so
/// there is nothing to authenticate to; the manifest still names where the key
/// is and never the key (204, 207, 207a).
const DEMO_KEY_FROM: &str = "FLYWHEEL_DEMO_KEY";

/// Make the instance a scenario is applied into: the manifest, the blueprints
/// and state repositories, and this host registered (204).
///
/// It is `flywheel init` and nothing else — a demo instance is a real instance
/// or it proves nothing.
pub fn make_instance(
    under: &Path,
    name: &str,
    host: &str,
    repositories: &[String],
    at: DateTime<Utc>,
) -> Result<Instance> {
    std::fs::create_dir_all(under)
        .with_context(|| format!("making {}", under.display()))?;
    // Absolute from here on: a bare repository's remote is written into the
    // manifest and read back by a host whose working directory is its own, so a
    // relative one would name nothing (204, 205).
    let under = &under
        .canonicalize()
        .with_context(|| format!("resolving {}", under.display()))?;
    let instance = Instance {
        name: name.to_string(),
        host: host.to_string(),
        under: under.to_path_buf(),
        manifest: under.join(flywheel_world_host::manifest::FILE),
        root: under.join("root"),
    };
    let report = crate::init::run(crate::init::Init {
        instance: name.to_string(),
        host: host.to_string(),
        root: instance.root.clone(),
        git_host: under.join("git-host"),
        app: "demo".into(),
        app_key_from: DEMO_KEY_FROM.into(),
        // The operator placed it; a demo says so directly rather than writing
        // into the process's own environment, which is one table for the whole
        // process (207a).
        app_key: Some("the operator placed this".into()),
        address: format!("http://{host}.local"),
        manifest: instance.manifest.clone(),
        // The repositories the scenario names, tracked by the instance before
        // anything is seeded into it. A seed that put an object naming a
        // repository the instance does not track left that object uncovered by
        // every host's declaration — correctly reported as a decision under
        // attention, but a decision about the seed rather than about the work
        // (149, 205, 206). `init` is the one place a repository comes into
        // existence, here as for an operator at the command line.
        repositories: repositories.to_vec(),
        curation: None,
        at,
    })
    .context("making the instance a scenario is applied into")?;
    if report.state != "hosted" {
        bail!(
            "the instance stopped at `{}`{}: a scenario is applied into a hosted instance (204)",
            report.state,
            report
                .attention
                .map(|a| format!(", awaiting {a}"))
                .unwrap_or_default()
        );
    }
    // The host clones what the manifest names and checks the layout, which is
    // how a host comes to hold a checkout at all: by one command and never by
    // hand (205, 222).
    let read = flywheel_world_host::Manifest::read(&instance.manifest)?;
    let mut world = flywheel_world_host::HostWorld::open(read, host)?;
    flywheel_world_host::join::join(&mut world).context("this host joining the instance")?;
    let differences = flywheel_world_host::join::doctor(&world);
    if !differences.is_empty() {
        bail!(
            "the layout is not what the manifest says: {}",
            differences.join("; ")
        );
    }
    Ok(instance)
}

/// What an apply left behind.
#[derive(Debug)]
pub struct Applied {
    /// Where the instance is, so a host can be served over it.
    pub state: PathBuf,
    pub manifest: PathBuf,
    /// How many actions were played.
    pub through: usize,
    /// How many the scenario holds.
    pub actions: usize,
    /// The moment the instance now stands at.
    pub at: DateTime<Utc>,
    /// One line per action played: what it was, and the tour copy where the
    /// scenario carries any.
    pub lines: Vec<String>,
    /// The objects the seed put in place.
    pub seeded: usize,
}

/// The instance a scenario is applied into is named for the scenario: lower
/// case, and one word, because it is a directory name and a repository prefix.
fn instance_name(scenario: &str) -> String {
    let name: String = scenario
        .to_lowercase()
        .chars()
        .map(|c| match c.is_ascii_alphanumeric() {
            true => c,
            false => '-',
        })
        .collect();
    match name.trim_matches('-').is_empty() {
        true => "demo".to_string(),
        false => name.trim_matches('-').to_string(),
    }
}

/// How far one action moves the clock. The actions are the clock: a demo that
/// waited out a lease in wall time would take a day, and a test that slept
/// would be a test that sleeps (D15).
pub const PER_ACTION: i64 = 60;

/// Play a scenario's `given:` and its first `through` actions into a fresh
/// instance under `into`.
pub fn apply(
    scenario: &Path,
    into: &Path,
    through: Option<usize>,
    at: DateTime<Utc>,
    per_action: Duration,
) -> Result<Applied> {
    let suite = Suite::for_scenario(scenario)
        .with_context(|| format!("the suite closing {}", scenario.display()))?;
    let (read, document) = flywheel_atoms::conformance::load(scenario)?;
    suite
        .validate(&document)
        .with_context(|| format!("{}", scenario.display()))?;
    let actions = read.actions()?;
    let through = through.unwrap_or(actions.len());
    if through > actions.len() {
        bail!(
            "{} has {} actions; there is no action {through}",
            scenario.display(),
            actions.len()
        );
    }
    let bundle = flywheel_atoms::conformance::bundle_of(scenario);

    // The actions are things that happened, and the last of them happened now.
    // So the run starts as far back as its actions reach and advances forward
    // to the present, rather than starting at the present and advancing past
    // it.
    //
    // The difference is the whole of what a served host reads. There is one
    // instance and one state, and every age the machinery judges by — a lease's
    // renewal, a stall window, a cadence's last run, "seen at" — is `now` less
    // the moment a state was entered. A run that ended ahead of the clock left
    // every one of those moments in the future of the host that picked the
    // instance up, so its ages were negative and its history read backwards:
    // the run record, which is what a person reads to know what happened (79,
    // 167), could not be read as a sequence at all. Ending at the present
    // instead leaves a thirty-action scenario standing with its first capture
    // half an hour old, which is what a demo wants of it, and leaves a host
    // that starts afterwards carrying the same clock forward (D15, 231).
    let start = at - per_action * (through as i32);

    // The instance is named for the scenario, so the singletons an instance
    // has — `curation/<instance>` among them — are the objects the scenario
    // names, whichever binding plays it (110, 218).
    let instance = make_instance(
        into,
        &instance_name(&read.scenario),
        "local",
        &read.repositories_named(),
        start,
    )?;
    let mut host = instance.open(start)?;
    host.set_now(start);
    host.declare().context("the host declaring itself")?;

    // The moment the scenario describes, put into the real state store through
    // the store's own write path — the `given:` half, which is what `flywheel
    // host seed` already does and this goes on from (125, 193).
    let declaration = host.declaration.clone();
    let seeded = crate::seed::from_conformance(&mut host.store.git, &read, start, &declaration)
        .context("seeding the scenario's given: state")?;
    settle(&mut host)?;

    let mut lines = Vec::new();
    for (index, action) in actions.iter().take(through).enumerate() {
        let number = index + 1;
        // The clock advances with the actions and never with wall time, so
        // things age, leases come due and "seen 07:40" means something without
        // anyone waiting (D15).
        host.set_now(host.now() + per_action);
        let what = play(&mut host, &instance, action, bundle.as_deref(), &suite)
            .with_context(|| format!("action {number}"))?;
        settle(&mut host)?;
        let mut line = format!("{number:>3}. {what}");
        if let Some(copy) = read.tour_line(number) {
            line.push_str(&format!("\n     {copy}"));
        }
        lines.push(line);
    }

    Ok(Applied {
        state: instance.checkout("flywheel-state"),
        manifest: instance.manifest.clone(),
        through,
        actions: actions.len(),
        at: host.now(),
        lines,
        seeded: seeded.objects,
    })
}

/// Run the machinery until it stops moving. A cascade is what the tick already
/// does; this is the caller waiting it out rather than the loop's poll, so a
/// demo advances on its actions and a test pays for nothing it did not need
/// (D6, D7, 78).
fn settle(host: &mut Host) -> Result<()> {
    // A bound rather than a hope: a machine that keeps moving forever is a
    // defect, and reporting it beats hanging.
    const PASSES: usize = 64;
    for pass in 0..PASSES {
        // Every pass reads everything. `once` only ticks what the notice named
        // unless the sweep is due, which is the loop's own economy against a
        // clock that is running (D6, D7); here the clock stands still between
        // actions, so after the first pass the sweep is never due again and a
        // cascade that needed a third pass stopped where it was. An action
        // left the elaboration mid-finish, with its place merged and the
        // object still `finishing`, and the moment the operator stepped to was
        // a moment the machinery had not arrived at. A sweep that moves
        // nothing writes nothing (78), so reading everything costs the reads
        // and no more.
        host.last_sweep = None;
        host.once()?;
        if !host.progressed() {
            return Ok(());
        }
        if pass + 1 == PASSES {
            bail!(
                "the machinery was still moving after {PASSES} passes; a cascade that does not \
                 settle is a defect, not a moment to stand at (78)"
            );
        }
    }
    Ok(())
}

/// Play one action. Each is one thing a real actor does, and each runs the
/// machinery's own path for it.
fn play(
    host: &mut Host,
    instance: &Instance,
    action: &Action,
    bundle: Option<&Path>,
    suite: &Suite,
) -> Result<String> {
    match action {
        Action::Capture(capture) => arrived(host, capture, suite),
        Action::Response(response) => answered(host, response),
        Action::Session(session) => delivered(host, instance, session, bundle),
    }
}

/// Something arrived. An adapter enumerated it, or a person typed or forwarded
/// it; both are the real path and neither writes an object by hand (111, 115,
/// 215, D13).
fn arrived(host: &mut Host, capture: &CaptureAction, suite: &Suite) -> Result<String> {
    let by = capture.by.clone().unwrap_or_else(|| "operator".into());
    let at = host.now();
    let defs = host.defs.clone();
    if let Some(command) = &capture.adapter {
        // The material stays where it is and the capture cites it, so the
        // enumerator is handed the pointer it was named with; a path that names
        // a fixture is resolved so a demo's transcript is a file that exists
        // (111).
        let argument = command.split_whitespace().last().unwrap_or_default();
        let _ = suite.resolve_fixture(argument, "");
        let enumerated = host.store.with_world(|store, world| {
            flywheel_domain::adapters::run(store, world, &defs, command, &by, at)
        })?;
        return Ok(format!(
            "capture · {command} · {} written",
            enumerated.captures_written
        ));
    }
    let text = capture
        .text
        .clone()
        .ok_or_else(|| anyhow!("a capture names an adapter or its text"))?;
    let source = capture.source.clone().unwrap_or_else(|| "page".into());
    let call = flywheel_surface::catalogue::Call {
        tool: "capture".into(),
        args: [
            ("text".to_string(), serde_json::json!(text)),
            ("source".to_string(), serde_json::json!(source)),
        ]
        .into_iter()
        .collect(),
        by: by.clone(),
        delivery: source.clone(),
        delivery_id: None,
        event_key: capture.key.clone(),
        proposed_by: None,
    };
    let outcome = host
        .store
        .with_world(|store, world| flywheel_surface::catalogue::call(store, world, &defs, &call))?;
    Ok(format!(
        "capture · {source} · \"{}\" · {}",
        text, outcome.id
    ))
}

/// The operator answered a numbered decision. The number is the register's, so
/// the decision is resolved through the rail as it stands at this moment and a
/// scenario naming nothing standing fails rather than passing silently (15,
/// D15).
fn answered(host: &mut Host, response: &ResponseStep) -> Result<String> {
    let defs = host.defs.clone();
    let rail = flywheel_domain::commands::rail(&mut host.store, &defs)?;
    let number = match (&response.decision, response.number) {
        (_, Some(number)) => number,
        (Some(named), None) => {
            let found = rail
                .iter()
                .find(|d| d.id == *named || format!("{}/{}", d.object, d.kind) == *named)
                .ok_or_else(|| {
                    anyhow!(
                        "`{named}` names no standing decision; standing are {}",
                        match rail.is_empty() {
                            true => "none".to_string(),
                            false => rail
                                .iter()
                                .map(|d| format!("{}/{}", d.object, d.kind))
                                .collect::<Vec<_>>()
                                .join(", "),
                        }
                    )
                })?;
            found
                .number
                .ok_or_else(|| anyhow!("`{named}` stands with no number; the register gave none"))?
        }
        (None, None) => bail!("a response names the decision it answers, or its number"),
    };
    let by = response.by.clone().unwrap_or_else(|| "operator".into());
    flywheel_domain::commands::respond(&mut host.store, &defs, number, &response.answer, &by)?;
    Ok(format!("response · {number} → {}", response.answer))
}

/// A session delivered and exited.
///
/// There is nothing to run the session, so the object is standing where a real
/// one would be working. What it delivered is copied out of the bundle into the
/// path the real session would have written — a real file at a real path from
/// this moment on — and the exit is written through the same reporting path the
/// command writes through, so the machinery reads a thread entry it cannot tell
/// from a real session's (67, 93).
fn delivered(
    host: &mut Host,
    instance: &Instance,
    session: &SessionAction,
    bundle: Option<&Path>,
) -> Result<String> {
    let id = session_of(host, &session.object)?;
    let mut delivered = Vec::new();
    for (from, to) in &session.deliver {
        let bundle = bundle.ok_or_else(|| {
            anyhow!(
                "the session delivers `{from}`, and there is no `bundle/` beside the scenario \
                 to deliver it from"
            )
        })?;
        let source = bundle.join(from);
        if !source.is_file() {
            bail!(
                "the bundle holds no `{from}`: a session delivers what the scenario's bundle \
                 carries, and nothing is invented for it"
            );
        }
        let (repository, under) = to.split_once('/').ok_or_else(|| {
            anyhow!(
                "`{to}` names no repository; a delivery is written at `<repository>/<path>`, \
                 which is where the real session's place would have put it"
            )
        })?;
        // Committed on the repository's shared line as the session, because
        // that is what a real session's landing does and because the machinery
        // reads a repository at its line and not at a working tree (167, 203).
        // It is a session's write and not the machinery's, so the reason names
        // the session rather than the prefix rule.
        instance.commit(
            repository,
            under,
            &std::fs::read_to_string(&source)
                .with_context(|| format!("reading {}", source.display()))?,
            &format!("{under}\n\nreason: delivered by {id}"),
        )?;
        delivered.push(to.clone());
    }

    let by = "operator".to_string();
    let at = host.now();
    let report = match session.exit.as_str() {
        "refused" => flywheel_domain::report::Report::Refuse {
            reason: session
                .question
                .clone()
                .unwrap_or_else(|| "the session refused the work".into()),
        },
        kind => flywheel_domain::report::Report::Exit {
            kind: kind.to_string(),
            deliverables: delivered.clone(),
            question: session.question.clone(),
            text: session.verdict.clone(),
        },
    };
    let outcome = flywheel_domain::report::write_report(&mut host.store, &id, &by, at, &report)?;
    for offer in &session.offers {
        flywheel_domain::report::write_report(
            &mut host.store,
            &id,
            &by,
            at,
            &flywheel_domain::report::Report::Offer {
                kind: offer.kind.clone(),
                document: offer.document.clone(),
            },
        )?;
    }
    let said = match &outcome {
        flywheel_domain::report::Reported::Accepted(_) => session.exit.clone(),
        flywheel_domain::report::Reported::Refused { reason, .. } => {
            format!("{} (refused: {reason})", session.exit)
        }
    };
    Ok(format!(
        "session · {id} · {said}{}",
        match delivered.is_empty() {
            true => String::new(),
            false => format!(" · {}", delivered.join(", ")),
        }
    ))
}

/// The session standing on an object: the one this host started and that has
/// not reported.
///
/// A scenario names the object because that is what its author is thinking
/// about; two sessions standing on one object is the scenario being ambiguous
/// rather than the machinery being wrong, so both are named and the action
/// fails.
fn session_of(host: &Host, object: &str) -> Result<String> {
    let under = format!("{}{object}/", flywheel_sessions_operator::session_fact(""));
    let standing: Vec<String> = host
        .store
        .list_records(&Scope::All)?
        .into_iter()
        .filter(|o| o.id.starts_with(&under))
        .filter(|o| !o.record.get("ended_at").is_some_and(|v| !v.is_null()))
        .map(|o| o.id.trim_start_matches(&flywheel_sessions_operator::session_fact("")).to_string())
        .collect();
    match standing.as_slice() {
        [one] => Ok(one.clone()),
        [] => bail!(
            "no session stands on `{object}`: a session delivers where the machinery started \
             one, and the machinery started none here. That is a hole in the machinery and \
             not something the scenario may set (125, 193)"
        ),
        many => bail!(
            "{} sessions stand on `{object}` — {}; the scenario says which delivers",
            many.len(),
            many.join(", ")
        ),
    }
}
