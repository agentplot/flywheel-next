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
}

impl Watchers {
    pub fn new(woken: Arc<tokio::sync::Notify>, changed: Arc<tokio::sync::watch::Sender<u64>>) -> Self {
        Watchers { watching: Arc::new(Mutex::new(HashSet::new())), woken, changed }
    }

    /// Start a wait on every live agent this store names that is not already
    /// waited on. Called after each pass; a wait that ends because the agent
    /// is gone frees its name, and the next pass decides whether to wait
    /// again.
    pub fn follow<S: flywheel_atoms::Records>(&self, store: &S) {
        for (_session, agent) in flywheel_sessions_herdr::live_agents(store) {
            {
                let mut set = self.watching.lock().expect("the watchers are poisoned");
                if !set.insert(agent.clone()) {
                    continue;
                }
            }
            let watching = self.watching.clone();
            let woken = self.woken.clone();
            let changed = self.changed.clone();
            tokio::task::spawn_blocking(move || {
                let herdr = flywheel_sessions_herdr::Herdr::default();
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
                            std::thread::sleep(std::time::Duration::from_secs(30));
                            break;
                        }
                    }
                }
                watching.lock().expect("the watchers are poisoned").remove(&agent);
            });
        }
    }
}
