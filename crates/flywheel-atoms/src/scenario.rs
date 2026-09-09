//! The scenario file, as types. A scenario is data — given this evidence, when
//! this tick or event, then these transitions, these effects and these
//! decisions (94) — so its shape lives beside the atoms and not inside any one
//! runner.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct Scenario {
    pub scenario: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub given: Given,
    #[serde(default)]
    pub when: Vec<BTreeMap<String, Value>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct Given {
    #[serde(default)]
    pub objects: Vec<GivenObject>,
    #[serde(default)]
    pub evidence: BTreeMap<String, BTreeMap<String, Value>>,
    #[serde(default)]
    pub script: BTreeMap<String, Vec<ScriptEntry>>,
    #[serde(default)]
    pub sessions: BTreeMap<String, SessionFact>,
    /// Service declarations per repository: what the repository's service data
    /// would say.
    #[serde(default)]
    pub services: BTreeMap<String, Vec<ServiceDecl>>,
    #[serde(default)]
    pub hosts: Vec<BTreeMap<String, Value>>,
    #[serde(default)]
    pub tail: Vec<flywheel_engine::TailEntry>,
    #[serde(default)]
    pub now: Option<DateTime<Utc>>,
    #[serde(default)]
    pub register_start: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct GivenObject {
    pub id: String,
    pub machine: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub state: BTreeMap<String, String>,
    #[serde(default)]
    pub record: BTreeMap<String, Value>,
    #[serde(default)]
    pub entered: Option<String>,
}

/// What the world reports about one session.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SessionFact {
    pub pane: bool,
    pub activity: String,
    pub exit: Option<String>,
    pub question: Option<String>,
    pub verdict: Option<String>,
    pub deliverables: Vec<String>,
    /// What the session committed in its place: what a real one would deliver
    /// as records, which the stand-in carries as the lines the script gave it.
    #[serde(default)]
    pub commits: Vec<String>,
    pub ticks_alive: u64,
    pub idle_since: Option<DateTime<Utc>>,
    pub inbox: Vec<String>,
    pub played: Vec<usize>,
    /// A script has spoken for this session. What a scenario says about one
    /// session by name is more particular than what it says about all of them
    /// at once, so a wildcard does not override it.
    #[serde(default)]
    pub scripted: bool,
}

/// One thing a scripted session does. `after` is a **step number**, never a
/// duration: the clock moves only through a `clock` step and the tick
/// interval, so a delay is written as a `clock` step in `when`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ScriptEntry {
    #[serde(default)]
    pub after: Option<String>,
    /// `present` or `absent`, as the multiplexer would report it.
    #[serde(default)]
    pub pane: Option<String>,
    /// `working` or `idle`.
    #[serde(default)]
    pub activity: Option<String>,
    /// The operator typed in the pane.
    #[serde(default)]
    pub keystroke: bool,
    /// `done`, `blocked`, `stalled` or `invalid`, reported by running the
    /// command a real session reports through (67, 93).
    #[serde(default)]
    pub exit: Option<String>,
    #[serde(default)]
    pub deliverables: Vec<String>,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub verdict: Option<String>,
    /// Findings and chores the session offered, each pointing at a document.
    #[serde(default)]
    pub offers: Vec<Offer>,
    #[serde(default)]
    pub refusal: Option<String>,
    /// The session ran `flywheel service start|stop <name>` (48).
    #[serde(default)]
    pub service: Option<BTreeMap<String, String>>,
    /// Files the session committed in its place.
    #[serde(default)]
    pub commits: Vec<String>,
}

/// One thing a session offered: its kind and the document it points at (58,
/// 59, 62). The record never holds the text.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Offer {
    pub kind: String,
    pub document: String,
}

/// One service declaration a repository carries. `fails: true` scripts a
/// process that exits instead of serving.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ServiceDecl {
    pub name: String,
    pub command: String,
    pub serves: String,
    pub port: Option<u16>,
    pub fails: bool,
}

pub fn load(path: &std::path::Path) -> Result<Scenario> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_yaml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}
