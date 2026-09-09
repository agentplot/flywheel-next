//! Signals, and the one effect that writes a capture's own (113, 19, 112).
//!
//! A signal is immutable: it carries its kind, its asserter, its subject, its
//! assertion and the verbatim excerpt it came from, and nothing rewrites one
//! (113). `ensure_signal` is the capture machine's effect for a capture that is
//! its own excerpt — the page's box and a forwarded single message — where no
//! judgment is involved and the signal is the message (19, 112,
//! `capture.yaml` reading.captured).
//!
//! Turning any other capture into signals is curation's, and never runs
//! unattended (115). Nothing here does that.

use anyhow::Result;
use chrono::{DateTime, Utc};
use flywheel_atoms::{Scope, StateStore};
use flywheel_engine::Definitions;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Every signal object's id begins here.
pub const PREFIX: &str = "signal/";

/// The signals a capture yielded, in the order `list` returns them. A signal
/// cites the capture it came from by being owned by it (`capture.yaml` owns).
pub fn of_capture<S: StateStore>(store: &S, capture: &str) -> Result<Vec<String>> {
    Ok(store
        .list_records(&Scope::All)?
        .into_iter()
        .filter(|o| o.machine == "signal" && o.parent.as_deref() == Some(capture))
        .map(|o| o.id)
        .collect())
}

/// `ensure_signal`: one capture, one signal, and never a second (19, 112,
/// `atoms.yaml` ensure_signal).
///
/// The signal's kind is `ask`, which is what a message with no judgment behind
/// it asserts; its excerpt is the capture's own raw material, verbatim (113).
/// The effect's proof is `capture.signals_present`, so running it again with
/// the signal already there changes nothing (127).
pub fn ensure_signal<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    capture: &str,
    by: &str,
    at: DateTime<Utc>,
) -> Result<Option<String>> {
    if !of_capture(store, capture)?.is_empty() {
        return Ok(None);
    }
    let held = store.get(capture)?;
    let field = |name: &str| {
        held.as_ref()
            .and_then(|o| o.record.get(name))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let key = held
        .as_ref()
        .and_then(|o| o.record.get("event_key"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| capture.trim_start_matches("capture/").to_string());
    let excerpt = field("raw");
    let asserter = match field("captured_by").is_empty() {
        true => by.to_string(),
        false => field("captured_by"),
    };
    let id = format!("{PREFIX}{key}");
    let record: BTreeMap<String, Value> = [
        // A message with no judgment behind it asks; the kind an operator's
        // response gives it replaces this and nothing else does (113, 116).
        ("kind", json!("ask")),
        ("asserted_by", json!(asserter)),
        ("subject", json!(field("source"))),
        ("assertion", json!(excerpt)),
        ("excerpt", json!(excerpt)),
        ("captured_at", json!(at.to_rfc3339())),
        ("subject_tags", json!([])),
        ("argues_with", json!([])),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    crate::commands::put_new(store, defs, &id, "signal", Some(capture), record, at)?;
    Ok(Some(id))
}
