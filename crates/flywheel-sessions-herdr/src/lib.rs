//! flywheel-sessions-herdr: `Sessions` over Herdr, the multiplexer with an
//! agent-aware API (171, 173, 174, 196, `profiles/sessions.yaml`).
//!
//! The machinery starts every kind of agent through the one command — `herdr
//! agent start` in a pane of its own — in a workspace named for the bolt or
//! the intent and a tab named for the unit or the elaboration, so `herdr
//! agent list` is a status view of its own (196). The pane is at the place,
//! the work order is in the place at `.flywheel/work-order.md`, and the first
//! prompt tells the agent to read it. What the session says comes back through
//! `flywheel exit`, as it does under every runner (67); what Herdr reports —
//! the pane's presence and the agent's activity — is evidence and never state.
//!
//! The record is the operator binding's: the same session fact, with the
//! agent's name and pane added, so a session that reported an exit reads as
//! exited whatever Herdr says, and a pane Herdr no longer lists reads as gone
//! (72, 150, `session.yaml` alive.presence).
//!
//! Every Herdr call is one process and its JSON answer; a Herdr that is not
//! there is reported by the caller and never invented.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use flywheel_atoms::{Records, WorkOrder};
use flywheel_sessions_operator as operator;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

/// The multiplexer, as a command on the path.
#[derive(Debug, Clone)]
pub struct Herdr {
    pub binary: String,
}

impl Default for Herdr {
    fn default() -> Self {
        Herdr {
            binary: "herdr".into(),
        }
    }
}

/// An agent's name in Herdr: `[a-z][a-z0-9_-]{0,31}`, unique among live
/// agents. The session id is the pane's name in the model (196); Herdr's
/// alphabet is narrower, so this is the id in that alphabet, its last 32
/// characters where it is longer — the attempt and the stage at the end are
/// what tell two sessions apart.
pub fn agent_name(session: &str) -> String {
    let mut out = String::new();
    for c in session.chars() {
        match c {
            c if c.is_ascii_alphanumeric() => out.push(c.to_ascii_lowercase()),
            _ if !out.ends_with('-') && !out.is_empty() => out.push('-'),
            _ => {}
        }
    }
    let out = out.trim_matches('-').to_string();
    let tail: String = match out.len() > 32 {
        true => out[out.len() - 32..].to_string(),
        false => out,
    };
    let tail = tail.trim_start_matches(|c: char| !c.is_ascii_lowercase()).to_string();
    match tail.is_empty() {
        true => "session".into(),
        false => tail,
    }
}

/// What the agent is told first: where the order is (89). The order itself is
/// in the place, written by `prepare_place`, so the prompt carries no text of
/// it and a session started later reads the order in force then.
pub fn first_prompt() -> String {
    "Read .flywheel/work-order.md in this directory and do the work it describes. Report the way it says to, with the flywheel command, when you are done or when you are blocked.".into()
}

impl Herdr {
    fn run(&self, args: &[&str]) -> Result<Value> {
        let out = Command::new(&self.binary)
            .args(args)
            .output()
            .with_context(|| format!("running {} {}", self.binary, args.join(" ")))?;
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        if !out.status.success() {
            // CLI server errors are JSON on stderr with exit status 1.
            let said = serde_json::from_str::<Value>(err.trim())
                .ok()
                .and_then(|v| v.pointer("/error/message").and_then(|m| m.as_str()).map(String::from))
                .unwrap_or_else(|| format!("{}{}", text.trim(), err.trim()));
            bail!("herdr {}: {said}", args.join(" "));
        }
        Ok(serde_json::from_str(text.trim()).unwrap_or(Value::Null))
    }

    /// Whether Herdr answers at all: the one check a host makes before
    /// binding a session to it (217f).
    pub fn available(&self) -> Result<()> {
        self.run(&["workspace", "list"]).map(|_| ())
    }

    /// The workspace carrying a label, where one does.
    pub fn workspace_by_label(&self, label: &str) -> Result<Option<String>> {
        let listed = self.run(&["workspace", "list"])?;
        Ok(listed
            .pointer("/result/workspaces")
            .and_then(|v| v.as_array())
            .and_then(|ws| {
                ws.iter()
                    .find(|w| w.get("label").and_then(|l| l.as_str()) == Some(label))
                    .and_then(|w| w.get("workspace_id").and_then(|id| id.as_str()).map(String::from))
            }))
    }

