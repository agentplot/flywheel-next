//! Which place and which session a region path refers to.
//!
//! An object's regions name the work under it: a stage's sessions, a bolt's own
//! place. Both the stand-in and a real host resolve a region path the same way,
//! so the rule is stated once here.

/// The stage or type a nested region path belongs to, for session and stage
/// facts.
pub fn session_key(object: &str, region: &str) -> String {
    let parts: Vec<&str> = region.split('.').collect();
    if let Some(i) = parts.iter().position(|p| *p == "stages") {
        if let Some(stage) = parts.get(i + 1) {
            return format!("{object}/{stage}");
        }
    }
    if parts.iter().any(|p| *p == "working") {
        return format!("{object}/work");
    }
    format!("{object}/main")
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
