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
    /// The objects this decision stands for: itself, and every object folded
    /// into it because its `batch` field carries the same value (11). One
    /// answer applies to all of them.
    #[serde(default)]
    pub folds: Vec<String>,
}

/// One entry of the register: the number a decision was given, when it was
/// raised, and — once it is gone — when it was retracted and by whose response,
/// where a response is what took it away. An entry outlives its decision so a
/// late reply still resolves and is reported rather than applied.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct RegisterEntry {
    pub number: u32,
    #[serde(default)]
    pub since: Option<DateTime<Utc>>,
    /// When the decision stopped standing. `None` while it stands.
    #[serde(default)]
    pub retracted_at: Option<DateTime<Utc>>,
    /// The response that answered it, where one did. An entry retracted with
    /// none was retracted by the machinery: the choice was no longer the
    /// operator's to make.
    #[serde(default)]
    pub answered_by: Option<String>,
    #[serde(default)]
    pub answered_at: Option<DateTime<Utc>>,
}

/// The rail's register: a short number per decision, given once, never reused.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Register {
    pub next_number: u32,
    #[serde(default)]
    pub entries: BTreeMap<String, RegisterEntry>,
}

impl Register {
    /// The number this decision carries, where it has one.
    pub fn number_of(&self, decision_id: &str) -> Option<u32> {
        self.entries.get(decision_id).map(|e| e.number)
    }

    /// The decision a number names, standing or retracted. A number is never
    /// reused, so this answer never changes once given.
    pub fn decision_of(&self, number: u32) -> Option<&str> {
        self.entries
            .iter()
            .find(|(_, e)| e.number == number)
            .map(|(k, _)| k.as_str())
    }

    /// The register as id → number, which is what a reader of the rail record
    /// and a scenario's `given.register` speak in.
    pub fn numbers(&self) -> BTreeMap<String, u32> {
        self.entries
            .iter()
            .map(|(id, e)| (id.clone(), e.number))
            .collect()
    }

    /// Give one decision a number, or return the one it has. The counter only
    /// grows and a number is never reused.
    pub fn number_for(&mut self, decision_id: &str, since: Option<DateTime<Utc>>) -> u32 {
        if let Some(e) = self.entries.get_mut(decision_id) {
            // It stands again: the same decision, not a new one — its id
            // carries the point its state was entered, so a re-entered state
            // is a different id and takes a number of its own.
            e.retracted_at = None;
            return e.number;
        }
        if self.next_number == 0 {
            self.next_number = 1;
        }
        let number = self.next_number;
        self.next_number += 1;
        self.entries.insert(
            decision_id.to_string(),
            RegisterEntry { number, since, ..Default::default() },
        );
        number
    }

    /// Whether any decision standing now has no entry: what the rail machine
    /// reads to know it must number this tick.
    pub fn unnumbered(&self, standing: &[DecisionInstance]) -> bool {
        standing.iter().any(|d| !self.entries.contains_key(&d.id))
    }

    /// Number every standing decision that has none, in the order they stand.
    /// One call is one atomic write of the register and the counter (15).
    pub fn number_all(&mut self, standing: &[DecisionInstance]) -> bool {
        let mut gave = false;
        for decision in standing {
            if self.entries.contains_key(&decision.id) {
                continue;
            }
            self.number_for(&decision.id, Some(decision.since));
            gave = true;
        }
        gave
    }

    /// Mark every entry whose decision no longer stands as retracted, at the
    /// point the tick is reading. An entry already retracted keeps the time it
    /// was, so a decision is retracted once (9, I3).
    pub fn retract_gone(&mut self, standing: &[String], at: DateTime<Utc>) -> Vec<String> {
        let mut retracted = Vec::new();
        for (id, entry) in self.entries.iter_mut() {
            if entry.retracted_at.is_some() || standing.iter().any(|s| s == id) {
                continue;
            }
            entry.retracted_at = Some(at);
            retracted.push(id.clone());
        }
        retracted
    }

    /// Record the response that answered a decision on its entry, so a second
    /// response to that number is refused as already answered and every reader
    /// sees by whom and when.
    pub fn answered(&mut self, decision_id: &str, by: &str, at: DateTime<Utc>) {
        if let Some(entry) = self.entries.get_mut(decision_id) {
            if entry.answered_by.is_none() {
                entry.answered_by = Some(by.to_string());
                entry.answered_at = Some(at);
            }
        }
    }

    /// Whether an entry has been retracted, and when.
    pub fn retracted_at(&self, decision_id: &str) -> Option<DateTime<Utc>> {
        self.entries.get(decision_id).and_then(|e| e.retracted_at)
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
