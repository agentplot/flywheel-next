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
//!
//! Every call names the herdr session it is for — `herdr --session
//! flywheel-<instance>-<role> <command…>` — so a host opens panes in sessions
//! of its own and never in the one its own process happens to run in, which is
//! the operator's (174, `sessions.yaml` multiplexer_sessions). A session herdr
//! does not list running is started headless, as a detached child, before any
//! call against its socket; the host never stops or deletes one.

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use flywheel_atoms::{Records, WorkOrder};
use flywheel_sessions_operator as operator;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

/// The multiplexer, as a command on the path, addressed at one of its named
/// sessions (174). There is no unaddressed Herdr: a call with no session runs
/// against whatever server the host's own process sits in, which is what put
/// the machinery's panes in the operator's session.
#[derive(Debug, Clone)]
pub struct Herdr {
    pub binary: String,
    /// The herdr session every call names: `flywheel-<instance>-intents`,
    /// `-bolts` or `-machinery`, or what the host's manifest overrode it with.
    pub session: String,
}

impl Herdr {
    /// The multiplexer on the path, at this session.
    pub fn at(session: &str) -> Herdr {
        Herdr {
            binary: "herdr".into(),
            session: session.to_string(),
        }
    }

    /// The multiplexer on the path with no session yet: every use resolves the
    /// session from the record it acts on, and a call made before that is
    /// refused rather than run against whatever server this process sits in
    /// (174). A session recorded before panes were addressed by name has none,
    /// and its pane is the operator's to close.
    pub fn unbound() -> Herdr {
        Herdr {
            binary: "herdr".into(),
            session: String::new(),
        }
    }

    /// The same multiplexer at another of its sessions: what a host does when
    /// the record says which session a pane is in.
    pub fn in_session(&self, session: &str) -> Herdr {
        Herdr {
            binary: self.binary.clone(),
            session: session.to_string(),
        }
    }

    /// The same session through another binary: what a test's own herdr is.
    pub fn with_binary(&self, binary: &str) -> Herdr {
        Herdr {
            binary: binary.to_string(),
            session: self.session.clone(),
        }
    }
}

/// Who charged a session, which is what names the herdr session it starts in
/// (174, `sessions.yaml` multiplexer_sessions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Charged {
    /// Elaboration sessions and the operator's own session.
    Intents,
    /// Stage sessions of units the operator approved.
    Bolts,
    /// Curation, planning, capture reading, and the fix of a take conflict.
    Machinery,
}

impl Charged {
    pub fn role(&self) -> &'static str {
        match self {
            Charged::Intents => "intents",
            Charged::Bolts => "bolts",
            Charged::Machinery => "machinery",
        }
    }

    /// The role whose kind and model this session runs by default, which is
    /// what the manifest declares and the shipped profile falls back to
    /// (`sessions.yaml` models, 173).
    pub fn model_role(&self) -> &'static str {
        match self {
            Charged::Intents => "elaboration",
            Charged::Bolts => "construction",
            Charged::Machinery => "machinery",
        }
    }
}

/// The herdr session a session starts in: `flywheel-<instance>-<role>` by who
/// charged it, overridden per host by kind or by repository, the most specific
/// winning — repository over kind over default (174).
pub fn session_name(
    instance: &str,
    charged: Charged,
    overrides: &BTreeMap<String, String>,
    kind: &str,
    repository: &str,
) -> String {
    if let Some(named) = overrides.get(repository).filter(|n| !n.is_empty()) {
        return named.clone();
    }
    if let Some(named) = overrides.get(kind).filter(|n| !n.is_empty()) {
        return named.clone();
    }
    format!("flywheel-{instance}-{}", charged.role())
}

/// The label a host marks a session it made with: the mark two hosts on one
/// computer share sight of, since the multiplexer session is what they share
/// (218, `sessions.yaml` own).
pub fn host_label(host: &str) -> String {
    format!("host/{host}")
}

/// Whose a running herdr session is (218).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Own {
    /// This host started it: its own after a restart.
    Mine,
    /// Another host on this computer already runs an instance of this name.
    AnotherHost(String),
    /// No host's label at all: a session the operator made by that name.
    TheOperators,
}

/// Who charged a session, from the object it is under and the chain of parents
/// above it: a unit's stage session is the bolts' and an elaboration's is the
/// intents', while curation, planning and capture reading are the machinery's
/// (174, `sessions.yaml` multiplexer_sessions).
pub fn charged_by(object: &str, chain: &[String]) -> Charged {
    if machinery_workspace(object).is_some() {
        return Charged::Machinery;
    }
    if chain.iter().any(|id| id.starts_with("bolt/")) || object.starts_with("unit/") {
        return Charged::Bolts;
    }
    if chain.iter().any(|id| id.starts_with("intent/"))
        || object.starts_with("elaboration/")
        || object.starts_with("operator-session/")
    {
        return Charged::Intents;
    }
    Charged::Machinery
}

