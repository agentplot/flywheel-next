//! The prefix rule: three repositories, three owners (203).
//!
//! The state repository is the machinery's alone. In a tracked repository —
//! the blueprints and every built repository — the machinery writes only under
//! its own prefix, `flywheel/`. The one exception is the change directory an
//! intent opens, and that is the effect of a response: a person asked for it
//! (23, 49, 203). Anything else is refused and reported rather than written.

use anyhow::{bail, Result};

/// What the machinery may write in a tracked repository without being asked.
pub const PREFIX: &str = "flywheel/";

/// The state repository, which is the machinery's own and has no prefix rule.
pub const STATE: &str = "flywheel-state";

/// Whether a path is inside the machinery's prefix.
pub fn inside(path: &str) -> bool {
    path.starts_with(PREFIX)
}

/// Refuse a write outside the prefix that no response asked for, and say which
/// path and which rule (203). A write the effect of a response makes is
/// allowed: the operator asked for it.
pub fn check(repository: &str, path: &str, by_response: Option<&str>) -> Result<()> {
    if repository == STATE || inside(path) {
        return Ok(());
    }
    if let Some(response) = by_response {
        return match response.trim().is_empty() {
            true => bail!(
                "{repository}: `{path}` is outside `{PREFIX}` and names an empty response (203)"
            ),
            false => Ok(()),
        };
    }
    bail!(
        "{repository}: `{path}` is outside the machinery's prefix `{PREFIX}` and no response asked \
         for it; the machinery writes under its prefix alone (203)"
    )
}
