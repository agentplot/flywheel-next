//! The conformance runner: load the definitions, seed a described state of the
//! stores, play a scenario's `when` steps against the real engine and assert
//! the `then` clauses (94).
//!
//! One state store is bound: the git-only profile against a bare repository on
//! the same computer, which is what a run with no live service standing up
//! proves against (92). The session binding (93) and the line-and-place
//! binding (93a) are what the profile leaves to be named (D15).

pub mod assertions;
pub mod drive;
pub mod hosts;
pub mod interpreter;
pub mod invariants;
pub mod phase;
pub mod record;
pub mod schema;
pub mod trace;

use anyhow::{Context, Result};
use flywheel_atoms::conformance::Requirement;
use std::path::{Path, PathBuf};

pub use record::RunRecord;
pub use schema::{Observations, Suite};

/// The state store binding a run uses. Phase 1 has one: the run with no live
/// service is the git-only profile against a bare repository on the same
/// computer (92). `tracker` is named in the scenario schema and is phase 2's
/// to bind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    GitOnly,
}

impl Profile {
    pub fn name(&self) -> &'static str {
        match self {
            Profile::GitOnly => "git-only",
        }
    }

    pub fn parse(s: &str) -> Result<Profile> {
        Ok(match s {
            "git-only" => Profile::GitOnly,
            "tracker" => anyhow::bail!(
                "`tracker` is the profile phase 2 binds; this release runs on git-only alone (92)"
            ),
            other => anyhow::bail!(
                "`{other}` is no profile; this release runs on git-only alone, \
                 a bare repository on this computer with no live service (92)"
            ),
        })
    }
}

/// What a run is configured with.
#[derive(Debug, Clone)]
pub struct RunOptions {
    pub profile: Profile,
    /// Every host in the scenario is a process of its own (D15, group 11).
    pub hosts_real: bool,
    /// Load the machine files from a directory instead of the embedded set (D2).
    pub definitions: Option<PathBuf>,
    /// Where the trace goes; `target/flywheel-trace/<profile>/` by default.
    pub trace: Option<PathBuf>,
    /// Whether to write a trace at all.
    pub tracing: bool,
    /// How far one tick moves the virtual clock; D7's 60-second sweep by
    /// default.
    pub interval: chrono::Duration,
    /// Leave the run's own directories where they are, for a caller that goes
    /// on using the store the run bound — the 390px pass answers the decision
    /// through the page after the steps are played (314, D15). The caller
    /// removes `Run::places` when it is done.
    pub keep_places: bool,
    /// How many of the scenario's actions to play; all of them by default.
    /// A caller that wants the instance as it stood at one moment plays up to
    /// it and stops, exactly as `flywheel scenario apply --through` does for a
    /// real instance. There is no stepping backwards (19.6).
    pub through: Option<usize>,
}

impl Default for RunOptions {
    fn default() -> Self {
        RunOptions {
            profile: Profile::GitOnly,
            hosts_real: false,
            definitions: None,
            trace: None,
            tracing: false,
            interval: chrono::Duration::seconds(60),
            keep_places: false,
            through: None,
        }
    }
}

impl RunOptions {
    /// What the bound implementations provide beyond the store binding. Phase 1
    /// records the workspace and scripts the sessions, so it provides neither
    /// (93a, 93b).
    pub fn provides(&self) -> Vec<Requirement> {
        // The workspace is recorded and the sessions are scripted in every
        // configuration this release runs; hosts are real under `--hosts real`.
        match self.hosts_real {
            true => vec![Requirement::RealHosts],
            false => vec![],
        }
    }

    /// The same run, with the hosts as processes of their own (D15).
    pub fn with_real_hosts(&self) -> RunOptions {
        RunOptions {
            hosts_real: true,
            ..self.clone()
        }
    }

    /// Hooks are honoured only when the runner holds the engine itself, which
    /// is the mode without `--hosts real` (D15).
    pub fn honours_hooks(&self) -> bool {
        !self.hosts_real
    }

    pub fn trace_dir(&self) -> PathBuf {
        self.trace.clone().unwrap_or_else(|| {
            PathBuf::from("target/flywheel-trace").join(self.profile.name())
        })
    }

    /// Where the run's own record is written, never beside the scenario files,
    /// which are the model's copy (D15).
    pub fn record_dir(&self) -> PathBuf {
        PathBuf::from("target/flywheel-run").join(self.profile.name())
    }
}