    /// A workspace at a place, labelled; returns the workspace, its first tab
    /// and that tab's pane, which is at a shell prompt in the place.
    pub fn create_workspace(&self, cwd: &Path, label: &str) -> Result<(String, String, String)> {
        let made = self.run(&["workspace", "create", "--cwd", &cwd.to_string_lossy(), "--label", label, "--no-focus"])?;
        let workspace = text_at(&made, &["/result/workspace/workspace_id", "/result/workspace_id"])?;
        let tab = text_at(&made, &["/result/tab/tab_id", "/result/tab_id"])?;
        let pane = text_at(&made, &["/result/root_pane/pane_id", "/result/pane/pane_id", "/result/pane_id"])?;
        Ok((workspace, tab, pane))
    }

    /// A tab in a workspace at a place, labelled; returns the tab and its pane.
    pub fn create_tab(&self, workspace: &str, cwd: &Path, label: &str) -> Result<(String, String)> {
        let made = self.run(&["tab", "create", "--workspace", workspace, "--cwd", &cwd.to_string_lossy(), "--label", label, "--no-focus"])?;
        let tab = text_at(&made, &["/result/tab/tab_id", "/result/tab_id"])?;
        let pane = text_at(&made, &["/result/root_pane/pane_id", "/result/pane/pane_id", "/result/pane_id"])?;
        Ok((tab, pane))
    }

    pub fn rename_tab(&self, tab: &str, label: &str) -> Result<()> {
        self.run(&["tab", "rename", tab, label]).map(|_| ())
    }

