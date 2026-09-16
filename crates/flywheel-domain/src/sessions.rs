//! What the host's reconciliation closes: panes an exit left open, and tabs
//! naming work that has left every view (73, 74, 186, 196).
//!
//! Both are decided here, over the records alone, so the rule is one thing and
//! the host is left with the multiplexer calls. What the host reads from herdr
//! — the agents it lists, the labels its workspaces and tabs carry — is handed
//! in; nothing here talks to a multiplexer.

use crate::regions;
use anyhow::Result;
use flywheel_atoms::{Records, Scope};
use flywheel_engine::{Definitions, Object};

/// The exit a session reported, from the newest exit entry on its thread (67).
/// The same read the sessions bindings make, written here because the domain
/// is below them.
fn exit_of<S: Records>(store: &S, session: &str) -> Option<String> {
    store
        .thread(session)
        .ok()?
        .into_iter()
        .filter(|entry| entry.kind == "exit")
        .next_back()
        .and_then(|entry| entry.fields.get("exit").and_then(|v| v.as_str()).map(String::from))
}

/// The workspaces a machinery kind reads, which are per kind and not per
/// object: they stay whatever their runs do (196, `sessions.yaml` layout).
pub const MACHINERY_WORKSPACES: &[&str] = &["curation", "capture-reading"];

/// A pane still open for a session that has finished with it (74).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenPane {
    /// The session whose pane it is.
    pub session: String,
    /// The herdr session the pane is in, as the record names it (174).
    pub herdr_session: String,
    pub pane: String,
    /// What the run record says about why it was ended.
    pub why: String,
}

/// Every session whose pane ought to be closed: one exited whose type does not
/// keep it, one whose record is already ended, and one whose owner has reached
/// a final state (74, 196, `atoms.yaml` host.no_exited_panes).
///
/// Blocked is not an exit and keeps its pane for the answer (70); a session
/// with `keep_alive` true stands after its exit until its owner ends it (25,
/// 26, 69); a session with no pane recorded has nothing to close (73).
pub fn panes_to_end<S: Records>(store: &S, defs: &Definitions) -> Result<Vec<OpenPane>> {
    let records = store.list_records(&Scope::All)?;
    let prefix = "fact/session/";
    let mut out = Vec::new();
    for fact in records.iter().filter(|o| o.id.starts_with(prefix)) {
        let session = fact.id.trim_start_matches(prefix).to_string();
        let text = |name: &str| fact.record.get(name).and_then(|v| v.as_str()).map(String::from);
        let Some(pane) = text("herdr_pane") else {
            continue;
        };
        let herdr_session = text("herdr_session").unwrap_or_default();
        let ended = fact.record.get("ended_at").is_some_and(|v| !v.is_null());
        let exit = exit_of(store, &session);
        let keep_alive = fact
            .record
            .get("keep_alive")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        // The owner is the longest prefix of the session id that is on record,
        // the same rule the offer command finds its object by.
        let owner = owner_of(&records, &session);
        let why = if ended {
            "the session record is ended and its pane was still open"
        } else if matches!(exit.as_deref(), Some("done" | "stalled" | "invalid")) && !keep_alive {
            "its type does not keep the session and it exited"
        } else if owner.is_some_and(|object| ended_object(defs, object)) {
            "its owner has reached a final state"
        } else {
            continue;
        };
        out.push(OpenPane {
            session,
            herdr_session,
            pane,
            why: why.to_string(),
        });
    }
    Ok(out)
}

/// The labels reconciliation closes, of those the host's own sessions hold
/// (186, 196).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Layout {
    pub tabs: Vec<String>,
    pub workspaces: Vec<String>,
}

/// Which of the labels herdr lists name work that has left every view.
///
/// A tab is labelled by a unit's or an elaboration's id, or by a machinery
/// run's session id; a workspace by a bolt's or an intent's id, or by the
/// machinery kind it reads. A machinery run's tab goes once the run is final
/// and its pane is gone; the machinery workspaces stay, because they are per
/// kind and not per object (196).
pub fn layout_to_close<S: Records>(
    store: &S,
    defs: &Definitions,
    workspaces: &[String],
    tabs: &[String],
) -> Result<Layout> {
    let records = store.list_records(&Scope::All)?;
    let held = |id: &str| records.iter().find(|o| o.id == id);
    let gone = |id: &str| match held(id) {
        Some(object) => ended_object(defs, object),
        // A label naming nothing on record names work that has left (186).
        None => true,
    };
    let mut out = Layout::default();
    for label in tabs {
        // A machinery run's tab is labelled by its session id; it goes once
        // the run is final and its pane is gone.
        if let Some(run) = run_of(store, &records, label) {
            if run {
                out.tabs.push(label.clone());
            }
            continue;
        }
        if gone(label) {
            out.tabs.push(label.clone());
        }
    }
    for label in workspaces {
        if MACHINERY_WORKSPACES.contains(&label.as_str()) || label.starts_with("planning/") {
            continue;
        }
        // The label a host marks its own session with is not a view of work.
        if label.starts_with("host/") {
            continue;
        }
        if gone(label) {
            out.workspaces.push(label.clone());
        }
    }
    Ok(out)
}

/// Whether a label names a machinery run whose tab may go: its session is
/// final and its pane is gone. `None` when the label names no session record.
fn run_of<S: Records>(store: &S, records: &[Object], label: &str) -> Option<bool> {
    let fact = records.iter().find(|o| o.id == format!("fact/session/{label}"))?;
    let ended = fact.record.get("ended_at").is_some_and(|v| !v.is_null());
    let exit = exit_of(store, label);
    let final_run = ended || matches!(exit.as_deref(), Some("done" | "stalled" | "invalid"));
    let pane_gone = ended || fact.record.get("herdr_pane").is_none();
    Some(final_run && pane_gone)
}

/// The object a session id runs under: the longest prefix of the id that is on
/// record (`session.yaml` id).
fn owner_of<'a>(records: &'a [Object], session: &str) -> Option<&'a Object> {
    let mut at = session.to_string();
    while let Some((head, _)) = at.rsplit_once('/') {
        at = head.to_string();
        if let Some(object) = records.iter().find(|o| o.id == at) {
            return Some(object);
        }
    }
    None
}

/// Whether an object has ended: every top-level region of its machine it
/// stands in is at a final state.
fn ended_object(defs: &Definitions, object: &Object) -> bool {
    let Some(machine) = defs.for_object(&object.machine).or_else(|| defs.get(&object.machine)) else {
        return false;
    };
    let mut any = false;
    for (name, region) in &machine.regions {
        let Some(state) = object.config.get(name).and_then(|s| region.states.get(s)) else {
            continue;
        };
        if !state.is_final {
            return false;
        }
        any = true;
    }
    any
}

/// The keep_alive a session's owner gives it, for the record written at start
/// (74, `session.yaml` record.keep_alive).
pub fn keep_alive_at(defs: &Definitions, object: &Object, region: &str) -> Option<bool> {
    regions::keep_alive_of(defs, object, region)
}