/// What became of one scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Passed,
    /// An assertion failed (exit 1).
    Failed,
    /// Invalid against the schema, or using a name nothing binds (exit 2).
    Invalid,
    /// The profile was refused (exit 3).
    Refused,
    /// A requirement the bound implementations do not provide (93a).
    Skipped,
    /// The scenario does not apply to this store binding.
    NotApplicable,
}

#[derive(Debug, Clone)]
pub struct Outcome {
    pub scenario: String,
    pub path: PathBuf,
    pub status: Status,
    /// One line per failure, in the failure format of D15.
    pub failures: Vec<String>,
    pub reason: Option<String>,
    pub trace: Option<PathBuf>,
}

impl Outcome {
    /// The name the acceptance table knows this scenario by: its file's, or its
    /// directory's where it is kept as one, and the name it declares where the
    /// outcome has no file.
    pub fn row(&self) -> String {
        Some(row_of(&self.path))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| self.scenario.clone())
    }

    /// The one line the run prints for this scenario.
    pub fn line(&self) -> String {
        let mark = match self.status {
            Status::Passed => "pass",
            Status::Failed => "FAIL",
            Status::Invalid => "INVALID",
            Status::Refused => "REFUSED",
            Status::Skipped => "skip",
            Status::NotApplicable => "n/a",
        };
        match &self.reason {
            Some(r) => format!("{mark:<8} {:<28} {r}", self.scenario),
            None => format!("{mark:<8} {}", self.scenario),
        }
    }
}

/// What a whole run came to.
#[derive(Debug, Clone, Default)]
pub struct RunReport {
    pub outcomes: Vec<Outcome>,
    /// What the run recorded of itself: the profile it bound and the hash of
    /// the machine files it ran (168, D2).
    pub record: RunRecord,
    /// The record as the store holds it, read back after it was written. What
    /// a reader with nothing running finds (167).
    pub recorded: Vec<flywheel_domain::records::RunEntry>,
    /// Why the profile was refused before any scenario played, where it was.
    pub refusal: Option<String>,
}

impl RunReport {
    /// 0 every scenario passed and every skip was for a stated requirement; 1
    /// an assertion failed; 2 a scenario is invalid or uses a name nothing
    /// binds; 3 the profile was refused (D15). A row the phase defers weighs on
    /// none of them: it is reported, and gates nothing until its phase opens
    /// (roadmap, phase gates).
    pub fn exit_code(&self) -> i32 {
        if self.refusal.is_some() || self.outcomes.iter().any(|o| o.status == Status::Refused) {
            return 3;
        }
        let gating = || self.outcomes.iter().filter(|o| !phase::deferred(&o.row()));
        if gating().any(|o| o.status == Status::Invalid) {
            return 2;
        }
        if gating().any(|o| o.status == Status::Failed) {
            return 1;
        }
        // A row the acceptance table lists that this run skipped, or found not
        // applicable, is a failure of the run: the phase accepts the row, so
        // a configuration under which it never plays proves nothing about it
        // (93a, tasks 2.15 and 11.4).
        if !self.listed_but_not_run().is_empty() {
            return 1;
        }
        0
    }

    /// The rows of the acceptance table this run held and did not play — each
    /// with the reason the run gave (93a).
    pub fn listed_but_not_run(&self) -> Vec<(String, String)> {
        self.outcomes
            .iter()
            .filter(|o| !matches!(o.status, Status::Passed | Status::Failed))
            .filter(|o| phase::accepted(&o.row()))
            .map(|o| {
                (
                    o.row(),
                    format!("{:?}: {}", o.status, o.reason.clone().unwrap_or_default()),
                )
            })
            .collect()
    }

