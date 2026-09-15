//! What a session reports, and the one path it reports through.
//!
//! The report itself is `flywheel-domain`'s, because it is a record of the
//! domain and because the page's curator surface writes the same one the
//! command does (67, 93b, D16). `flywheel exit | offer | note | refuse` is this
//! module's caller and nothing here is a second path. `flywheel ask` is the
//! one command here that calls a tool of the catalogue as the session, which
//! is the only path a session has to a tool (67, 197).

use anyhow::{anyhow, bail, Result};
use flywheel_atoms::{StateStore, World};
use flywheel_engine::Definitions;
use flywheel_surface::catalogue::{self, Call};
use serde_json::json;

pub use flywheel_domain::report::{write_report, Report, Reported, EXITS, OFFERS, SESSION_ENV};

/// `flywheel ask <repository> <text>`: the catalogue's `ask` called with the
/// session's identity (`sessions.yaml` commands.ask).
///
/// The record written is the one the operator's dictation writes, `by` the
/// session, and nothing goes on the session's thread. What comes back is the
/// ask's name, `ask/<id>`, which the command prints and the session's route
/// move names (116). A refusal — a session not granted the ask, a repository
/// the instance does not track — is an entry on the session's own thread and
/// comes back as the error (43, 197).
pub fn ask<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    session: &str,
    repository: &str,
    text: &str,
) -> Result<String> {
    if session.is_empty() {
        bail!("no session: pass --session or set {SESSION_ENV}, which the work order names");
    }
    let call = Call::new("ask", session, "session")
        .arg("repository", json!(repository))
        .arg("text", json!(text));
    let outcome = catalogue::call(store, world, defs, &call)?;
    catalogue::asked(&outcome).ok_or_else(|| anyhow!("the ask was recorded and named nothing"))
}
