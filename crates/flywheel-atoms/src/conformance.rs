//! The conformance scenario file, as types, faithful to
//! `conformance/schema.json`.
//!
//! A scenario is data — given this evidence, when this tick or event, then
//! these transitions, these effects and these decisions (94). Every vocabulary
//! is closed, so a typo fails validation rather than being ignored in silence.
//!
//! `scenario.rs` beside this holds the prototype's own seed file, which is a
//! different shape for a different job.

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

use crate::scenario::{ScriptEntry, SessionFact};

/// evidence name -> {object id -> value}; `*` for every object.
pub type Evidence = BTreeMap<String, BTreeMap<String, Value>>;
/// path -> a fixture path or inline content.
pub type Files = BTreeMap<String, String>;
/// session id -> what it plays.
pub type Script = BTreeMap<String, Vec<ScriptEntry>>;

/// What a run must provide beyond the store binding (93a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Requirement {
    /// The scenario asserts a real take, merge, rebase, conflict or landing,
    /// or a real place, hook or tethered process.
    RealWorkspace,
    /// It asserts a real agent session rather than a scripted one.
    RealSessions,
}

impl Requirement {
    pub fn reason(&self) -> &'static str {
        match self {
            Requirement::RealWorkspace => {
                "the workspace is recorded, and this asserts a real take, merge, rebase, conflict or landing (93a)"
            }
            Requirement::RealSessions => {
                "the sessions are scripted, and this asserts a real agent session (93)"
            }
        }
    }
}

/// In-process fault injection, allowed only under `contract/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Hook {
    /// The acting hosts attempt their writes without taking the lease first
    /// (B.2.134).
    BypassLease,
    /// The acting host's write is cut off midway (B.2.135).
    InterruptWrite,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Scenario {
    pub scenario: String,
    pub title: String,
    /// The state store bindings this scenario runs on, and nothing else.
    pub profiles: Vec<String>,
    #[serde(default)]
    pub requires: Vec<Requirement>,
    #[serde(default)]
    pub hooks: Vec<Hook>,
    pub satisfies: Vec<u32>,
    #[serde(default)]
    pub invariants: Vec<String>,
    /// The machines directory to load; `../machines` by default, and
    /// `./contract/lamp` for the contract set.
    #[serde(default)]
    pub machines: Option<String>,
    #[serde(default)]
    pub given: Given,
    pub when: Vec<BTreeMap<String, Value>>,
    #[serde(default)]
    pub then: Then,
}

impl Scenario {
    /// Whether the scenario applies to a store binding.
    pub fn runs_on(&self, profile: &str) -> bool {
        self.profiles.iter().any(|p| p == "all" || p == profile)
    }

    /// The requirement the bound implementations do not provide, if any.
    pub fn unmet(&self, provided: &[Requirement]) -> Option<Requirement> {
        self.requires.iter().find(|r| !provided.contains(r)).copied()
    }