    /// The agent by name, or none when Herdr lists no such agent.
    pub fn agent(&self, name: &str) -> Result<Option<Value>> {
        match self.run(&["agent", "get", name]) {
            Ok(v) => Ok(Some(v.pointer("/result/agent").cloned().unwrap_or(v))),
            Err(e) if e.to_string().contains("not found") => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// `herdr agent start <name> --kind <kind> --pane <pane>`: returns once
    /// the agent is ready for input (72).
    pub fn start_agent(&self, name: &str, kind: &str, pane: &str) -> Result<()> {
        self.run(&["agent", "start", name, "--kind", kind, "--pane", pane, "--timeout", "120000"])
            .map(|_| ())
    }

    /// Text to a living agent, submitted as one prompt (197).
    pub fn prompt(&self, name: &str, text: &str) -> Result<()> {
        self.run(&["agent", "prompt", name, text]).map(|_| ())
    }

    pub fn close_pane(&self, pane: &str) -> Result<()> {
        self.run(&["pane", "close", pane]).map(|_| ())
    }

    /// The pane's visible text, plain: what the agent is showing (196).
    pub fn read_visible(&self, name: &str) -> Result<String> {
        let out = Command::new(&self.binary)
            .args(["agent", "read", name, "--source", "visible", "--lines", "60"])
            .output()
            .with_context(|| format!("running {} agent read {name}", self.binary))?;
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    pub fn send_keys(&self, name: &str, keys: &[&str]) -> Result<()> {
        let mut args = vec!["agent", "send-keys", name];
        args.extend_from_slice(keys);
        self.run(&args).map(|_| ())
    }

    /// The agent's status as Herdr reports it: working, idle, blocked, done,
    /// unknown; `absent` when Herdr lists no such agent.
    pub fn status(&self, name: &str) -> Result<String> {
        Ok(self
            .agent(name)?
            .and_then(|a| a.get("agent_status").and_then(|s| s.as_str()).map(String::from))
            .unwrap_or_else(|| "absent".into()))
    }

    /// An agent just started may stand at its own first-run question before it
    /// takes a prompt: Claude Code asks whether the folder is trusted, and a
    /// place the machinery made is. The question is answered from what the
    /// pane shows, and the prompt waits until the agent is at its prompt line
    /// (72, 196, 197). What Herdr shows is evidence, so a pane showing nothing
    /// of the kind is left alone.
    pub fn clear_the_way(&self, name: &str) -> Result<bool> {
        let mut answered = false;
        for _ in 0..30 {
            match self.status(name)?.as_str() {
                "idle" | "done" => return Ok(answered),
                "blocked" => {
                    let shown = self.read_visible(name)?;
                    if shown.contains("trust") && shown.contains("folder") {
                        self.send_keys(name, &["down", "enter"])?;
                        answered = true;
                    }
                }
                _ => {}
            }
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
        Ok(answered)
    }
}

fn text_at(v: &Value, pointers: &[&str]) -> Result<String> {
    for p in pointers {
        if let Some(s) = v.pointer(p).and_then(|x| x.as_str()) {
            return Ok(s.to_string());
        }
    }
    bail!("herdr answered without {}: {v}", pointers.join(" or "))
}

/// Where a session's pane goes: the workspace for the bolt or the intent, the
/// tab for the unit or the elaboration (196, layout).
#[derive(Debug, Clone)]
pub struct Placement {
    pub workspace_label: String,
    pub tab_label: String,
    pub cwd: std::path::PathBuf,
    /// The agent kind: claude, codex or opencode (173).
    pub kind: String,
}

/// Start the session: the record as the operator binding writes it, and the
/// agent in a pane of its own at the place. A second start of the same name
/// is refused by Herdr, which is the proof that it is running (72, 73).
pub fn start<S: Records>(
    store: &mut S,
    herdr: &Herdr,
    host: &str,
    now: DateTime<Utc>,
    order: &WorkOrder,
    placement: &Placement,
) -> Result<()> {
    if operator::running(store, &order.session) {
        return Ok(());
    }
    let name = agent_name(&order.session);
    let mut pane = None;
    let mut workspace = None;
    let mut fresh = false;
    if herdr.agent(&name)?.is_none() {
        let (ws, tab, pane_id) = match herdr.workspace_by_label(&placement.workspace_label)? {
            Some(ws) => {
                let (tab, pane) = herdr.create_tab(&ws, &placement.cwd, &placement.tab_label)?;
                (ws, tab, pane)
            }
            None => {
                let (ws, tab, pane) = herdr.create_workspace(&placement.cwd, &placement.workspace_label)?;
                let _ = herdr.rename_tab(&tab, &placement.tab_label);
                (ws, tab, pane)
            }
        };
        // An agent stopped at its own first-run question is "blocked during
        // startup" to Herdr and not a failure here: the question is answered
        // below and the agent is then ready (72, 196).
        match herdr.start_agent(&name, &placement.kind, &pane_id) {
            Ok(()) => {}
            Err(e) if e.to_string().contains("blocked during startup") => {}
            Err(e) => {
                return Err(e).with_context(|| format!("starting `{name}` ({}) in pane {pane_id}", placement.kind));
            }
        }
        fresh = true;
        pane = Some(pane_id);
        workspace = Some((ws, tab));
    }
    // A fresh agent is told where the order is; one found standing at its
    // first-run question is told once the question is answered; one found
    // already at work is left to it (197).
    let answered = herdr.clear_the_way(&name)?;
    if fresh || answered {
        herdr.prompt(&name, &first_prompt())?;
    }
    operator::start(store, host, now, order)?;
    let mut fields: Vec<(&str, Value)> = vec![("runner", json!("herdr")), ("herdr_agent", json!(name))];
    if let Some(pane) = &pane {
        fields.push(("herdr_pane", json!(pane)));
    }
    if let Some((ws, tab)) = &workspace {
        fields.push(("herdr_workspace", json!(ws)));
        fields.push(("herdr_tab", json!(tab)));
    }
    operator::set(store, &order.session, &fields)
}

/// The `session.*` evidence: the pane and the activity from Herdr for a
/// session this binding started and that has not reported; everything else
/// the operator binding's rule (`sessions.yaml` evidence).
pub fn evidence<S: Records>(store: &S, herdr: &Herdr, session: &str, name: &str) -> Option<Value> {
    let asks_the_pane = matches!(
        name,
        "session.pane" | "session.pane_absent" | "session.activity" | "session.idle_since" | "session.operator_present"
    );
    if !asks_the_pane {
        return operator::evidence(store, session, name);
    }
    let fact = store.get(&operator::session_fact(session)).ok().flatten();
    let agent = fact
        .as_ref()
        .and_then(|f| f.record.get("herdr_agent"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let Some(agent) = agent else {
        return operator::evidence(store, session, name);
    };
    // A session that reported done is exited whatever its pane does (68, 70).
    if !operator::running(store, session) {
        return operator::evidence(store, session, name);
    }
    let listed = herdr.agent(&agent).ok().flatten();
    let status = listed
        .as_ref()
        .and_then(|a| a.get("agent_status"))
        .and_then(|s| s.as_str())
        .unwrap_or("absent")
        .to_string();
    Some(match name {
        "session.pane" => json!(match listed.is_some() {
            true => "present",
            false => "absent",
        }),
        "session.pane_absent" => json!(listed.is_none()),
        "session.activity" => json!(match status.as_str() {
            "working" => "working",
            "absent" => "none",
            _ => "idle",
        }),
        "session.idle_since" => Value::Null,
        "session.operator_present" => json!(false),
        _ => return None,
    })
}

/// End the pane and close the record (26, 74, 196).
pub fn end<S: Records>(store: &mut S, herdr: &Herdr, session: &str, now: DateTime<Utc>) -> Result<()> {
    if let Some(pane) = field(store, session, "herdr_pane") {
        // A pane already gone is not an error: the end is what was wanted.
        let _ = herdr.close_pane(&pane);
    }
    operator::end(store, session, now)
}

/// The answer on the thread, and to the living agent as a prompt (68, 70, 197).
pub fn answer<S: Records>(store: &mut S, herdr: &Herdr, session: &str, host: &str, now: DateTime<Utc>, text: &str) -> Result<()> {
    operator::answer(store, session, host, now, text)?;
    if let Some(agent) = field(store, session, "herdr_agent") {
        if herdr.agent(&agent)?.is_some() {
            herdr.prompt(&agent, &format!("The operator answered your question: {text}\n\nCarry on from where you stopped, and report with the flywheel command when you are done."))?;
        }
    }
    Ok(())
}

/// What moved under the place, on the thread and to the agent (51, 71, 197).
pub fn moved<S: Records>(store: &mut S, herdr: &Herdr, session: &str, host: &str, now: DateTime<Utc>) -> Result<()> {
    operator::moved(store, session, host, now)?;
    if let Some(agent) = field(store, session, "herdr_agent") {
        if herdr.agent(&agent)?.is_some() {
            herdr.prompt(&agent, "The line moved under your place and it has been rebased onto it; read what changed before you continue.")?;
        }
    }
    Ok(())
}

fn field<S: Records>(store: &S, session: &str, name: &str) -> Option<String> {
    store
        .get(&operator::session_fact(session))
        .ok()
        .flatten()?
        .record
        .get(name)
        .and_then(|v| v.as_str())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flywheel_atoms::testing::FakeStore;

    #[test]
    fn a_name_is_at_most_thirty_two_and_starts_with_a_letter() {
        let name = agent_name("work-item/storefront/order-total/wi-1/fix/1");
        assert!(name.len() <= 32, "{name}");
        assert!(name.chars().next().unwrap().is_ascii_lowercase(), "{name}");
        assert!(name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_'), "{name}");
        assert!(name.ends_with("wi-1-fix-1"), "the stage and the attempt are what tell two apart: {name}");
        assert_eq!(agent_name("curation/storefront/main/1"), "curation-storefront-main-1");
        assert_eq!(agent_name("///"), "session");
    }

    #[test]
    fn a_session_this_binding_did_not_start_reads_by_the_operators_rule() {
        let store = FakeStore::default();
        let herdr = Herdr { binary: "/nonexistent/herdr".into() };
        // No fact at all: absent, as the operator binding says.
        assert_eq!(evidence(&store, &herdr, "unit/atlas/u/fix/1", "session.pane"), Some(json!("absent")));
        assert_eq!(evidence(&store, &herdr, "unit/atlas/u/fix/1", "session.exit"), Some(json!("none")));
    }

    #[test]
    fn a_session_that_reported_done_is_exited_whatever_the_pane_says() {
        let mut store = FakeStore::default();
        let herdr = Herdr { binary: "/nonexistent/herdr".into() };
        operator::set(
            &mut store,
            "unit/atlas/u/fix/1",
            &[
                ("started_at", json!("2026-09-14T12:00:00Z")),
                ("ended_at", Value::Null),
                ("host", json!("local")),
                ("herdr_agent", json!("u-fix-1")),
            ],
        )
        .unwrap();
        // Running and started here: the pane is asked, and a Herdr that is not
        // there lists no agent, so the pane is absent (150).
        assert_eq!(evidence(&store, &herdr, "unit/atlas/u/fix/1", "session.pane"), Some(json!("absent")));
        // Reported done: exited by the record, and Herdr is not even asked.
        flywheel_domain::report::write_report(
            &mut store,
            "unit/atlas/u/fix/1",
            "u-fix-1",
            chrono::Utc::now(),
            &flywheel_domain::report::Report::Exit { kind: "done".into(), deliverables: vec!["commits".into()], question: None, text: None },
        )
        .unwrap();
        assert_eq!(evidence(&store, &herdr, "unit/atlas/u/fix/1", "session.pane"), Some(json!("absent")));
        assert_eq!(evidence(&store, &herdr, "unit/atlas/u/fix/1", "session.exit"), Some(json!("done")));
    }
}
