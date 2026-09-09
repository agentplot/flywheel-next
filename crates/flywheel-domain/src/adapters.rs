//! The adapters: the enumerator half of 215, which runs unattended (115).
//!
//! An adapter writes one keyed capture per source event with its provenance and
//! a pointer to the raw material, and does nothing else: it makes no judgment
//! about what the material says, starts no session, and reads nothing into
//! signals (111, 115, 215). Turning a capture into signals is curation's, and
//! never runs unattended — except where the capture is its own excerpt, which
//! the capture machine's own `ensure_signal` covers (19, 112).
//!
//! This phase ships three (D13): the page's capture box and the chat forward,
//! which are the `capture` tool itself, and the meeting transcript here. The
//! pull-request and issue-tracker adapters read the git host's issues and
//! reviews, which C.2 forbids on this profile; the capture endpoint is
//! dispatch's and is phase 4 (215, 216).

use crate::signals::{self, Capture};
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use flywheel_atoms::{StateStore, World};
use flywheel_engine::Definitions;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// What one enumeration did. An adapter reports rather than decides: the
/// counts are what the run record carries and what a repeat import shows as
/// zero (111, 79).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Enumerated {
    /// The source-event keys this run saw.
    pub keys: Vec<String>,
    /// How many capture records it wrote. A repeat writes none (111).
    pub captures_written: usize,
    /// How many signal records it wrote. An enumerator writes none: reading a
    /// capture into signals is a judgment (115).
    pub signals_written: usize,
}

/// The source name a meeting transcript is captured under.
pub const MEETING: &str = "meeting";

/// `flywheel capture meeting <file>`: one transcript, one capture (111, 215).
///
/// The key is `meeting/<date>/<name>`, read from the file's own name, so the
/// same transcript imported on two days yields one capture (111, S22). The
/// transcript itself stays where it is — the capture holds a pointer to it and
/// no repository holds a line of it (111).
pub fn meeting<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    path: &str,
    by: &str,
    at: DateTime<Utc>,
) -> Result<Enumerated> {
    let key = meeting_key(path)?;
    let capture = Capture {
        key: key.clone(),
        source: MEETING.into(),
        event_at: at.to_rfc3339(),
        captured_by: by.to_string(),
        // The pointer, and never the transcript: raw material stays outside
        // version control and the capture cites it (111).
        raw: path.to_string(),
    };
    let wrote = signals::write_capture(world, &capture)?;
    // The object the engine ticks, beside the record a person reads. A repeat
    // finds both and writes neither (111, 127).
    let id = signals::object_of(&key);
    if store.get(&id)?.is_none() {
        let record: BTreeMap<String, Value> = [
            ("source", json!(MEETING)),
            ("event_key", json!(key)),
            ("event_at", json!(at.to_rfc3339())),
            ("captured_by", json!(by)),
            ("raw", json!(path)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        crate::commands::put_new(store, defs, &id, "capture", None, record, at)?;
    }
    Ok(Enumerated {
        keys: vec![key],
        captures_written: usize::from(wrote),
        // The enumerator makes no judgment, so it reads nothing into signals: a
        // capture-reader session does, and only when the operator's curation
        // charges one (115).
        signals_written: 0,
    })
}

/// The source-event key a transcript's own name gives it:
/// `meeting/<date>/<name>` from `<date>-<name>.<ext>` (111,
/// `blueprints.yaml` adapters.meeting).
pub fn meeting_key(path: &str) -> Result<String> {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
    // `2026-09-02-willdan-weekly` — the date is the first three parts and the
    // name is the rest.
    let parts: Vec<&str> = stem.splitn(4, '-').collect();
    let [year, month, day, name] = parts.as_slice() else {
        bail!(
            "`{path}` does not name a meeting; a transcript is named \
             `<date>-<name>.<ext>`, as `2026-09-02-willdan-weekly.vtt` is (111)"
        );
    };
    if year.len() != 4 || month.len() != 2 || day.len() != 2 {
        bail!("`{path}` does not begin with a date; a transcript is named `<date>-<name>.<ext>`");
    }
    Ok(format!("{MEETING}/{year}-{month}-{day}/{name}"))
}

/// Run one adapter by the command a person or a scenario names it with, so the
/// binary and the conformance runner drive one implementation (93, D15).
pub fn run<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    command: &str,
    by: &str,
    at: DateTime<Utc>,
) -> Result<Enumerated> {
    let words: Vec<&str> = command.split_whitespace().collect();
    match words.as_slice() {
        [.., "meeting", path] => meeting(store, world, defs, path, by, at),
        _ => bail!(
            "`{command}` names no adapter this release ships; the meeting transcript is \
             `flywheel capture meeting <file>` (215, D13)"
        ),
    }
}
