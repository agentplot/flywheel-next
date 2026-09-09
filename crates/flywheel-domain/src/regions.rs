//! Which place and which session a region path refers to.
//!
//! An object's regions name the work under it: a stage's sessions, a bolt's own
//! place. Both the stand-in and a real host resolve a region path the same way,
//! so the rule is stated once here.

/// The stage or type a nested region path belongs to: the stem a session's id
/// is built on, `<owner id>/<stage or type>` (`session.yaml` id).
pub fn session_stem(object: &str, region: &str, kind: Option<&str>) -> String {
    let parts: Vec<&str> = region.split('.').collect();
    if let Some(i) = parts.iter().position(|p| *p == "stages") {
        if let Some(stage) = parts.get(i + 1) {
            return format!("{object}/{stage}");
        }
    }
    if let Some(kind) = kind.filter(|k| !k.is_empty()) {
        return format!("{object}/{kind}");
    }
    if parts.iter().any(|p| *p == "working") {
        return format!("{object}/work");
    }
    format!("{object}/main")
}

/// A session's id: deterministic, `<owner id>/<stage or type>/<attempt>`, and
/// also the pane's and the agent's name (`session.yaml` id, 196). The attempt
/// is what makes a fresh session after a takeover or a lost pane a session of
/// its own rather than the same one twice (150, S13).
pub fn session_id(stem: &str, attempt: u32) -> String {
    format!("{stem}/{attempt}")
}

/// The attempt an id ends in, where it carries one.
pub fn attempt_of(session: &str) -> Option<u32> {
    session.rsplit('/').next().and_then(|n| n.parse().ok())
}

/// The stem of a session id: everything before the attempt.
pub fn stem_of(session: &str) -> &str {
    match session.rsplit_once('/') {
        Some((stem, attempt)) if attempt.parse::<u32>().is_ok() => stem,
        _ => session,
    }
}

/// The stem and the first attempt, for a caller with no object to hand.
pub fn session_key(object: &str, region: &str) -> String {
    session_id(&session_stem(object, region, None), 1)
}

/// The place a region path refers to: a bolt's own place is `<id>#own`; every
/// other object has one.
pub fn place_key(object: &str, region: &str) -> String {
    if region.starts_with("place") {
        format!("{object}#own")
    } else {
        object.to_string()
    }
}
