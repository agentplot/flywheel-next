//! Herdr's agents, watched rather than polled (S221, 130, D6).
//!
//! A session's agent going idle, blocked or done is a cause the host should
//! act on when it happens, not at its next poll. Herdr has a wait on exactly
//! that, so the host keeps one blocking wait per live agent; when the wait
//! returns with a change the loop is woken and every open page is told.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub struct Watchers {
    watching: Arc<Mutex<HashSet<String>>>,
    woken: Arc<tokio::sync::Notify>,
    changed: Arc<tokio::sync::watch::Sender<u64>>,
    herdr: flywheel_sessions_herdr::Herdr,
    /// How long a wait that failed stands before its agent may be waited on
    /// again.
    retry: std::time::Duration,
}

impl Watchers {
    pub fn new(woken: Arc<tokio::sync::Notify>, changed: Arc<tokio::sync::watch::Sender<u64>>) -> Self {
        Watchers::over(flywheel_sessions_herdr::Herdr::unbound(), std::time::Duration::from_secs(30), woken, changed)
    }

    /// The same over a Herdr of the caller's.
    pub fn over(
        herdr: flywheel_sessions_herdr::Herdr,
        retry: std::time::Duration,
        woken: Arc<tokio::sync::Notify>,
        changed: Arc<tokio::sync::watch::Sender<u64>>,
    ) -> Self {
        Watchers { watching: Arc::new(Mutex::new(HashSet::new())), woken, changed, herdr, retry }
    }

    /// Start a wait on every live agent this store names that is not already
    /// waited on. Called after each pass; a wait that ends because the agent
    /// is gone frees its name, and the next pass decides whether to wait
    /// again.
    pub fn follow<S: flywheel_atoms::Records>(&self, store: &S) {
        for live in flywheel_sessions_herdr::live_agents(store) {
            let agent = live.agent.clone();
            // A pane recorded before sessions were addressed by name is the
            // operator's, in the operator's own session: it is not watched
            // here, and nothing this host does reaches it (174).
            if live.multiplexer.is_empty() {
                continue;
            }
            {
                let mut set = self.watching.lock().expect("the watchers are poisoned");
                if !set.insert(agent.clone()) {
                    continue;
                }
            }
            let watching = self.watching.clone();
            let woken = self.woken.clone();
            let changed = self.changed.clone();
            // Every wait addresses the herdr session the pane is in (174).
            let herdr = self.herdr.in_session(&live.multiplexer);
            let retry = self.retry;
            tokio::task::spawn_blocking(move || {
                let mut current = herdr.status(&agent).unwrap_or_else(|_| "absent".into());
                loop {
                    match herdr.wait_change(&agent, &current, 60_000) {
                        Ok(Some(status)) => {
                            current = status;
                            changed.send_modify(|generation| *generation += 1);
                            woken.notify_waiters();
                        }
                        Ok(None) => {}
                        // The agent is gone, or Herdr is: the loop's own
                        // evidence says which, and this wait is not renewed
                        // for half a minute so a gone agent is not asked for
                        // every pass.
                        Err(_) => {
                            std::thread::sleep(retry);
                            break;
                        }
                    }
                }
                watching.lock().expect("the watchers are poisoned").remove(&agent);
            });
        }
    }
}