    /// The rows the phase defers that failed or did not validate. Each is in
    /// the report with what it expected and what it got, and gates nothing
    /// until its phase opens (roadmap, phase gates).
    pub fn deferred_not_passing(&self) -> Vec<&Outcome> {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.status, Status::Failed | Status::Invalid))
            .filter(|o| phase::deferred(&o.row()))
            .collect()
    }

    pub fn ran(&self) -> Vec<&Outcome> {
        self.outcomes
            .iter()
            .filter(|o| matches!(o.status, Status::Passed | Status::Failed))
            .collect()
    }

    pub fn skipped(&self) -> Vec<&Outcome> {
        self.outcomes
            .iter()
            .filter(|o| o.status == Status::Skipped)
            .collect()
    }

    /// One line per scenario, then a summary.
    pub fn render(&self) -> String {
        let mut out = String::new();
        if let Some(refusal) = &self.refusal {
            out.push_str(refusal);
            out.push('\n');
            return out;
        }
        for o in &self.outcomes {
            out.push_str(&o.line());
            out.push('\n');
            for f in &o.failures {
                out.push_str(f);
                out.push('\n');
            }
        }
        let passed = self
            .outcomes
            .iter()
            .filter(|o| o.status == Status::Passed)
            .count();
        let failed = self
            .outcomes
            .iter()
            .filter(|o| o.status == Status::Failed)
            .count();
        out.push_str(&format!(
            "\n{passed} passed · {failed} failed · {} skipped · {} not applicable\n",
            self.skipped().len(),
            self.outcomes
                .iter()
                .filter(|o| o.status == Status::NotApplicable)
                .count()
        ));
        // The gate is the rows the acceptance table lists; a row the phase
        // defers that did not pass is named beside it and gates nothing
        // (roadmap, phase gates; 93a).
        let listed: Vec<&Outcome> = self
            .outcomes
            .iter()
            .filter(|o| phase::accepted(&o.row()))
            .collect();
        if !listed.is_empty() {
            let passed = listed.iter().filter(|o| o.status == Status::Passed).count();
            out.push_str(&format!("gate      {passed} of {} listed rows passed", listed.len()));
            for o in listed.iter().filter(|o| o.status == Status::Failed) {
                out.push_str(&format!(" · {} failed", o.row()));
            }
            for (row, reason) in self.listed_but_not_run() {
                out.push_str(&format!(" · {row} did not play ({reason})"));
            }
            out.push('\n');
        }
        let deferred = self.deferred_not_passing();
        if !deferred.is_empty() {
            let rows: Vec<String> = deferred
                .iter()
                .map(|o| match o.status {
                    Status::Failed => format!("{} failed", o.row()),
                    _ => format!("{} does not validate", o.row()),
                })
                .collect();
            out.push_str(&format!(
                "deferred  {} — each above with what it expected and what it got, gating nothing until its phase opens\n",
                rows.join(" · ")
            ));
        }
        // What the record holds, so a reader of the run sees what a reader of
        // the record will (167, D15).
        out.push_str(&format!(
            "profile {} · definitions {:016x}\n",
            self.record.profile, self.record.definitions_hash
        ));
        out
    }
}

/// Every scenario file under a path, in order. A directory yields the contract
/// set first, then the scenarios, because the contract admits the profile
/// before any domain definition loads (168).
/// The suite's own files, which sit beside the scenarios and are not ones.
const SUITE_FILES: &[&str] = &["observations.yaml"];

pub fn scenario_files(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    // A scenario is a directory — `<name>/scenario.yaml` with `bundle/` beside
    // it — and a single file is the same thing with nothing beside it, so a
    // directory named directly is one scenario and not a set of them.
    let own = flywheel_atoms::conformance::scenario_file(path);
    if own.is_file() {
        return Ok(vec![own]);
    }
    let mut contract = Vec::new();
    let mut scenarios = Vec::new();
    collect(path, &mut contract, &mut scenarios)?;
    contract.sort();
    scenarios.sort();
    contract.extend(scenarios);
    Ok(contract)
}

fn collect(dir: &Path, contract: &mut Vec<PathBuf>, scenarios: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .flatten()
    {
        let p = entry.path();
        if p.is_dir() {
            // `lamp` holds the toy machine, not scenarios; `fixtures` holds the
            // world's inputs; `bundle` holds one scenario's artifacts.
            if p.file_name().is_some_and(|f| {
                f == "lamp" || f == "fixtures" || f == flywheel_atoms::conformance::BUNDLE_DIR
            }) {
                continue;
            }
            // A directory holding `scenario.yaml` is one scenario, whatever
            // else is beside it.
            let own = flywheel_atoms::conformance::scenario_file(&p);
            if own.is_file() {
                scenarios.push(own);
                continue;
            }
            collect(&p, contract, scenarios)?;
        } else if p.extension().is_some_and(|x| x == "yaml") {
            // The suite's own files are not scenarios: `observations.yaml` is
            // the registry every `then.state_store` key is bound in (D15).
            if p.file_name().is_some_and(|f| SUITE_FILES.iter().any(|own| f == *own)) {
                continue;
            }
            if p.components().any(|c| c.as_os_str() == "contract") {
                contract.push(p);
            } else {
                scenarios.push(p);
            }
        }
    }
    Ok(())
}

