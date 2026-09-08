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
    pub ticks_alive: u64,
    pub idle_since: Option<DateTime<Utc>>,
    pub inbox: Vec<String>,
    pub played: Vec<usize>,
}

/// One thing a scripted session does, at an offset from its start.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ScriptEntry {
    #[serde(default)]
    pub after: Option<String>,
    #[serde(default)]
    pub pane: Option<String>,
    #[serde(default)]
    pub activity: Option<String>,
    #[serde(default)]
    pub exit: Option<String>,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub verdict: Option<String>,
    #[serde(default)]
    pub deliverables: Vec<String>,
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
