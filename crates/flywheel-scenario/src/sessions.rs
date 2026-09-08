//! The scripted session binding: the one stand-in clause 93 admits.
//!
//! The stand-in does not call the reporting command's function; it runs the
//! binary. A scripted exit is a subprocess of `std::env::current_exe()`, its
//! working directory the session's place, its session id in the environment
//! under the same name the rendered work order tells a real session to use
//! (89, D8). One path serves the in-process runner and `--hosts real` alike,
//! and the operator's own hand-run command under 93b is the same invocation,
//! so the phase-2 runner changes nothing about it.
//!
//! Every assertion reads the thread entry the command wrote and never the
//! script.

use crate::store::Store;
use anyhow::{anyhow, Context, Result};
use flywheel_atoms::scenario::ScriptEntry;
use flywheel_atoms::{SessionPresence, Sessions, WorkOrder};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The environment variable the work order names, and the command reads.
pub const SESSION_ENV: &str = "FLYWHEEL_SESSION";

/// An override for the binary the stand-in runs, so a test that is not itself
/// the binary can point at it. Unset, the stand-in runs the process it is.
pub const BINARY_ENV: &str = "FLYWHEEL_BIN";

/// The scripted `Sessions` stand-in.
pub struct ScriptedSessions {
    /// Where the store the command writes through lives.
    pub state: PathBuf,
    /// The root the places sit under; a session's place is its working
    /// directory.
    pub root: PathBuf,
}

impl ScriptedSessions {
    pub fn new(state: impl Into<PathBuf>, root: impl Into<PathBuf>) -> Self {
        ScriptedSessions {
            state: state.into(),
            root: root.into(),
        }
    }

    /// The binary a scripted report runs.
    pub fn binary() -> Result<PathBuf> {
        if let Ok(p) = std::env::var(BINARY_ENV) {
            return Ok(PathBuf::from(p));
        }
        std::env::current_exe().context("the running binary has no path")
    }

    /// A session's place: where the command runs from.
    pub fn place(&self, session: &str) -> PathBuf {
        self.root.join(session.replace('/', "-"))
    }

    /// Run one report as the session would: the binary, the place, the id in
    /// the environment.
    fn run(&self, session: &str, args: &[String]) -> Result<()> {
        let place = self.place(session);
        std::fs::create_dir_all(&place)
            .with_context(|| format!("making the place {}", place.display()))?;
        let binary = Self::binary()?;
        let out = Command::new(&binary)
            .current_dir(&place)
            .env(SESSION_ENV, session)
            .arg("--state")
            .arg(&self.state)
            .args(args)
            .output()
            .with_context(|| format!("running {} {}", binary.display(), args.join(" ")))?;
        // A refused report exits non-zero and is still recorded (80); that is
        // the command's business and not a failure of the script.
        if !out.status.success() && out.status.code() != Some(1) {
            return Err(anyhow!(
                "{} {} failed: {}",
                binary.display(),
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(())
    }

    /// Play one script entry for one session at one step. What the multiplexer
    /// reports — the pane, the activity, a keystroke — is set as world facts,
    /// because that is what the multiplexer would say. What the *session*
    /// reports — an exit, an offer, a refusal — goes through the command.
    pub fn play_entry(
        &self,
        store: &mut Store,
        session: &str,
        entry: &ScriptEntry,
        state_path: &Path,
    ) -> Result<()> {
        let fact = store.world.sessions.entry(session.to_string()).or_default();
        if let Some(pane) = &entry.pane {
            fact.pane = pane == "present";
        }
        if let Some(activity) = &entry.activity {
            fact.activity = activity.clone();
        }
        if entry.keystroke {
            fact.idle_since = None;
        }
        if let Some(q) = &entry.question {
            fact.question = Some(q.clone());
        }
        if let Some(v) = &entry.verdict {
            fact.verdict = Some(v.clone());
        }

        let reports = entry.exit.is_some() || !entry.offers.is_empty() || entry.refusal.is_some();
        if !reports {
            return Ok(());
        }
        // The command writes through the store on disk, so what is in memory
        // has to be there first and read back after.
        crate::save(store, state_path)?;
        if let Some(kind) = &entry.exit {
            let mut args = vec!["exit".to_string(), kind.clone()];
            for d in &entry.deliverables {
                args.push("--deliverable".into());
                args.push(d.clone());
            }
            if let Some(q) = &entry.question {
                args.push("--question".into());
                args.push(q.clone());
            }
            self.run(session, &args)?;
        }
        for offer in &entry.offers {
            self.run(
                session,
                &[
                    "offer".to_string(),
                    offer.kind.clone(),
                    "--document".to_string(),
                    offer.document.clone(),
                ],
            )?;
        }
        if let Some(reason) = &entry.refusal {
            self.run(session, &["refuse".to_string(), reason.clone()])?;
        }
        let written = crate::load(state_path)?;
        store.threads = written.threads;
        store.writes = written.writes;
        store.moved = written.moved;
        Ok(())
    }
}

impl Sessions for ScriptedSessions {
    fn start_session(&self, order: &WorkOrder) -> Result<()> {
        // The stand-in starts nothing: the script says what the session does,
        // and the place is where its reports run from.
        std::fs::create_dir_all(self.place(&order.session))
            .with_context(|| format!("making the place for {}", order.session))?;
        Ok(())
    }

    fn presence(&self, _session: &str) -> Result<SessionPresence> {
        // Presence is what the multiplexer reports, which the store holds; the
        // runner reads it there.
        Ok(SessionPresence::Absent)
    }

    fn end_session(&self, _session: &str) -> Result<()> {
        Ok(())
    }

    fn deliver_answer(&self, _session: &str, _text: &str) -> Result<()> {
        Ok(())
    }

    fn tell_moved(&self, _session: &str) -> Result<()> {
        Ok(())
    }
}
