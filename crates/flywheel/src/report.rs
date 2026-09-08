//! What a session reports, and the one path it reports through.
//!
//! A session's only outputs to the machinery are a fixed set of exits — done
//! with deliverables, blocked on a question, offering a finding, offering a
//! chore, stalled (65). It reports them through a command the machinery
//! provides, which writes to the state store; nothing it leaves on the place's
//! disk is state (67). The command never moves the machinery's state: it
//! writes one thread entry and the machinery decides what the entry means (66).
//!
//! In phase 1 the operator is the session (93b, D8), so this is the command the
//! operator runs; the scripted stand-in runs the same binary, and the phase-2
//! runner will call it unchanged.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use flywheel_atoms::{Records, ThreadEntry};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The environment variable a rendered work order tells a session to report
/// under (89). One name, so the operator, the stand-in and the phase-2 runner
/// all use it.
pub const SESSION_ENV: &str = "FLYWHEEL_SESSION";

/// The exits of 65, as the machine reads them. `invalid` is not one a session
/// may report: it is what the machinery records when the report is none of the
/// others (80).
pub const EXITS: &[&str] = &["done", "blocked", "stalled"];

/// What a session offers rather than exits on: the other two of 65's five.
pub const OFFERS: &[&str] = &["finding", "chore"];

/// One report, before it is a thread entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    /// `done`, `blocked` or `stalled`, with what each carries.
    Exit {
        kind: String,
        deliverables: Vec<String>,
        question: Option<String>,
        text: Option<String>,
    },
    /// A finding or a chore, pointing at its document; the record never holds
    /// the text (58, 59, 62).
    Offer { kind: String, document: String },
    /// Something said on the thread that is not an exit.
    Note { text: String },
    /// The session refused the work it was given (43).
    Refuse { reason: String },
}

/// What became of a report. An unknown exit is refused and the refusal is
/// recorded, so nothing is dropped and nothing is guessed at (66, 80).
#[derive(Debug, Clone, PartialEq)]
pub enum Reported {
    Accepted(ThreadEntry),
    Refused { entry: ThreadEntry, reason: String },
}

impl Reported {
    pub fn entry(&self) -> &ThreadEntry {
        match self {
            Reported::Accepted(e) => e,
            Reported::Refused { entry, .. } => entry,
        }
    }
}

fn entry(at: DateTime<Utc>, kind: &str, by: &str, fields: BTreeMap<String, Value>) -> ThreadEntry {
    ThreadEntry {
        at,
        kind: kind.to_string(),
        by: Some(by.to_string()),
        fields,
    }
}

/// Write one report as one thread entry on the session, through `append`.
pub fn write_report(
    store: &mut impl Records,
    session: &str,
    by: &str,
    at: DateTime<Utc>,
    report: &Report,
) -> Result<Reported> {
    if session.is_empty() {
        return Err(anyhow!(
            "no session: pass --session or set {SESSION_ENV}, which the work order names"
        ));
    }
    let mut fields: BTreeMap<String, Value> = BTreeMap::new();
    let reported = match report {
        Report::Exit {
            kind,
            deliverables,
            question,
            text,
        } => {
            if EXITS.contains(&kind.as_str()) {
                fields.insert("exit".into(), json!(kind));
                if !deliverables.is_empty() {
                    fields.insert("deliverables".into(), json!(deliverables));
                }
                if let Some(q) = question {
                    fields.insert("question".into(), json!(q));
                }
                if let Some(t) = text {
                    fields.insert("text".into(), json!(t));
                }
                Reported::Accepted(entry(at, "exit", by, fields))
            } else {
                // A report that is none of the exits of 65 is recorded as
                // invalid, with the raw text, and the refusal stands in the
                // record rather than being dropped (66, 80).
                let reason = format!(
                    "`{kind}` is not an exit; the exits are {} and the offers are {}",
                    EXITS.join(", "),
                    OFFERS.join(", ")
                );
                fields.insert("exit".into(), json!("invalid"));
                fields.insert("raw".into(), json!(kind));
                fields.insert("refused".into(), json!(reason));
                if let Some(t) = text {
                    fields.insert("text".into(), json!(t));
                }
                Reported::Refused {
                    entry: entry(at, "exit", by, fields),
                    reason,
                }
            }
        }
        Report::Offer { kind, document } => {
            if OFFERS.contains(&kind.as_str()) {
                fields.insert("offer".into(), json!(kind));
                fields.insert("document".into(), json!(document));
                Reported::Accepted(entry(at, "offer", by, fields))
            } else {
                let reason = format!("`{kind}` is not an offer; the offers are {}", OFFERS.join(", "));
                fields.insert("raw".into(), json!(kind));
                fields.insert("refused".into(), json!(reason));
                Reported::Refused {
                    entry: entry(at, "offer", by, fields),
                    reason,
                }
            }
        }
        Report::Note { text } => {
            fields.insert("text".into(), json!(text));
            Reported::Accepted(entry(at, "note", by, fields))
        }
        Report::Refuse { reason } => {
            fields.insert("reason".into(), json!(reason));
            Reported::Accepted(entry(at, "refusal", by, fields))
        }
    };
    // One report, one entry, whatever it was (67).
    store.append(session, reported.entry())?;
    Ok(reported)
}