/// The scenarios the 390px pass runs: every one that carries an operator's
/// response (314).
///
/// The set is read off the scenario files and no list of it is kept anywhere:
/// a scenario that gains a response step joins the pass, and one that loses it
/// leaves, with nothing to remember to edit (D15).
pub fn phone_set(path: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for file in scenario_files(path)? {
        let Ok((scenario, _)) = flywheel_atoms::conformance::load(&file) else {
            continue;
        };
        if scenario.carries_response() {
            out.push(file);
        }
    }
    Ok(out)
}

/// Run every scenario a path names.
///
/// The machine files are hashed before anything plays and the hash is compared
/// with `definitions/`: a run against a directory the repository does not hold
/// proves nothing about the model, so the profile is refused and no scenario
/// plays (168, D2).
pub fn run(paths: &[PathBuf], options: &RunOptions) -> Result<RunReport> {
    let mut report = RunReport {
        record: RunRecord::of(options)?,
        ..Default::default()
    };
    if let Some(refusal) = report.record.refusal() {
        report.refusal = Some(refusal);
        return Ok(report);
    }
    for path in paths {
        for file in scenario_files(path)? {
            // A row that declares the mode it needs is run in that mode, so a
            // set run in process still plays its real-host rows and a skip
            // means the row named nothing this run could provide (93a, D15).
            let declared_real_hosts = flywheel_atoms::conformance::load(&file)
                .map(|(scenario, _)| scenario.requires.contains(&Requirement::RealHosts))
                .unwrap_or(false);
            let outcome = match declared_real_hosts && !options.hosts_real {
                true => run_one(&file, &options.with_real_hosts()),
                false => run_one(&file, options),
            };
            report.record.saw(&outcome);
            report.outcomes.push(outcome);
        }
    }
    // The run's own record, written through the store the run bound: the
    // profile, the definitions hash, what ran, what was skipped and why, and
    // every failure (79–82, 93a, 167, D15).
    report.recorded = record::write(&report.record, options)
        .map(|store| store.run_record())
        .unwrap_or_default();
    Ok(report)
}

/// Run one scenario. Every way it can fail is an outcome, never a panic: the
/// run prints one line per scenario whatever happened.
pub fn run_one(path: &Path, options: &RunOptions) -> Outcome {
    let name = row_of(path);
    let invalid = |reason: String| Outcome {
        scenario: name.clone(),
        path: path.to_path_buf(),
        status: Status::Invalid,
        failures: vec![],
        reason: Some(reason),
        trace: None,
    };

    let suite = match Suite::for_scenario(path) {
        Ok(s) => s,
        Err(e) => return invalid(format!("{e:#}")),
    };
    let (scenario, document) = match flywheel_atoms::conformance::load(path) {
        Ok(x) => x,
        Err(e) => return invalid(format!("{e:#}")),
    };
    if let Err(e) = suite.validate(&document) {
        return invalid(format!("{e:#}"));
    }
    if let Err(e) = suite.check_names(&scenario, path) {
        return invalid(format!("{e:#}"));
    }

    if !scenario.runs_on(options.profile.name()) {
        return Outcome {
            scenario: scenario.scenario.clone(),
            path: path.to_path_buf(),
            status: Status::NotApplicable,
            failures: vec![],
            reason: Some(format!("profiles: {:?}", scenario.profiles)),
            trace: None,
        };
    }
    // `requires:` is read from the data; no list of skips is kept outside it
    // (93a, D15).
    if let Some(unmet) = scenario.unmet(&options.provides()) {
        return Outcome {
            scenario: scenario.scenario.clone(),
            path: path.to_path_buf(),
            status: Status::Skipped,
            failures: vec![],
            reason: Some(unmet.reason().to_string()),
            trace: None,
        };
    }

    match drive::play(&scenario, path, &suite, options) {
        Ok(run) => {
            let failures = assertions::check(&scenario, &run, &suite);
            let trace_path = if options.tracing {
                trace::write(&scenario, &run, options).ok()
            } else {
                None
            };
            let failures: Vec<String> = failures
                .iter()
                .map(|f| f.render(&scenario, trace_path.as_deref()))
                .collect();
            // A run's directories stand while it is asserted — the run record
            // and every object a clause reads are files under them — and go
            // once every assertion has read them (167, D15).
            if !run.keep_places {
                let _ = std::fs::remove_dir_all(&run.places);
            }
            Outcome {
                scenario: scenario.scenario.clone(),
                path: path.to_path_buf(),
                status: if failures.is_empty() {
                    Status::Passed
                } else {
                    Status::Failed
                },
                failures,
                reason: None,
                trace: trace_path,
            }
        }
        Err(e) => invalid(format!("{e:#}")),
    }
}