    /// The steps, parsed. An unknown key is an error, never a silent skip.
    pub fn steps(&self) -> Result<Vec<Step>> {
        self.when
            .iter()
            .enumerate()
            .map(|(n, raw)| {
                Step::parse(raw).with_context(|| format!("step {}", n + 1))
            })
            .collect()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Given {
    #[serde(default)]
    pub objects: Vec<GivenObject>,
    #[serde(default)]
    pub evidence: Evidence,
    #[serde(default)]
    pub leases: Vec<BTreeMap<String, Value>>,
    #[serde(default)]
    pub hosts: Vec<BTreeMap<String, Value>>,
    #[serde(default)]
    pub sinks: Vec<BTreeMap<String, Value>>,
    #[serde(default)]
    pub responses: Vec<BTreeMap<String, Value>>,
    /// The rail's register: decision id -> number, and `next`.
    #[serde(default)]
    pub register: BTreeMap<String, Value>,
    /// sink -> delivery mark.
    #[serde(default)]
    pub marks: BTreeMap<String, String>,
    #[serde(default)]
    pub files: Files,
    #[serde(default)]
    pub script: Script,
    /// What the world reports about a session at the start.
    #[serde(default)]
    pub sessions: BTreeMap<String, SessionFact>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GivenObject {
    pub id: String,
    pub machine: String,
    #[serde(default)]
    pub parent: Option<String>,
    pub state: BTreeMap<String, String>,
    #[serde(default)]
    pub record: BTreeMap<String, Value>,
    #[serde(default)]
    pub applied_responses: Vec<String>,
}

// ------------------------------------------------------------------ the steps

/// One `when` step. The set is closed and each step carries one key.
#[derive(Debug, Clone)]
pub enum Step {
    Tick(Tick),
    Response(ResponseStep),
    Evidence(Evidence),
    Notify(BTreeMap<String, Value>),
    Restart,
    Disconnect,
    Reconnect,
    Host(HostStep),
    Clock(Clock),
    Direct(Direct),
    Files(Files),
    Script(Script),
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tick {
    /// These hosts tick at the same moment against the same store, which is how
    /// a race is written. Concurrency is never a property of the acting host.
    #[serde(default)]
    pub concurrent_hosts: Vec<String>,
    /// How far this tick moves the virtual clock; the run's interval otherwise.
    #[serde(default)]
    pub interval: Option<String>,
    /// Run the named scenario file to completion in place of ticking (95).
    #[serde(default)]
    pub run: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseStep {
    /// `<object id>/<decision kind>`, resolved through the register at the
    /// moment the step runs.
    #[serde(default)]
    pub decision: Option<String>,
    /// The number the register gave, for answering the same decision again (15).
    #[serde(default)]
    pub number: Option<u32>,
    /// For a dictation: the object it names.
    #[serde(default)]
    pub object: Option<String>,
    pub answer: String,
    pub id: String,
    #[serde(default)]
    pub by: Option<String>,
}

/// `{name}` alone sets the acting host; at most one transition is performed.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostStep {
    pub name: String,
    #[serde(default)]
    pub start: bool,
    #[serde(default)]
    pub lose: bool,
    #[serde(default)]
    pub disconnect: bool,
    #[serde(default)]
    pub r#return: bool,
}

impl HostStep {
    /// The one transition, if the step performs one. Two is an error.
    pub fn transition(&self) -> Result<Option<HostTransition>> {
        let named: Vec<HostTransition> = [
            (self.start, HostTransition::Start),
            (self.lose, HostTransition::Lose),
            (self.disconnect, HostTransition::Disconnect),
            (self.r#return, HostTransition::Return),
        ]
        .into_iter()
        .filter_map(|(set, t)| set.then_some(t))
        .collect();
        match named.len() {
            0 => Ok(None),
            1 => Ok(Some(named[0])),
            _ => bail!(
                "a host step carries at most one of start, lose, disconnect, return; \
                 this one carries {named:?}"
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostTransition {
    Start,
    Lose,
    Disconnect,
    Return,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Clock {
    /// How far the virtual clock moves: 30s, 6m, 1d.
    pub advance: String,
    /// The time of day it lands on, for a cadence a scenario must hit exactly.
    #[serde(default)]
    pub at: Option<String>,
}

/// Something acting outside the machinery's own loop, which the machinery must
/// then read as it finds it. `do` names which, and each arm carries its own
/// fields, so a misspelled key fails validation rather than being ignored.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "do", rename_all = "snake_case", deny_unknown_fields)]
pub enum Direct {
    /// The operator said this in chat (194).
    Dictation {
        text: String,
        #[serde(default)]
        by: Option<String>,
    },
    /// An adapter's own binary writing a capture, from any machine (217g).
    Adapter {
        command: String,
        #[serde(default)]
        by: Option<String>,
    },
    /// The operator edited the state where it is kept and committed (3, 159).
    Commit {
        file: String,
        #[serde(default)]
        set: BTreeMap<String, Value>,
        #[serde(default)]
        sha: Option<String>,
        #[serde(default)]
        by: Option<String>,
    },
    /// The operator moved a card on the tracker's board (159).
    Board {
        issue: String,
        column: String,
        #[serde(default)]
        event_id: Option<String>,
        #[serde(default)]
        by: Option<String>,
    },
    /// The operator closed the issue behind an object.
    Close {
        issue: String,
        #[serde(default)]
        event_id: Option<String>,
        #[serde(default)]
        by: Option<String>,
    },
    /// The operator opened a page route.
    Page {
        path: String,
        #[serde(default)]
        by: Option<String>,
    },
    /// The operator ran something by hand, outside every tool (S15, X08).
    Shell {
        command: String,
        #[serde(default)]
        by: Option<String>,
    },
    /// A projection drifted from the state it projects (136).
    Projection {
        object: String,
        set: BTreeMap<String, Value>,
        #[serde(default)]
        by: Option<String>,
    },
    /// Something else changed the object in the store (130).
    Store {
        object: String,
        set: BTreeMap<String, Value>,
        #[serde(default)]
        by: Option<String>,
    },
}

impl Step {
    /// Parse one step's single key. Every name is closed.
    pub fn parse(raw: &BTreeMap<String, Value>) -> Result<Step> {
        if raw.len() != 1 {
            bail!(
                "a step carries exactly one key; this one carries {:?}",
                raw.keys().collect::<Vec<_>>()
            );
        }
        let (key, value) = raw.iter().next().expect("one key");
        macro_rules! arm {
            ($variant:path, $name:literal) => {
                $variant(serde_json::from_value(value.clone()).context($name)?)
            };
        }
        Ok(match key.as_str() {
            "tick" => arm!(Step::Tick, "tick"),
            "response" => arm!(Step::Response, "response"),
            "evidence" => arm!(Step::Evidence, "evidence"),
            "notify" => arm!(Step::Notify, "notify"),
            "restart" => Step::Restart,
            "disconnect" => Step::Disconnect,
            "reconnect" => Step::Reconnect,
            "host" => arm!(Step::Host, "host"),
            "clock" => arm!(Step::Clock, "clock"),
            "direct" => arm!(Step::Direct, "direct"),
            "files" => arm!(Step::Files, "files"),
            "script" => arm!(Step::Script, "script"),
            other => {
                return Err(anyhow!(
                    "`{other}` is not a step; the steps are tick, response, evidence, notify, \
                     restart, disconnect, reconnect, host, clock, direct, files, script"
                ))
            }
        })
    }
}

// ------------------------------------------------------------------- the then

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Then {
    /// In order, across all ticks.
    #[serde(default)]
    pub transitions: Vec<TransitionExpectation>,
    #[serde(default)]
    pub no_transitions: Vec<String>,
    /// By atom name, with a count; a repeat must not add.
    #[serde(default)]
    pub effects: Vec<EffectExpectation>,
    /// When true, an effect the scenario does not list is asserted absent.
    #[serde(default)]
    pub effects_closed: bool,
    #[serde(default)]
    pub decisions: Vec<DecisionExpectation>,
    /// The state a region is in, by object. A scenario names the state as the
    /// machine file writes it; a value that is not text — a version, a count —
    /// is compared as it reads.
    #[serde(default)]
    pub states: BTreeMap<String, BTreeMap<String, Value>>,
    #[serde(default)]
    pub records: BTreeMap<String, Value>,
    #[serde(default)]
    pub applied_responses: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub writes: BTreeMap<String, Value>,
    #[serde(default)]
    pub reports: Vec<String>,
    #[serde(default)]
    pub leases: BTreeMap<String, Value>,
    #[serde(default)]
    pub status: BTreeMap<String, Value>,
    /// Assertions on the profile's own behaviour. Every key is bound in
    /// `observations.yaml`.
    #[serde(default)]
    pub state_store: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TransitionExpectation {
    pub object: String,
    #[serde(default)]
    pub from: Option<String>,
    pub to: String,
    #[serde(default)]
    pub region: Option<String>,
    /// The response that caused it, where one did.
    #[serde(default)]
    pub response: Option<String>,
    /// The host that made it, where a scenario says which — two hosts of one
    /// instance write on the same line (147, 232, S18).
    #[serde(default)]
    pub host: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectExpectation {
    pub r#do: String,
    pub count: usize,
    #[serde(default)]
    pub object: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionExpectation {
    /// The step boundary the assertion is made at, counting from 1.
    #[serde(default)]
    pub after_step: Option<usize>,
    #[serde(default)]
    pub present: Vec<String>,
    #[serde(default)]
    pub absent: Vec<String>,
    #[serde(default)]
    pub count: Option<usize>,
    #[serde(default)]
    pub tail: Vec<Value>,
    #[serde(default)]
    pub numbers: BTreeMap<String, u32>,
}

/// Read a scenario file. The bytes are also returned, because the schema is
/// checked against the document and not against these types.
pub fn load(path: &std::path::Path) -> Result<(Scenario, Value)> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let document: Value = serde_yaml::from_str(&text)
        .with_context(|| format!("parsing {}", path.display()))?;
    let scenario: Scenario = serde_json::from_value(document.clone())
        .with_context(|| format!("reading {} as a scenario", path.display()))?;
    Ok((scenario, document))
}
