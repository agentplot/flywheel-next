//! The record one run of the suite leaves behind.
//!
//! A run says which profile it bound and which machine files it ran, and the
//! hash of those files is compared with `definitions/` — the mirror of the
//! model — so a suite that passed against a hand-edited directory is refused
//! rather than counted (168, D2). What the record holds beyond that — the
//! scenarios that ran, the subset skipped with its reason, the failures — is
//! written through the store (79–82, 167, D15).

use super::{Outcome, RunOptions, Status};
use anyhow::{Context, Result};
use flywheel_domain::records::RunEntry;

/// The name the record is written under. The runner is not a host: it is what
/// ran the suite, and its record is its own.
pub const RUNNER: &str = "runner";

/// What a run recorded about itself.
#[derive(Debug, Clone)]
pub struct RunRecord {
    /// The store binding the run bound (140).
    pub profile: String,
    /// The hash of the machine files the run loaded: the set the binary
    /// carries, or the directory `--definitions` named (D2).
    pub definitions_hash: u64,
    /// The hash of `definitions/` as the binary was built from it, where the
    /// directory is there to read.
    pub repository_hash: Option<u64>,
    /// The scenarios that ran, in the order they did.
    pub ran: Vec<String>,
    /// The subset skipped, each with the reason it was (93a).
    pub skipped: Vec<(String, String)>,
    /// Every failure, as the run printed it (79–82).
    pub failures: Vec<String>,
}

impl Default for RunRecord {
    fn default() -> Self {
        RunRecord {
            profile: "stand-in".into(),
            definitions_hash: 0,
            repository_hash: None,
            ran: vec![],
            skipped: vec![],
            failures: vec![],
        }
    }
}

impl RunRecord {
    /// What this run will record before it plays a scenario.
    pub fn of(options: &RunOptions) -> Result<RunRecord> {
        Ok(RunRecord {
            profile: options.profile.name().to_string(),
            definitions_hash: definitions_hash(options)?,
            repository_hash: flywheel_domain::set::repository_digest(),
            ran: vec![],
            skipped: vec![],
            failures: vec![],
        })
    }

    /// Take what became of one scenario into the record.
    pub fn saw(&mut self, outcome: &Outcome) {
        match outcome.status {
            Status::Passed | Status::Failed => self.ran.push(outcome.scenario.clone()),
            Status::Skipped => self.skipped.push((
                outcome.scenario.clone(),
                outcome.reason.clone().unwrap_or_default(),
            )),
            // A scenario that does not apply to this binding was never this
            // run's to skip: `profiles:` is the store binding and nothing else.
            Status::NotApplicable => {}
            Status::Invalid | Status::Refused => {}
        }
        self.failures.extend(outcome.failures.iter().cloned());
    }

    /// The record as entries on the run record: what the run bound, what it
    /// ran, what it skipped and why, and every failure (79–82, 93a, 167).
    pub fn entries(&self, at: chrono::DateTime<chrono::Utc>) -> Vec<RunEntry> {
        let mut out = vec![RunEntry::new(
            at,
            RUNNER,
            "binding",
            &self.profile,
            "the conformance suite ran on this profile",
        )
        .with("definitions_hash", &format!("{:016x}", self.definitions_hash))
        .with(
            "repository_hash",
            &self
                .repository_hash
                .map(|h| format!("{h:016x}"))
                .unwrap_or_else(|| "unread".into()),
        )
        .with(
            "definitions_match_repository",
            &self.hash_matches_repository().to_string(),
        )
        .with("ran", &self.ran.join(" "))
        .with(
            "skipped",
            &self
                .skipped
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        )];
        // A skip is never silent: the scenario and the requirement it names are
        // each an entry of their own (93a).
        for (scenario, why) in &self.skipped {
            out.push(
                RunEntry::new(at, RUNNER, "skipped", scenario, why)
                    .with("profile", &self.profile),
            );
        }
        for failure in &self.failures {
            out.push(
                RunEntry::new(at, RUNNER, "problem", &self.profile, failure)
                    .with("profile", &self.profile),
            );
        }
        out
    }

    /// Whether the machine files that ran are the repository's. With no
    /// directory to read there is nothing to disagree with, and the run stands
    /// on the set the binary carries.
    pub fn hash_matches_repository(&self) -> bool {
        self.repository_hash
            .is_none_or(|held| held == self.definitions_hash)
    }

    /// Why the profile is refused, where it is. An unmatched hash is exit 3,
    /// beside an incomplete binding: the run proved nothing about the model
    /// the repository holds (168, D15).
    pub fn refusal(&self) -> Option<String> {
        if self.hash_matches_repository() {
            return None;
        }
        Some(format!(
            "REFUSED  the machine files are not definitions/ · ran {:016x} · definitions/ {:016x} (168, D2)",
            self.definitions_hash,
            self.repository_hash.unwrap_or_default()
        ))
    }
}

/// Write the run's record through the store the run bound (79–82, 93a, 167).
///
/// The run's own record is not a scenario's: each scenario plays over a store
/// of its own, and this is what the run did with all of them. It is written
/// through a store bound the same way — a state repository on `--profile
/// git-only`, the stand-in's own record otherwise — so the record is a record
/// and never a file the runner wrote beside itself.
pub fn write(record: &RunRecord, options: &RunOptions) -> Result<crate::store::Store> {
    let at = super::drive::start_of_time();
    let mut store = crate::store::Store::default();
    store.now = at;
    if options.profile == super::Profile::GitOnly {
        // Absolute: git is run from inside the checkout, and a relative remote
        // would resolve against that and not against where the run started.
        let base = match options.record_dir().is_absolute() {
            true => options.record_dir(),
            false => std::env::current_dir()
                .context("where the run started")?
                .join(options.record_dir()),
        };
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base)
            .with_context(|| format!("making {}", base.display()))?;
        let git = flywheel_store_git::store::sandbox(&base, RUNNER, at)
            .context("opening the state repository the run's record is written to")?;
        store
            .durable
            .insert(RUNNER.to_string(), std::sync::Arc::new(std::sync::Mutex::new(git)));
        store.acting_host = Some(RUNNER.to_string());
    }
    store
        .append_run(&record.entries(at))
        .context("writing the run's record")?;
    Ok(store)
}

/// The hash of the machine files a run loads: the directory `--definitions`
/// names, or the set the binary carries (D2). A scenario naming its own
/// `machines:` replaces the domain for that scenario alone and leaves the set
/// the run recorded where it is.
pub fn definitions_hash(options: &RunOptions) -> Result<u64> {
    match &options.definitions {
        Some(dir) => flywheel_domain::set::digest_of_dir(dir)
            .with_context(|| format!("hashing the machine files at {}", dir.display())),
        None => Ok(flywheel_domain::set::digest()),
    }
}