/// Everything one run of one scenario produced.
pub struct Run {
    pub ticks: Vec<crate::runner::TickRecord>,
    /// The decisions standing after each step, by step number counting from 1.
    pub decisions_after: Vec<Vec<crate::runner::DecisionRecord>>,
    pub runtime: crate::Runtime,
    /// What the profile answered for each observation the scenario asserts.
    pub observations: std::collections::BTreeMap<String, serde_json::Value>,
    pub skipped_steps: Vec<String>,
    /// The store's write count when the first step began, so `writes:` counts
    /// what the steps did and not what seeding put in place.
    pub writes_at_start: u64,
    /// The tail as a sink whose mark is the start of the run reads it, at each
    /// step boundary. Derived like the decisions and stored nowhere (14, 15).
    pub tail_after: Vec<Vec<flywheel_engine::runtime::TailEntry>>,
    /// The status view at each step boundary, so a scenario asserting what the
    /// view showed after a numbered step is answered from the view as it stood
    /// then (141, 143, 146).
    pub status_after: Vec<serde_json::Value>,
    /// The profile the run bound, which is the binding a scenario about
    /// bindings is asserted against (140).
    pub profile: &'static str,
    /// How many dictations this scenario has made, so each takes a delivery id
    /// of its own and a repeat is recognised (137).
    pub dictations: u32,
    /// How many ticks the runner itself made. Under `--hosts real` this stays
    /// zero: the hosts are processes, the runner holds no engine, and every
    /// transition it asserts was read from the store (D15, 134, 162, I15).
    pub engine_ticks: usize,
    /// When each host had no route, by the run's own clock: the moment it was
    /// cut and the moment it came back, where it did (151, D4a). What a
    /// scenario asks about what a host did while it was offline is answered
    /// against these.
    pub offline: std::collections::BTreeMap<String, Vec<(chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)>>,
    /// The run's own directories: the state repository it bound and the places
    /// it made. They outlive the run itself, because what a scenario asserts is
    /// read from the state repository under them; they go when the run does,
    /// unless the caller asked to keep them.
    pub places: std::path::PathBuf,
    /// Whether to leave those directories behind for a person to look at.
    pub keep_places: bool,
    /// Every host's run record on the shared line, read once when the last step
    /// had been played and every host had stopped. What a scenario asserts is
    /// read from here rather than from the repository afterwards: a record the
    /// runner could not read is a broken run, not an empty one (79, 167, D15).
    pub record: Vec<flywheel_domain::records::RunEntry>,
    /// How many entries of each host's run record have been read into `ticks`,
    /// so each pass reads only what was written since the last one. One host
    /// appends to its own file and never to another's, which is why the mark is
    /// per file and not one count over the whole set.
    pub read_at: std::collections::BTreeMap<String, usize>,
}

/// A scenario's name as the acceptance table and this repository's tasks give
/// it: the file's name without its extension. A scenario kept as a directory is
/// named for the directory, `scenario` being the file's name in every one of
/// them and naming nothing.
fn row_of(path: &Path) -> String {
    let named = match path.file_name().is_some_and(|f| f == flywheel_atoms::conformance::SCENARIO_FILE) {
        true => path.parent().unwrap_or(path),
        false => path,
    };
    named
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// The listed rows the run did not play. A scenario that runs against no
/// configuration at all is a failure, never a silent pass (D15). The acceptance
/// table is the caller's; this is the check it uses.
pub fn every_listed_scenario_ran(listed: &[String], report: &RunReport) -> Vec<String> {
    listed
        .iter()
        .filter(|name| {
            !report
                .ran()
                .iter()
                .any(|o| &&o.scenario == name || &&o.row() == name)
        })
        .cloned()
        .collect()
}
