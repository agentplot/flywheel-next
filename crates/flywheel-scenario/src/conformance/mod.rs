//! The conformance runner: load the definitions, seed a described state of the
//! stores, play a scenario's `when` steps against the real engine and assert
//! the `then` clauses (94).
//!
//! The same runner and the same files, byte-identical, run against the stand-in
//! store and against the git-only profile; only the store binding, the session
//! binding (93) and the line-and-place binding (93a) differ (D15).

pub mod assertions;
pub mod drive;
pub mod hosts;
pub mod interpreter;
pub mod schema;
pub mod trace;

use anyhow::{Context, Result};
use flywheel_atoms::conformance::Requirement;
use std::path::{Path, PathBuf};

pub use schema::{Observations, Suite};

/// The state store binding a run uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    StandIn,
    GitOnly,
    Tracker,
}

impl Profile {
    pub fn name(&self) -> &'static str {
        match self {
            Profile::StandIn => "stand-in",
            Profile::GitOnly => "git-only",
            Profile::Tracker => "tracker",
        }
    }

    pub fn parse(s: &str) -> Result<Profile> {
        Ok(match s {
            "stand-in" => Profile::StandIn,
            "git-only" => Profile::GitOnly,
            "tracker" => Profile::Tracker,
            other => anyhow::bail!("`{other}` is no profile; they are stand-in, git-only, tracker"),
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
}

impl Default for RunOptions {
    fn default() -> Self {
        RunOptions {
            profile: Profile::StandIn,
            hosts_real: false,
            definitions: None,
            trace: None,
            tracing: false,
            interval: chrono::Duration::seconds(60),
        }
    }
}

impl RunOptions {
    /// What the bound implementations provide beyond the store binding. Phase 1
    /// records the workspace and scripts the sessions, so it provides neither
    /// (93a, 93b).
    pub fn provides(&self) -> Vec<Requirement> {
        vec![]
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
}

impl RunReport {
    /// 0 every scenario passed and every skip was for a stated requirement; 1
    /// an assertion failed; 2 a scenario is invalid or uses a name nothing
    /// binds; 3 the profile was refused (D15).
    pub fn exit_code(&self) -> i32 {
        if self.outcomes.iter().any(|o| o.status == Status::Refused) {
            return 3;
        }
        if self.outcomes.iter().any(|o| o.status == Status::Invalid) {
            return 2;
        }
        if self.outcomes.iter().any(|o| o.status == Status::Failed) {
            return 1;
        }
        0
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
        out
    }
}

/// Every scenario file under a path, in order. A directory yields the contract
/// set first, then the scenarios, because the contract admits the profile
/// before any domain definition loads (168).
pub fn scenario_files(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
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
            // world's inputs.
            if p.file_name().is_some_and(|f| f == "lamp" || f == "fixtures") {
                continue;
            }
            collect(&p, contract, scenarios)?;
        } else if p.extension().is_some_and(|x| x == "yaml") {
            if p.components().any(|c| c.as_os_str() == "contract") {
                contract.push(p);
            } else {
                scenarios.push(p);
            }
        }
    }
    Ok(())
}

/// Run every scenario a path names.
pub fn run(paths: &[PathBuf], options: &RunOptions) -> Result<RunReport> {
    let mut report = RunReport::default();
    for path in paths {
        for file in scenario_files(path)? {
            report.outcomes.push(run_one(&file, options));
        }
    }
    Ok(report)
}

/// Run one scenario. Every way it can fail is an outcome, never a panic: the
/// run prints one line per scenario whatever happened.
pub fn run_one(path: &Path, options: &RunOptions) -> Outcome {
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
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
    /// How many entries of each host's run record have been read into `ticks`,
    /// so each pass reads only what was written since the last one. One host
    /// appends to its own file and never to another's, which is why the mark is
    /// per file and not one count over the whole set.
    pub read_at: std::collections::BTreeMap<String, usize>,
}

/// A scenario that runs against no configuration at all is a failure, never a
/// silent pass (D15). The acceptance table is the caller's; this is the check
/// it uses.
pub fn every_listed_scenario_ran(listed: &[String], report: &RunReport) -> Vec<String> {
    listed
        .iter()
        .filter(|name| {
            !report
                .ran()
                .iter()
                .any(|o| &&o.scenario == name || &&o.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default() == name)
        })
        .cloned()
        .collect()
}
