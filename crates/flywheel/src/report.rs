//! What a session reports, and the one path it reports through.
//!
//! The report itself is `flywheel-domain`'s, because it is a record of the
//! domain and because the page's curator surface writes the same one the
//! command does (67, 93b, D16). `flywheel exit | offer | note | refuse` is this
//! module's caller and nothing here is a second path. `flywheel ask` is the
//! one command here that calls a tool of the catalogue as the session, which
//! is the only path a session has to a tool (67, 197).

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Utc};
use flywheel_atoms::{Records, StateStore, World};
use flywheel_engine::Definitions;
use flywheel_surface::catalogue::{self, Call};
use serde_json::json;
use std::path::Path;

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

/// `flywheel offer finding|chore|signal --document <path> [--about <object>] [--scope bolt-line|<repository>]`:
/// one entry on the session's thread (`sessions.yaml` commands.offer). Any
/// other kind is refused there, as a report outside the exits is (65, 66).
///
/// The command runs in the session's place, and the entry names the place's
/// head, which must hold the document: the document is read at that revision
/// wherever it is read, so one not committed there is refused on the thread
/// naming the path and is never pending (62).
///
/// A chore says where its fix belongs. One offered off every bolt that names
/// no repository the instance tracks, and not the blueprints, is refused on the
/// thread with the names it could have given and is never pending. The tracked
/// names are read from the manifest only when the scope needs them, as the ask
/// reads them (60, 67).
#[allow(clippy::too_many_arguments)]
pub fn offer<S: Records>(
    store: &mut S,
    tracked: impl FnOnce() -> Result<Vec<String>>,
    place: &Path,
    session: &str,
    by: &str,
    at: DateTime<Utc>,
    kind: &str,
    document: &str,
    scope: Option<&str>,
    about: Option<&str>,
) -> Result<Reported> {
    // A report that is refused whatever the place holds reads nothing of it.
    let judged = !session.is_empty() && OFFERS.contains(&kind);
    let head = match judged {
        true => flywheel_world_host::git::head_holding(place, document)?,
        false => None,
    };
    let report = Report::Offer {
        kind: kind.to_string(),
        document: document.to_string(),
        scope: scope.map(String::from),
        about: about.map(String::from),
        revision: head.as_ref().map(|h| h.revision.clone()),
    };
    if !judged {
        return write_report(store, session, by, at, &report);
    }
    let uncommitted = match &head {
        Some(head) if head.holds => None,
        Some(head) => Some(format!(
            "`{document}` is not committed at this place's head {}; commit it here and offer again (62)",
            &head.revision[..head.revision.len().min(12)]
        )),
        None => Some(format!(
            "`{document}` is offered from {}, which holds no commit; an offer points at a document \
             committed in the session's place (62)",
            place.display()
        )),
    };
    if let Some(reason) = uncommitted {
        return flywheel_domain::report::refuse_offer(store, session, by, at, &report, None, &reason);
    }
    if kind == "chore" {
        let owner = flywheel_domain::offers::owner_of(&*store, session)?;
        if let Some(reason) = flywheel_domain::offers::chore_refused(&*store, owner.as_deref(), scope, tracked)? {
            return flywheel_domain::report::refuse_offer(store, session, by, at, &report, None, &reason);
        }
    }
    write_report(store, session, by, at, &report)
}

/// The code a report exits with: nought when it is recorded, one when it is
/// refused and the refusal recorded (66, 80).
pub fn exit_code(reported: &Reported) -> i32 {
    match reported {
        Reported::Accepted(_) => 0,
        Reported::Refused { .. } => 1,
    }
}
