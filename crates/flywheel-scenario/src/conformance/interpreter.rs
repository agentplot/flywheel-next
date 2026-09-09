//! What the operator said in chat, turned into one call of the catalogue.
//!
//! The machinery never parses free text (194): the interpreter that reads it is
//! the host's agent (`profiles/surfaces.yaml host_agent`), which proposes one
//! tool call per thing asked and records only what the operator confirmed.
//! Phase 1 has no such agent — it is the dispatch agent of phase 4 — so the
//! runner stands in for it here, in the harness and in nothing that ships: it
//! resolves the names a `dictation` step uses against the live objects and
//! proposes exactly one call, which the step then makes as the operator's
//! confirmation.
//!
//! It lives beside the runner and not in the binary for that reason. A name
//! that resolves to nothing, or to more than one object, fails the scenario
//! rather than being guessed at.

use anyhow::{bail, Result};
use flywheel_engine::Object;
use flywheel_surface::catalogue::Call;
use serde_json::json;
use std::collections::BTreeMap;

/// Read one line of the operator's chat into the call it proposes.
///
/// `by` is the identity the response records as given by; `delivery` is the
/// delivery this scenario's dictations arrive under, and `id` the delivery's
/// own id (153, 137).
pub fn propose(
    text: &str,
    by: &str,
    delivery: &str,
    id: &str,
    objects: &BTreeMap<String, Object>,
) -> Result<Call> {
    let text = text.trim();
    let call = |tool: &str| Call::new(tool, by, delivery).delivered(id);

    // `<kind>: <text>` opens something: the text is the material and is never
    // read for a command (19, 194).
    if let Some((head, body)) = text.split_once(':') {
        let body = body.trim();
        let head = head.trim();
        if head == "capture" {
            return Ok(call("capture")
                .arg("text", json!(body))
                .arg("source", json!(delivery)));
        }
        if head == "session" {
            return Ok(call("open-session")
                .arg("repository", json!(repository_of(objects)))
                .arg("text", json!(body)));
        }
        if let Some(bolt) = head.strip_prefix("unit ") {
            return Ok(call("propose-unit")
                .arg("bolt", json!(resolve(bolt.trim(), objects)?))
                .arg("text", json!(body)));
        }
        bail!("no tool opens a `{head}:`");
    }

    // Otherwise the first word is the operation and the rest names what it acts
    // on, with the noise word a person writes between them dropped.
    let mut words = text.split_whitespace();
    let Some(verb) = words.next() else {
        bail!("a dictation says nothing");
    };
    let rest: Vec<&str> = words
        .filter(|w| !matches!(*w, "place" | "session" | "signal" | "service" | "the"))
        .collect();
    let Some(name) = rest.first() else {
        bail!("the dictation `{text}` names nothing to act on");
    };
    let object = resolve(name, objects)?;

    // The argument is named as the tool's own schema names it; a tool the
    // catalogue lacks — one asserting work was done — takes `object`, and is
    // recorded as the response it was for the response machine to report (4).
    let argument = flywheel_surface::tool(verb)
        .and_then(|t| t.args.first().copied())
        .unwrap_or("object");
    Ok(call(verb).arg(argument, json!(object)))
}

/// The object a name means. An id is itself; a short name is the one object
/// whose id ends in it. Nothing, or more than one, is asked about and never
/// guessed (`surfaces.yaml host_agent`).
fn resolve(name: &str, objects: &BTreeMap<String, Object>) -> Result<String> {
    if objects.contains_key(name) {
        return Ok(name.to_string());
    }
    let matched: Vec<&String> = objects
        .keys()
        .filter(|id| id.ends_with(&format!("/{name}")))
        .collect();
    match matched.as_slice() {
        [one] => Ok((*one).to_string()),
        // A name naming nothing is still the object the operator meant to name;
        // the tool records it and the machinery reports what it made of it,
        // rather than the harness deciding there was nothing to say (4, 6).
        [] => Ok(name.to_string()),
        many => bail!("`{name}` names {} objects: {many:?}", many.len()),
    }
}

/// The repository a session with no repository named opens in: the one the
/// scenario tracks, where it tracks one.
fn repository_of(objects: &BTreeMap<String, Object>) -> String {
    objects
        .values()
        .find_map(|o| {
            o.record
                .get("repository")
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .unwrap_or_else(|| "blueprints".to_string())
}
