//! The runtime shape of state the engine reads and writes: objects with an
//! active configuration, responses, and the evidence the state store serves.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// One object as the engine sees it. `config` maps a region path to its
/// active state. A region path is `region` for a top-level region and
/// `region.state.subregion` for a region nested in a state (including the
/// regions of a submachine instantiated in that state).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub id: String,
    pub machine: String,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub config: BTreeMap<String, String>,
    #[serde(default)]
    pub entered_at: BTreeMap<String, DateTime<Utc>>,
    #[serde(default)]
    pub record: BTreeMap<String, Value>,
    #[serde(default)]
    pub counters: BTreeMap<String, i64>,
    #[serde(default)]
    pub applied_responses: Vec<String>,
    #[serde(default)]
    pub seq: u64,
    #[serde(default)]
    pub created: u64,
}

impl Object {
    /// The active state of the first top-level region: what "the object's state" means in prose.
    pub fn top_state(&self) -> Option<&str> {
        self.config
            .iter()
            .filter(|(k, _)| !k.contains('.'))
            .map(|(_, v)| v.as_str())
            .next()
    }
    pub fn top_states(&self) -> Vec<&str> {
        self.config.iter().filter(|(k, _)| !k.contains('.')).map(|(_, v)| v.as_str()).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponseKind {
    Answer,
    Dictation,
}

/// The operator's response: an answer to a numbered decision, or a dictation naming an object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: String,
    pub kind: ResponseKind,
    #[serde(default)]
    pub decision: Option<u32>,
    #[serde(default)]
    pub object: Option<String>,
    pub answer: String,
    #[serde(default)]
    pub given_by: String,
    pub given_at: DateTime<Utc>,
    #[serde(default)]
    pub delivery: String,
}

/// One standing decision, derived from a state that carries `decision:`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecisionInstance {
    pub id: String,
    pub object: String,
    pub region: String,
    pub state: String,
    pub kind: String,
    pub group: String,
    pub answers: Vec<String>,
    pub shows: Vec<String>,
    pub document: Option<Value>,
    pub since: DateTime<Utc>,
    pub number: Option<u32>,
}

/// The rail's register: a short number per decision, given once, never reused.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Register {
    pub next_number: u32,
    pub numbers: BTreeMap<String, u32>,
}

impl Register {
    pub fn number_for(&mut self, decision_id: &str) -> u32 {
        if let Some(n) = self.numbers.get(decision_id) {
            return *n;
        }
        if self.next_number == 0 {
            self.next_number = 1;
        }
        let n = self.next_number;
        self.next_number += 1;
        self.numbers.insert(decision_id.to_string(), n);
        n
    }
    pub fn decision_of(&self, number: u32) -> Option<&str> {
        self.numbers.iter().find(|(_, n)| **n == number).map(|(k, _)| k.as_str())
    }
}

/// An entry in the SINCE tail: a state with `tail:` entered.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TailEntry {
    pub at: DateTime<Utc>,
    pub object: String,
    pub kind: String,
    pub state: String,
    pub by: Option<String>,
}

/// Evidence the state store serves for one object, by atom name.
pub trait EvidenceSource {
    fn evidence(&self, object: &str, region: &str, name: &str) -> Option<Value>;
}

/// Everything one tick reads.
pub struct Snapshot<'a> {
    pub objects: &'a BTreeMap<String, Object>,
    pub responses: &'a [Response],
    pub register: &'a Register,
    pub evidence: &'a dyn EvidenceSource,
    pub now: DateTime<Utc>,
}
