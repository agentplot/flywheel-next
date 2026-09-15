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

/// Where the standing claims are: one specification per capability.
pub const SPECS: &str = "openspec/specs/";

/// One standing claim, as a curation session's work order names it and a
/// challenge names it back: `<capability>/<requirement>` in one word, as a
/// signal's list of claims holds it, with its title and the file it stands in
/// (97, 113, 116; context.yaml sessions.curation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub name: String,
    pub title: String,
    pub path: String,
}

/// Every standing claim on the blueprints' shared line: each requirement of
/// each capability's `spec.md` under `openspec/specs/` (97, 98).
pub fn standing_claims<R: Reads + ?Sized>(files: &R) -> Vec<Claim> {
    let mut paths: Vec<String> = files.list(SPECS).into_iter().filter(|p| p.ends_with("/spec.md")).collect();
    paths.sort();
    let mut out = Vec::new();
    for path in paths {
        let capability = path.trim_start_matches(SPECS).trim_end_matches("/spec.md").to_string();
        let Some(text) = files.read(&path) else {
            continue;
        };
        for line in text.lines() {
            if let Some(requirement) = line.strip_prefix("### Requirement:") {
                let title = requirement.trim().to_string();
                let slug: String = title
                    .to_lowercase()
                    .split(|c: char| !c.is_ascii_alphanumeric())
                    .filter(|word| !word.is_empty())
                    .collect::<Vec<_>>()
                    .join("-");
                out.push(Claim {
                    name: format!("{capability}/{slug}"),
                    title,
                    path: path.clone(),
                });
            }
        }
    }
    out
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
