//! The ask: a dictation naming a repository and the words, the operator's or
//! curation's when it routes a signal that argues with no claim (28, 116).
//!
//! It is a record in the state store and not an object — no machine ticks it
//! — written by the catalogue's `ask` tool through the store's commit path,
//! and read by planning, which sets `consumed_by` when a unit takes it up
//! (model.md 1, git-only `layout.asks`). What points at one names it
//! `ask/<id>`: a signal's route move, and the response the call was recorded
//! as.

use anyhow::Result;
use flywheel_atoms::{Ask, Records};

/// How anything that points at an ask names it.
pub const PREFIX: &str = "ask/";

/// The name an ask is pointed at by: `ask/<id>`.
pub fn name_of(id: &str) -> String {
    format!("{PREFIX}{id}")
}

/// The id a name points at, where it names an ask at all.
pub fn id_named(name: &str) -> Option<&str> {
    name.strip_prefix(PREFIX).filter(|id| !id.is_empty())
}

/// The id the next ask naming this repository takes: `<repository>-<n>`, one
/// past the highest the store holds for it. The records are the count, so no
/// counter one store knows about is kept and a number is never reused (15).
pub fn next_id<S: Records + ?Sized>(store: &S, repository: &str) -> Result<String> {
    let stem = format!("{repository}-");
    let highest = store
        .asks()?
        .iter()
        .filter_map(|ask| ask.id.strip_prefix(&stem).and_then(|n| n.parse::<u64>().ok()))
        .max()
        .unwrap_or(0);
    Ok(format!("{stem}{}", highest + 1))
}

/// The ask a name points at, where one was written.
pub fn named<S: Records + ?Sized>(store: &S, name: &str) -> Result<Option<Ask>> {
    let Some(id) = id_named(name) else {
        return Ok(None);
    };
    Ok(store.asks()?.into_iter().find(|ask| ask.id == id))
}

/// Whether a session may file an ask: the curation session and the operator's
/// own session, and no other (69, 197, `sessions.yaml` commands.ask).
///
/// A session's id is `<owner id>/<type>/<attempt>` and an object's id begins
/// with its machine, so the first segment says whose session it is.
pub fn granted(session: &str) -> bool {
    let id = session.strip_prefix("session/").unwrap_or(session);
    matches!(id.split('/').next(), Some("curation" | "operator-session"))
}

/// The identity a session's own command is recorded as given by:
/// `session/<session id>` (197, `sessions.yaml` commands).
pub fn by_session(session: &str) -> String {
    match session.starts_with("session/") {
        true => session.to_string(),
        false => format!("session/{session}"),
    }
}