/// The one workspace a machinery run reads, where the object is a machinery
/// one: curation, planning per repository, and capture reading each have a
/// workspace of their own in the machinery session, with a tab per run
/// (196, `sessions.yaml` layout.workspace).
pub fn machinery_workspace(object: &str) -> Option<String> {
    let head = object.split('/').next().unwrap_or_default();
    match head {
        "curation" => Some("curation".into()),
        // `planning/<repository>` is the object's own id, which is the label.
        "planning" => Some(
            object
                .split('/')
                .take(2)
                .collect::<Vec<_>>()
                .join("/"),
        ),
        "capture" => Some("capture-reading".into()),
        _ => None,
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
    /// One call at this host's own session: its JSON answer, or what Herdr
    /// said when it refused. The refusal is Herdr's words alone, so a caller
    /// reading it for "not found" or "timed out" is not reading its own
    /// arguments back.
    fn call(&self, args: &[&str]) -> Result<std::result::Result<Value, String>> {
        if self.session.is_empty() {
            bail!(
                "`herdr {}` names no session: a host addresses its own sessions by name and \
                 never the one its process runs in, which is the operator's (174)",
                args.join(" ")
            );
        }
        let mut addressed: Vec<&str> = vec!["--session", &self.session];
        addressed.extend_from_slice(args);
        self.unaddressed(&addressed)
    }

    /// A call that names no session: `session list`, and the `server` that
    /// starts one. Everything else goes through `call` (174).
    fn unaddressed(&self, args: &[&str]) -> Result<std::result::Result<Value, String>> {
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
            return Ok(Err(said));
        }
        Ok(Ok(serde_json::from_str(text.trim()).unwrap_or(Value::Null)))
    }

    fn run(&self, args: &[&str]) -> Result<Value> {
        match self.call(args)? {
            Ok(answer) => Ok(answer),
            Err(said) => bail!("herdr {}: {said}", args.join(" ")),
        }
    }

    /// Whether Herdr answers at all: the one check a host makes before
    /// binding a session to it (217f). It names no session, since a session
    /// that is not running is a thing to start and not an absent Herdr.
    pub fn available(&self) -> Result<()> {
        match self.unaddressed(&["session", "list", "--json"])? {
            Ok(_) => Ok(()),
            Err(said) => bail!("herdr session list: {said}"),
        }
    }

    /// Whether `herdr session list` names this session running. A session is a
    /// named server with its own socket, so this is what says whether a call
    /// against that socket will be answered (174).
    /// `herdr session list` prints a table for a person; `--json` is the
    /// answer this reads, and it names each session with a `running` flag
    /// (verified against herdr 0.9, 2026-09-15).
    pub fn session_running(&self) -> Result<bool> {
        let listed = match self.unaddressed(&["session", "list", "--json"])? {
            Ok(v) => v,
            Err(said) => bail!("herdr session list: {said}"),
        };
        let rows = ["/sessions", "/result/sessions"]
            .iter()
            .find_map(|at| listed.pointer(at).and_then(|v| v.as_array()))
            .cloned()
            .or_else(|| listed.as_array().cloned())
            .unwrap_or_default();
        Ok(rows.iter().any(|row| {
            let named = row.get("name").and_then(|v| v.as_str()) == Some(self.session.as_str());
            // A session herdr lists as not running is one to start: its socket
            // refuses every call until the server is up (174).
            let running = row
                .get("running")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            named && running
        }))
    }

    /// The session, running and this host's to open panes in.
    ///
    /// One herdr does not list running is started headless as a detached child
    /// — `herdr --session <name> server` — and the proof is the session listed
    /// running; the host never stops or deletes one (174).
    ///
    /// An instance's name is unique on its computer, since its multiplexer
    /// sessions and every agent in them are named from it (174, 196, 218). The
    /// host that starts a session labels its first workspace `host/<host id>`;
    /// a host that finds the session already running reads its workspaces
    /// before any other call — its own label is its own after a restart,
    /// another host's label means another host on this computer already runs
    /// an instance of this name, and no host label at all is a session the
    /// operator made by that name. Either collision is declined, naming what
    /// it found, and no further herdr call is made (218, `sessions.yaml` own).
    pub fn ensure_session(&self, host: &str) -> Result<()> {
        if self.session.is_empty() {
            return Ok(());
        }
        if self.session_running()? {
            return match self.whose(host)? {
                Own::Mine => Ok(()),
                Own::AnotherHost(other) => bail!(
                    "the herdr session `{}` belongs to host `{other}`: another host on this \
                     computer already runs an instance of this name, and an instance's name is \
                     unique on its computer, so this host starts nothing (218)",
                    self.session
                ),
                Own::TheOperators => bail!(
                    "the herdr session `{}` carries no host's label, so it is the operator's own \
                     session of that name; this host starts nothing in it (218)",
                    self.session
                ),
            };
        }
        Command::new(&self.binary)
            .args(["--session", &self.session, "server"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .with_context(|| format!("starting the herdr session `{}` headless", self.session))?;
        // The server is listed the moment its socket is up; this is the wait
        // for that and nothing else, so a herdr that answers at once costs no
        // pause at all.
        for _ in 0..100 {
            if self.session_running()? {
                // The session this host made is marked its own, once, so a
                // second host of another instance by this name finds it and
                // declines rather than opening panes beside these (218).
                let made = self.run(&["workspace", "create", "--label", &host_label(host), "--no-focus"])?;
                // The workspace herdr makes comes with a tab of its own,
                // labelled by its number. That tab carries the host's mark
                // too, so reconciliation reads it as the mark it is rather
                // than as a tab naming work that has left every view — which
                // is what closed the mark, and with it the session's hold on
                // its own name (218, 186, 196).
                if let Ok(tab) = text_at(&made, &["/result/tab/tab_id", "/result/tab_id"]) {
                    self.rename_tab(&tab, &host_label(host))?;
                }
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        bail!(
            "the herdr session `{}` did not come up after `{} --session {} server` (174)",
            self.session,
            self.binary,
            self.session
        )
    }

    /// Whose session this is, read from its workspace labels (218).
    pub fn whose(&self, host: &str) -> Result<Own> {
        let mine = host_label(host);
        let mut another = None;
        for (_, label) in self.workspaces()? {
            if label == mine {
                return Ok(Own::Mine);
            }
            if let Some(other) = label.strip_prefix("host/") {
                another = Some(other.to_string());
            }
        }
        Ok(match another {
            Some(other) => Own::AnotherHost(other),
            None => Own::TheOperators,
        })
    }

    /// This session's workspaces: each by its id and its label.
    pub fn workspaces(&self) -> Result<Vec<(String, String)>> {
        let listed = self.run(&["workspace", "list"])?;
        Ok(listed
            .pointer("/result/workspaces")
            .and_then(|v| v.as_array())
            .map(|ws| {
                ws.iter()
                    .filter_map(|w| {
                        let id = w.get("workspace_id").and_then(|v| v.as_str())?;
                        let label = w.get("label").and_then(|v| v.as_str()).unwrap_or_default();
                        Some((id.to_string(), label.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// The workspace carrying a label, where one does.
    pub fn workspace_by_label(&self, label: &str) -> Result<Option<String>> {
        Ok(self
            .workspaces()?
            .into_iter()
            .find(|(_, held)| held == label)
            .map(|(id, _)| id))
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

    /// A workspace's tabs: each by its id and its label. A tab's listing names
    /// no pane of its own (herdr 0.9), so its panes are read from `pane list`.
    pub fn tabs(&self, workspace: &str) -> Result<Vec<(String, String)>> {
        let listed = self.run(&["tab", "list", "--workspace", workspace])?;
        Ok(listed
            .pointer("/result/tabs")
            .and_then(|v| v.as_array())
            .map(|ts| {
                ts.iter()
                    .filter_map(|t| {
                        let id = t.get("tab_id").and_then(|v| v.as_str())?;
                        let label = t.get("label").and_then(|v| v.as_str()).unwrap_or_default();
                        Some((id.to_string(), label.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// A workspace's panes: each by its id and the tab it is in.
    pub fn panes(&self, workspace: &str) -> Result<Vec<(String, String)>> {
        let listed = self.run(&["pane", "list", "--workspace", workspace])?;
        Ok(listed
            .pointer("/result/panes")
            .and_then(|v| v.as_array())
            .map(|ps| {
                ps.iter()
                    .filter_map(|p| {
                        let pane = p.get("pane_id").and_then(|v| v.as_str())?;
                        let tab = p.get("tab_id").and_then(|v| v.as_str()).unwrap_or_default();
                        Some((pane.to_string(), tab.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// The tab of a workspace carrying a label, with a pane of its own where
    /// it has one: the pane a further session of that tab is split off (196).
    pub fn tab_by_label(&self, workspace: &str, label: &str) -> Result<Option<(String, Option<String>)>> {
        let Some((tab, _)) = self.tabs(workspace)?.into_iter().find(|(_, held)| held == label) else {
            return Ok(None);
        };
        let pane = self
            .panes(workspace)?
            .into_iter()
            .find(|(_, at)| at == &tab)
            .map(|(pane, _)| pane);
        Ok(Some((tab, pane)))
    }

    /// A tab in a workspace at a place, labelled; returns the tab and its pane.
    pub fn create_tab(&self, workspace: &str, cwd: &Path, label: &str) -> Result<(String, String)> {
        let made = self.run(&["tab", "create", "--workspace", workspace, "--cwd", &cwd.to_string_lossy(), "--label", label, "--no-focus"])?;
        let tab = text_at(&made, &["/result/tab/tab_id", "/result/tab_id"])?;
        let pane = text_at(&made, &["/result/root_pane/pane_id", "/result/pane/pane_id", "/result/pane_id"])?;
        Ok((tab, pane))
    }

    /// A further session of a tab takes a pane split off the one it has
    /// (196, `sessions.yaml` layout.pane).
    pub fn split_pane(&self, pane: &str, cwd: &Path) -> Result<String> {
        let made = self.run(&[
            "pane",
            "split",
            pane,
            "--direction",
            "right",
            "--cwd",
            &cwd.to_string_lossy(),
            "--no-focus",
        ])?;
        text_at(&made, &["/result/pane/pane_id", "/result/pane_id"])
    }

    pub fn rename_tab(&self, tab: &str, label: &str) -> Result<()> {
        self.run(&["tab", "rename", tab, label]).map(|_| ())
    }

    pub fn close_tab(&self, tab: &str) -> Result<()> {
        self.run(&["tab", "close", tab]).map(|_| ())
    }

    pub fn close_workspace(&self, workspace: &str) -> Result<()> {
        self.run(&["workspace", "close", workspace]).map(|_| ())
    }

    /// Every agent this session lists, as Herdr reports them.
    pub fn agents(&self) -> Result<Vec<Value>> {
        let listed = self.run(&["agent", "list"])?;
        Ok(listed
            .pointer("/result/agents")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default())
    }

    /// The agent by name, or none when Herdr lists no such agent.
    pub fn agent(&self, name: &str) -> Result<Option<Value>> {
        match self.call(&["agent", "get", name])? {
            Ok(v) => Ok(Some(v.pointer("/result/agent").cloned().unwrap_or(v))),
            Err(said) if said.contains("not found") => Ok(None),
            Err(said) => bail!("herdr agent get {name}: {said}"),
        }
    }

    /// `herdr agent start <name> --kind <kind> --pane <pane> -- <the kind's
    /// own arguments>`: returns once the agent is ready for input (72).
    ///
    /// What follows `--` is the program's own command line, naming the agent
    /// and the model the machinery resolved for this session, so the program
    /// runs what the flywheel chose and not what its own configuration says
    /// (173, 183, `sessions.yaml` kinds).
    pub fn start_agent(&self, name: &str, kind: &str, pane: &str, arguments: &[String]) -> Result<()> {
        let mut call: Vec<&str> =
            vec!["agent", "start", name, "--kind", kind, "--pane", pane, "--timeout", "120000"];
        if !arguments.is_empty() {
            call.push("--");
            call.extend(arguments.iter().map(String::as_str));
        }
        self.run(&call).map(|_| ())
    }

    /// Text to a living agent, submitted as one prompt (197).
    pub fn prompt(&self, name: &str, text: &str) -> Result<()> {
        self.run(&["agent", "prompt", name, text]).map(|_| ())
    }

    pub fn close_pane(&self, pane: &str) -> Result<()> {
        self.run(&["pane", "close", pane]).map(|_| ())
    }

    /// The pane's visible text, plain: what the agent is showing (196). Read
    /// in the session the pane is in, like every other call (174).
    pub fn read_visible(&self, name: &str) -> Result<String> {
        let out = Command::new(&self.binary)
            .args([
                "--session",
                &self.session,
                "agent",
                "read",
                name,
                "--source",
                "visible",
                "--lines",
                "60",
            ])
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

    /// Block until the agent's status is something other than `current`, for
    /// at most `timeout_ms`: `Some(status)` when it changed, `None` when the
    /// wait ran out with nothing changed, and an error when Herdr no longer
    /// lists the agent (S221). This is Herdr's own wait, so a host learns of
    /// an agent going idle the moment Herdr does and not at its next poll.
    pub fn wait_change(&self, name: &str, current: &str, timeout_ms: u64) -> Result<Option<String>> {
        let mut args: Vec<String> = vec!["agent".into(), "wait".into(), name.into()];
        for status in ["working", "idle", "blocked", "done", "unknown"] {
            if status != current {
                args.push("--until".into());
                args.push(status.into());
            }
        }
        args.push("--timeout".into());
        args.push(timeout_ms.to_string());
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        match self.call(&borrowed)? {
            Ok(v) => {
                let status = ["/result/agent/agent_status", "/result/agent_status", "/result/status"]
                    .iter()
                    .find_map(|at| v.pointer(at).and_then(|s| s.as_str()).map(String::from));
                match status {
                    Some(status) => Ok(Some(status)),
                    None => Ok(Some(self.status(name)?)),
                }
            }
            Err(said) if said.contains("timed out") || said.contains("timeout") => Ok(None),
            Err(said) => bail!("herdr agent wait {name}: {said}"),
        }
    }

    /// An agent just started may stand at its own first-run question before it
    /// takes a prompt: Claude Code asks whether the folder is trusted, and a
    /// place the machinery made is. The question is answered from what the
    /// pane shows, and the prompt waits until the agent is at its prompt line
    /// (72, 196, 197). What Herdr shows is evidence, so a pane showing nothing
    /// of the kind is left alone.
    pub fn clear_the_way(&self, name: &str) -> Result<bool> {
        self.clear_the_way_every(name, std::time::Duration::from_millis(1000))
    }

    /// The same, looking at the pane once per `pause`, thirty times at most.
    pub fn clear_the_way_every(&self, name: &str, pause: std::time::Duration) -> Result<bool> {
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
            std::thread::sleep(pause);
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

/// Where a session's pane goes: the herdr session by who charged it, the
/// workspace for the bolt or the intent, the tab for the unit or the
/// elaboration (174, 196, layout). A machinery run reads one workspace of its
/// kind — `curation`, `planning/<repository>`, `capture-reading` — with a tab
/// per run.
#[derive(Debug, Clone)]
pub struct Placement {
    /// The herdr session the pane is started in (174).
    pub session: String,
    pub workspace_label: String,
    pub tab_label: String,
    pub cwd: std::path::PathBuf,
    /// The agent kind: claude, codex or opencode (173).
    pub kind: String,
}

/// The program's own arguments, as the kind's command names them
/// (`sessions.yaml` kinds): the agent definition and the model the machinery
/// resolved for this session, handed to the program after `--` so what it runs
/// is the flywheel's choice and not its own configuration (173, 183).
fn program_arguments(order: &WorkOrder) -> Vec<String> {
    let mut out = Vec::new();
    let named = |flag: &str, value: &Option<String>, out: &mut Vec<String>| {
        if let Some(value) = value.as_deref().filter(|v| !v.is_empty()) {
            out.push(flag.to_string());
            out.push(value.to_string());
        }
    };
    match order.program.as_str() {
        // claude and opencode name the agent the same way; codex names it a
        // profile. The model is the same flag for all three.
        "codex" => named("--profile", &order.agent, &mut out),
        _ => named("--agent", &order.agent, &mut out),
    }
    named("--model", &order.model, &mut out);
    out
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
    // Every call below is at the session this placement names, and the session
    // is started headless first: a socket command against a stopped session is
    // refused, which is the signal to start it (174).
    let herdr = &herdr.in_session(&placement.session);
    herdr.ensure_session(host)?;
    let name = agent_name(&order.session);
    let mut pane = None;
    let mut workspace = None;
    let mut fresh = false;
    if herdr.agent(&name)?.is_none() {
        let (ws, tab, pane_id) = match herdr.workspace_by_label(&placement.workspace_label)? {
            Some(ws) => match herdr.tab_by_label(&ws, &placement.tab_label)? {
                // A further session of the same tab is a split of its pane, so
                // two agents of one unit sit side by side (196, layout.pane).
                Some((tab, Some(at))) => {
                    let pane = herdr.split_pane(&at, &placement.cwd)?;
                    (ws, tab, pane)
                }
                _ => {
                    let (tab, pane) = herdr.create_tab(&ws, &placement.cwd, &placement.tab_label)?;
                    (ws, tab, pane)
                }
            },
            None => {
                let (ws, tab, pane) = herdr.create_workspace(&placement.cwd, &placement.workspace_label)?;
                let _ = herdr.rename_tab(&tab, &placement.tab_label);
                (ws, tab, pane)
            }
        };
        // An agent stopped at its own first-run question is "blocked during
        // startup" to Herdr and not a failure here: the question is answered
        // below and the agent is then ready (72, 196).
        match herdr.start_agent(&name, &placement.kind, &pane_id, &program_arguments(order)) {
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
    // The record keeps the herdr session's name beside the workspace, tab and
    // pane ids, so every later call — the pane's evidence, its end, an answer
    // — addresses the same server (174, 196).
    // What it was started as, on the record from the start: the page's chip
    // reads the program and the model from here and never from the program
    // itself (173, 183, S53; `session.yaml` record agent, kind, model).
    let mut fields: Vec<(&str, Value)> = vec![
        ("runner", json!("herdr")),
        ("herdr_agent", json!(name)),
        ("herdr_session", json!(placement.session)),
        ("agent", json!(order.agent)),
        ("kind", json!(order.program)),
        ("model", json!(order.model)),
    ];
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
    // The pane is asked for in the herdr session the record names (174).
    let herdr = &at_recorded(herdr, store, session);
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

/// A session this store says is running with an agent in Herdr: what a host
/// watches (S221), with the herdr session its pane is in so the wait addresses
/// the right server (174).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Live {
    pub session: String,
    pub agent: String,
    /// The herdr session the pane is in, as the record names it.
    pub multiplexer: String,
}

/// Every session this store says is running with an agent in Herdr.
pub fn live_agents<S: Records>(store: &S) -> Vec<Live> {
    let prefix = operator::session_fact("");
    store
        .list_records(&flywheel_atoms::Scope::All)
        .unwrap_or_default()
        .into_iter()
        .filter(|o| o.id.starts_with(&prefix))
        .filter_map(|o| {
            let session = o.id.strip_prefix(&prefix)?.to_string();
            let agent = o.record.get("herdr_agent")?.as_str()?.to_string();
            let multiplexer = o
                .record
                .get("herdr_session")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            operator::running(store, &session).then_some(Live {
                session,
                agent,
                multiplexer,
            })
        })
        .collect()
}

/// The multiplexer at the herdr session a session's record names, which is
/// where its pane is (174). A record written before sessions were addressed by
/// name leaves the caller's own session standing.
fn at_recorded<S: Records>(herdr: &Herdr, store: &S, session: &str) -> Herdr {
    match field(store, session, "herdr_session") {
        Some(named) if !named.is_empty() => herdr.in_session(&named),
        _ => herdr.clone(),
    }
}

/// End the pane and close the record: `pane close <pane id>` in the session
/// the record names, which ends the agent with its pane (26, 74, 196).
pub fn end<S: Records>(store: &mut S, herdr: &Herdr, session: &str, now: DateTime<Utc>) -> Result<()> {
    if let Some(pane) = field(store, session, "herdr_pane") {
        // A pane already gone is not an error: the end is what was wanted (73).
        let _ = at_recorded(herdr, store, session).close_pane(&pane);
    }
    operator::end(store, session, now)
}

/// The answer on the thread, and to the living agent as a prompt (68, 70, 197).
pub fn answer<S: Records>(store: &mut S, herdr: &Herdr, session: &str, host: &str, now: DateTime<Utc>, text: &str) -> Result<()> {
    operator::answer(store, session, host, now, text)?;
    if let Some(agent) = field(store, session, "herdr_agent") {
        let herdr = &at_recorded(herdr, store, session);
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
        let herdr = &at_recorded(herdr, store, session);
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

    /// The herdr session these tests address, as a machinery run's would be
    /// named on an instance called agentplot (174).
    const SESSION: &str = "flywheel-agentplot-machinery";

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
        let herdr = Herdr::at("flywheel-agentplot-machinery").with_binary("/nonexistent/herdr");
        // No fact at all: absent, as the operator binding says.
        assert_eq!(evidence(&store, &herdr, "unit/atlas/u/fix/1", "session.pane"), Some(json!("absent")));
        assert_eq!(evidence(&store, &herdr, "unit/atlas/u/fix/1", "session.exit"), Some(json!("none")));
    }

    #[test]
    fn a_session_that_reported_done_is_exited_whatever_the_pane_says() {
        let mut store = FakeStore::default();
        let herdr = Herdr::at("flywheel-agentplot-machinery").with_binary("/nonexistent/herdr");
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

    /// A `herdr` of the test's own: a script in a directory of its own that
    /// writes every call it is given to `calls` and answers from `body`.
    struct Fake {
        dir: std::path::PathBuf,
        herdr: Herdr,
    }

    impl Fake {
        fn new(name: &str, body: &str) -> Fake {
            use std::os::unix::fs::PermissionsExt;
            let dir = std::env::temp_dir().join(format!(
                "flywheel-herdr-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let script = dir.join("herdr");
            std::fs::write(
                &script,
                format!("#!/bin/sh\nD=\"{}\"\necho \"$@\" >> \"$D/calls\"\n{body}\n", dir.display()),
            )
            .unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            let herdr = Herdr::at(SESSION).with_binary(&script.display().to_string());
            Fake { dir, herdr }
        }

        fn calls(&self) -> String {
            std::fs::read_to_string(self.dir.join("calls")).unwrap_or_default()
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// A wait asks Herdr for every status but the one the agent stands at, and
    /// returns the one it changed to (S221).
    #[test]
    fn a_wait_returns_the_status_the_agent_changed_to() {
        let fake = Fake::new(
            "wait",
            r#"echo '{"result":{"agent":{"agent_status":"idle"}}}'"#,
        );
        assert_eq!(fake.herdr.wait_change("u-fix-1", "working", 500).unwrap().as_deref(), Some("idle"));
        let calls = fake.calls();
        assert!(
            calls.contains("agent wait u-fix-1 --until idle --until blocked --until done --until unknown --timeout 500"),
            "{calls}"
        );
        assert!(!calls.contains("--until working"), "the status it stands at is not waited for: {calls}");
    }

    /// A wait that runs out with nothing changed is none; a wait on an agent
    /// Herdr no longer lists is an error, which frees its watcher (S221).
    #[test]
    fn a_wait_that_runs_out_is_none_and_one_on_a_gone_agent_is_an_error() {
        let fake = Fake::new(
            "timeout",
            r#"case "$*" in
  *"agent wait gone"*) echo '{"error":{"message":"agent gone not found"}}' >&2; exit 1 ;;
esac
echo '{"error":{"message":"timed out after 500ms"}}' >&2; exit 1"#,
        );
        assert_eq!(fake.herdr.wait_change("u-fix-1", "working", 500).unwrap(), None);
        assert!(fake.herdr.wait_change("gone", "working", 500).is_err());
    }

    /// The agents a host watches are the running sessions this binding started
    /// in Herdr: not one that ended, not one that reported done, and not one
    /// the operator runs (72, 150, S221).
    #[test]
    fn the_live_agents_are_the_running_sessions_started_in_herdr() {
        let mut store = FakeStore::default();
        let started = ("started_at", json!("2026-09-14T12:00:00Z"));
        for (session, fields) in [
            ("unit/atlas/a/fix/1", vec![started.clone(), ("ended_at", Value::Null), ("herdr_agent", json!("a-fix-1"))]),
            ("unit/atlas/b/fix/1", vec![started.clone(), ("ended_at", Value::Null)]),
            ("unit/atlas/c/fix/1", vec![started.clone(), ("ended_at", json!("2026-09-14T12:10:00Z")), ("herdr_agent", json!("c-fix-1"))]),
            ("unit/atlas/d/fix/1", vec![started.clone(), ("ended_at", Value::Null), ("herdr_agent", json!("d-fix-1"))]),
        ] {
            operator::set(&mut store, session, &fields).unwrap();
        }
        flywheel_domain::report::write_report(
            &mut store,
            "unit/atlas/d/fix/1",
            "d-fix-1",
            chrono::Utc::now(),
            &flywheel_domain::report::Report::Exit { kind: "done".into(), deliverables: vec![], question: None, text: None },
        )
        .unwrap();

        assert_eq!(
            live_agents(&store),
            vec![Live {
                session: "unit/atlas/a/fix/1".into(),
                agent: "a-fix-1".into(),
                // Started before panes were addressed by name: its pane is the
                // operator's, and the host watches nothing of it (174).
                multiplexer: String::new(),
            }]
        );
    }

    /// Claude Code's question whether the folder is trusted is answered from
    /// what the pane shows, and the way is clear once the agent is at its
    /// prompt (72, 196, 197).
    #[test]
    fn the_trust_question_is_answered_and_the_way_waits_for_the_prompt() {
        let fake = Fake::new(
            "trust",
            r#"case "$*" in
  *"agent get"*) if [ -e "$D/answered" ]; then s=idle; else s=blocked; fi
    echo "{\"result\":{\"agent\":{\"agent_status\":\"$s\"}}}" ;;
  *"agent read"*) echo 'Do you trust the files in this folder?' ;;
  *"agent send-keys"*) touch "$D/answered" ;;
esac"#,
        );
        assert!(fake.herdr.clear_the_way_every("u-fix-1", std::time::Duration::ZERO).unwrap());
        assert!(fake.calls().contains("agent send-keys u-fix-1 down enter"), "{}", fake.calls());
    }

    /// A pane at its prompt, or asking something else, is left alone: what
    /// Herdr shows is evidence, and no key is pressed on a guess (196).
    #[test]
    fn a_pane_not_asking_for_trust_is_left_alone() {
        let idle = Fake::new(
            "idle",
            r#"echo '{"result":{"agent":{"agent_status":"idle"}}}'"#,
        );
        assert!(!idle.herdr.clear_the_way_every("u-fix-1", std::time::Duration::ZERO).unwrap());
        assert!(!idle.calls().contains("send-keys"), "{}", idle.calls());

        let asking = Fake::new(
            "asking",
            r#"case "$*" in
  *"agent get"*) echo '{"result":{"agent":{"agent_status":"blocked"}}}' ;;
  *"agent read"*) echo 'Which file should I change first?' ;;
esac"#,
        );
        assert!(!asking.herdr.clear_the_way_every("u-fix-1", std::time::Duration::ZERO).unwrap());
        assert!(!asking.calls().contains("send-keys"), "{}", asking.calls());
    }

    /// A herdr that answers everything `start` asks: the session running, no
    /// agent until one is started, and the layout ids it hands back. What it
    /// already holds is said by the marker files the test touches first.
    const ANSWERS: &str = r#"case "$*" in
  *"session list"*) echo '{"sessions":[{"name":"flywheel-agentplot-machinery","running":true}]}' ;;
  *"agent get"*) if [ -e "$D/started" ]; then echo '{"result":{"agent":{"agent_status":"idle"}}}';
    else echo '{"error":{"message":"agent not found"}}' >&2; exit 1; fi ;;
  *"workspace list"*) if [ -e "$D/ws" ]; then echo '{"result":{"workspaces":[{"workspace_id":"w0","label":"host/mac-studio"},{"workspace_id":"w1","label":"curation"}]}}';
    else echo '{"result":{"workspaces":[{"workspace_id":"w0","label":"host/mac-studio"}]}}'; fi ;;
  *"workspace create"*) touch "$D/ws"; echo '{"result":{"workspace":{"workspace_id":"w1"},"tab":{"tab_id":"w1:t1"},"root_pane":{"pane_id":"w1:p1"}}}' ;;
  *"tab list"*) if [ -e "$D/tab" ]; then echo '{"result":{"tabs":[{"tab_id":"w1:t1","label":"curation/agentplot/main/1","pane_count":1,"workspace_id":"w1"}]}}';
    else echo '{"result":{"tabs":[]}}'; fi ;;
  *"pane list"*) if [ -e "$D/tab" ]; then echo '{"result":{"panes":[{"pane_id":"w1:p1","tab_id":"w1:t1","workspace_id":"w1"}]}}';
    else echo '{"result":{"panes":[]}}'; fi ;;
  *"tab create"*) touch "$D/tab"; echo '{"result":{"tab":{"tab_id":"w1:t2"},"root_pane":{"pane_id":"w1:p2"}}}' ;;
  *"pane split"*) echo '{"result":{"pane":{"pane_id":"w1:p9"}}}' ;;
  *"agent start"*) touch "$D/started"; echo '{"result":{}}' ;;
  *) echo '{"result":{}}' ;;
esac"#;

    fn order(session: &str) -> WorkOrder {
        WorkOrder {
            session: session.into(),
            kind: "curation".into(),
            place: "curation/agentplot#own".into(),
            agent: Some("curation".into()),
            program: "claude".into(),
            model: Some("claude-fable-5-1".into()),
            body: "the work order".into(),
        }
    }

    fn placement(workspace: &str, tab: &str) -> Placement {
        Placement {
            session: SESSION.into(),
            workspace_label: workspace.into(),
            tab_label: tab.into(),
            cwd: std::env::temp_dir(),
            kind: "claude".into(),
        }
    }

    /// Every call a host makes names the herdr session it is for, so its panes
    /// open in sessions of its own and never in the one its own process runs
    /// in, which is the operator's (174).
    #[test]
    fn every_call_names_the_herdr_session() {
        let fake = Fake::new("addressed", ANSWERS);
        let mut store = FakeStore::default();
        start(
            &mut store,
            &fake.herdr,
            "mac-studio",
            Utc::now(),
            &order("curation/agentplot/main/1"),
            &placement("curation", "curation/agentplot/main/1"),
        )
        .unwrap();
        let calls = fake.calls();
        assert!(calls.contains("agent start"), "the agent was never started: {calls}");
        for line in calls.lines().filter(|l| !l.trim().is_empty()) {
            assert!(
                line.starts_with(&format!("--session {SESSION} ")) || line == "session list --json",
                "a call naming no session runs in whatever session this process sits in: {line}"
            );
        }
        // The record keeps the session's name, so every later call finds the
        // same server (174).
        assert_eq!(
            field(&store, "curation/agentplot/main/1", "herdr_session").as_deref(),
            Some(SESSION)
        );
    }

    /// A session herdr does not list running is started headless as a detached
    /// child before any call against its socket (174).
    #[test]
    fn a_session_not_running_is_started_headless() {
        let fake = Fake::new(
            "headless",
            r#"case "$*" in
  *"session list"*) if [ -e "$D/up" ]; then echo '{"sessions":[{"name":"flywheel-agentplot-machinery","running":true}]}';
    else echo '{"sessions":[{"name":"flywheel-agentplot-machinery","running":false}]}'; fi ;;
  *"workspace list"*) if [ -e "$D/labelled" ]; then echo '{"result":{"workspaces":[{"workspace_id":"w0","label":"host/mac-studio"}]}}';
    else echo '{"result":{"workspaces":[]}}'; fi ;;
  *"workspace create"*) touch "$D/labelled"; echo '{"result":{"workspace":{"workspace_id":"w0"},"tab":{"tab_id":"w0:t1"},"root_pane":{"pane_id":"w0:p1"}}}' ;;
  *"server"*) touch "$D/up"; echo '{"result":{}}' ;;
  *) echo '{"result":{}}' ;;
esac"#,
        );
        fake.herdr.ensure_session("mac-studio").unwrap();
        let calls = fake.calls();
        assert!(
            calls.contains(&format!("--session {SESSION} server")),
            "the session was never started headless: {calls}"
        );
        // The host marks the session it made as its own, once (218).
        assert!(
            calls.contains("workspace create --label host/mac-studio --no-focus"),
            "the session it made carries no host label: {calls}"
        );
        // The marker's own tab carries the mark too, so reconciliation reads
        // it as the mark and never as a tab naming work that has left (218).
        assert!(
            calls.contains("tab rename w0:t1 host/mac-studio"),
            "the marker workspace's tab keeps herdr's own label: {calls}"
        );
        // Already running, so nothing is started a second time.
        let before = fake.calls().matches("server").count();
        fake.herdr.ensure_session("mac-studio").unwrap();
        assert_eq!(fake.calls().matches("server").count(), before);
    }

    /// Curation, planning and capture reading read one workspace each in the
    /// machinery session, with a tab per run (196).
    #[test]
    fn a_curation_run_is_a_tab_of_the_machinery_workspace() {
        let fake = Fake::new("machinery", ANSWERS);
        // The machinery workspace is already there; this run is a tab of it.
        std::fs::write(fake.dir.join("ws"), "").unwrap();
        let mut store = FakeStore::default();
        start(
            &mut store,
            &fake.herdr,
            "mac-studio",
            Utc::now(),
            &order("curation/agentplot/main/1"),
            &placement("curation", "curation/agentplot/main/1"),
        )
        .unwrap();
        let calls = fake.calls();
        assert!(
            calls.contains("tab create --workspace w1")
                && calls.contains("--label curation/agentplot/main/1"),
            "the run is a tab of the machinery workspace: {calls}"
        );
        assert!(!calls.contains("workspace create"), "the workspace is made once: {calls}");
        // The workspace a machinery object reads is its kind's, not its own id.
        assert_eq!(machinery_workspace("curation/agentplot").as_deref(), Some("curation"));
        assert_eq!(machinery_workspace("planning/atlas").as_deref(), Some("planning/atlas"));
        assert_eq!(machinery_workspace("capture/folder/notes/9f").as_deref(), Some("capture-reading"));
        assert_eq!(machinery_workspace("unit/atlas/u"), None);
    }

    /// A further session of a tab takes a pane split off the one it has, so
    /// two agents of one unit sit side by side (196).
    #[test]
    fn a_second_session_of_a_tab_splits_its_pane() {
        let fake = Fake::new("split", ANSWERS);
        std::fs::write(fake.dir.join("ws"), "").unwrap();
        std::fs::write(fake.dir.join("tab"), "").unwrap();
        let mut store = FakeStore::default();
        start(
            &mut store,
            &fake.herdr,
            "mac-studio",
            Utc::now(),
            &order("curation/agentplot/main/2"),
            &placement("curation", "curation/agentplot/main/1"),
        )
        .unwrap();
        let calls = fake.calls();
        assert!(
            calls.contains("pane split w1:p1 --direction right"),
            "the second session splits the tab's pane: {calls}"
        );
        assert!(!calls.contains("tab create"), "the tab is made once: {calls}");
        // The program's own arguments follow `--`: the agent definition and
        // the model the machinery resolved, so the program runs what the
        // flywheel chose and not what its own configuration says (173, 183).
        assert!(
            calls.contains(
                "agent start curation-agentplot-main-2 --kind claude --pane w1:p9 --timeout 120000 \
                 -- --agent curation --model claude-fable-5-1"
            ),
            "{calls}"
        );
    }

    /// Who charged the session names the herdr session it starts in, and a
    /// host's override wins by repository over kind over the default (174).
    #[test]
    fn the_herdr_session_is_named_by_who_charged_it() {
        let none = BTreeMap::new();
        let chain = |ids: &[&str]| ids.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(charged_by("unit/atlas/u", &chain(&["unit/atlas/u", "bolt/atlas/b"])), Charged::Bolts);
        assert_eq!(
            charged_by("elaboration/e", &chain(&["elaboration/e", "intent/i"])),
            Charged::Intents
        );
        assert_eq!(charged_by("curation/agentplot", &chain(&["curation/agentplot"])), Charged::Machinery);
        assert_eq!(
            session_name("agentplot", Charged::Machinery, &none, "claude", "atlas"),
            "flywheel-agentplot-machinery"
        );
        let mut overrides = BTreeMap::new();
        overrides.insert("codex".to_string(), "flywheel-agentplot-codex".to_string());
        assert_eq!(
            session_name("agentplot", Charged::Bolts, &overrides, "codex", "atlas"),
            "flywheel-agentplot-codex"
        );
        // The repository is the more specific of the two, so it wins.
        overrides.insert("atlas".to_string(), "flywheel-atlas".to_string());
        assert_eq!(
            session_name("agentplot", Charged::Bolts, &overrides, "codex", "atlas"),
            "flywheel-atlas"
        );
    }

    /// Ending a pane that is already gone is not an error: the end is what was
    /// wanted (73, 74).
    #[test]
    fn ending_a_pane_already_gone_is_not_an_error() {
        let fake = Fake::new(
            "gone-pane",
            r#"case "$*" in
  *"pane close"*) echo '{"error":{"message":"pane w1:p1 not found"}}' >&2; exit 1 ;;
  *) echo '{"result":{}}' ;;
esac"#,
        );
        let mut store = FakeStore::default();
        operator::set(
            &mut store,
            "curation/agentplot/main/1",
            &[
                ("started_at", json!("2026-09-15T09:00:00Z")),
                ("ended_at", Value::Null),
                ("host", json!("mac-studio")),
                ("herdr_agent", json!("curation-agentplot-main-1")),
                ("herdr_pane", json!("w1:p1")),
                ("herdr_session", json!(SESSION)),
            ],
        )
        .unwrap();

        end(&mut store, &fake.herdr, "curation/agentplot/main/1", Utc::now())
            .expect("a pane already gone is not an error");
        assert!(fake.calls().contains("pane close w1:p1"), "{}", fake.calls());
        // And the record is closed all the same.
        assert!(!operator::running(&store, "curation/agentplot/main/1"));
    }

    /// Reconciliation reads the host's own herdr sessions and no other: every
    /// call it makes names the session it was given (174, 196, 218).
    #[test]
    fn reconciliation_reads_only_the_hosts_own_sessions() {
        let fake = Fake::new(
            "reconcile",
            r#"case "$*" in
  *"agent list"*) echo '{"result":{"agents":[{"name":"curation-agentplot-main-1","pane_id":"w1:p1"}]}}' ;;
  *"workspace list"*) echo '{"result":{"workspaces":[{"workspace_id":"w0","label":"host/mac-studio"},{"workspace_id":"w1","label":"curation"}]}}' ;;
  *"tab list"*) echo '{"result":{"tabs":[{"tab_id":"w1:t1","label":"curation/agentplot/main/1"}]}}' ;;
  *"pane list"*) echo '{"result":{"panes":[{"pane_id":"w1:p1","tab_id":"w1:t1"}]}}' ;;
  *) echo '{"result":{}}' ;;
esac"#,
        );
        let mine = fake.herdr.in_session("flywheel-agentplot-machinery");
        assert_eq!(mine.agents().unwrap().len(), 1);
        assert_eq!(mine.workspaces().unwrap().len(), 2);
        assert_eq!(mine.tabs("w1").unwrap().len(), 1);
        assert_eq!(mine.panes("w1").unwrap().len(), 1);
        // The mark that says the session is this host's (218).
        assert!(mine
            .workspaces()
            .unwrap()
            .iter()
            .any(|(_, label)| label == &host_label("mac-studio")));

        for line in fake.calls().lines().filter(|l| !l.trim().is_empty()) {
            assert!(
                line.starts_with("--session flywheel-agentplot-machinery "),
                "reconciliation reached a session that is not the host's own: {line}"
            );
        }
    }

    /// A call that names no session is refused rather than run against
    /// whatever server this process sits in (174).
    #[test]
    fn an_unbound_herdr_refuses_to_call() {
        let said = Herdr::unbound().agent("u-fix-1").unwrap_err().to_string();
        assert!(said.contains("names no session"), "{said}");
    }
}
