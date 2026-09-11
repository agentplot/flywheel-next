//! The change directory behind an intent, on the blueprints' shared line:
//! OpenSpec's `openspec/changes/<name>/`, archived to
//! `openspec/changes/archive/<name>/` when the intent closes (A.11, 227).

use crate::signals::Reads;
use serde_json::{json, Value};

/// Where a change lives while open, and once archived.
pub const CHANGES: &str = "openspec/changes/";
pub const ARCHIVE: &str = "openspec/changes/archive/";

/// The change's name for an intent: the id without its kind (`intent/x` → `x`).
pub fn name_of(intent: &str) -> &str {
    intent.strip_prefix("intent/").unwrap_or(intent)
}

/// Whether the intent's change directory has been archived: a directory of
/// its name under the archive holds files, and none stands open
/// (`atoms.yaml` intent.archived).
pub fn archived<R: Reads + ?Sized>(files: &R, intent: &str) -> bool {
    let name = name_of(intent);
    let under = |prefix: &str| -> bool {
        let dir = format!("{prefix}{name}/");
        files.list(&dir).into_iter().any(|p| p.starts_with(&dir))
    };
    under(ARCHIVE) && !under(CHANGES)
}

/// The evidence the change directory answers (`intent.archived`).
pub fn evidence<R: Reads + ?Sized>(files: &R, object: &str, name: &str) -> Option<Value> {
    match name {
        "intent.archived" => Some(json!(archived(files, object))),
        // The proof of `open_intent`: the change directory stands open.
        "intent.change_open" => {
            let dir = format!("{CHANGES}{}/", name_of(object));
            Some(json!(files.list(&dir).into_iter().any(|p| p.starts_with(&dir))))
        }
        _ => None,
    }
}
