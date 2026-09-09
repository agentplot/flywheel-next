//! Which place and which session a region path refers to.
//!
//! An object's regions name the work under it: a stage's sessions, a bolt's own
//! place. Both the stand-in and a real host resolve a region path the same way,
//! so the rule is stated once here.

/// The stage or type a nested region path belongs to: the stem a session's id
/// is built on, `<owner id>/<stage or type>` (`session.yaml` id).
pub fn session_stem(object: &str, region: &str, kind: Option<&str>) -> String {
    session_stem_with(object, region, kind, None)
}

/// The same, with the agent the session sub-machine at this path names.
///
/// A stage names the session first, then the object's own type; where it has
/// neither, the session sub-machine's `agent:` param does — which is what names
/// curation's `curator` and planning's own (`curation.yaml`
/// running.session.params, S08). The order matters: an object with a type keeps
/// the stem that type gives it.
pub fn session_stem_with(
    object: &str,
    region: &str,
    kind: Option<&str>,
    agent: Option<&str>,
) -> String {
    let parts: Vec<&str> = region.split('.').collect();
    if let Some(i) = parts.iter().position(|p| *p == "stages") {
        if let Some(stage) = parts.get(i + 1) {
            return format!("{object}/{stage}");
        }
    }
    if let Some(kind) = kind.filter(|k| !k.is_empty()) {
        return format!("{object}/{kind}");
    }
    if let Some(agent) = agent.filter(|a| !a.is_empty()) {
        return format!("{object}/{agent}");
    }
    if parts.iter().any(|p| *p == "working") {
        return format!("{object}/work");
    }
    format!("{object}/main")
}

/// The agent the session sub-machine under a region names, where one is there
/// (`session.yaml` params.agent).
///
/// A guard on the `run` region and an effect at
/// `run.running.session.session.life` are asking about the same session, so the
/// object's own configuration is what says where it is: every path at or under
/// the region is walked, and the deepest one that runs a session machine names
/// the agent.
pub fn agent_of(
    defs: &flywheel_engine::Definitions,
    object: &flywheel_engine::Object,
    region: &str,
) -> Option<String> {
    let under = format!("{region}.");
    let mut deepest: Option<(usize, String)> = None;
    for path in object.config.keys() {
        if path != region && !path.starts_with(&under) {
            continue;
        }
        let Some(agent) = agent_at(defs, &object.machine, path) else {
            continue;
        };
        let depth = path.split('.').count();
        if deepest.as_ref().is_none_or(|(held, _)| depth > *held) {
            deepest = Some((depth, agent));
        }
    }
    deepest.map(|(_, agent)| agent)
}

/// The agent named along one region path.
///
/// The path alternates region and state from the machine's top — `run` is a
/// region, `running` the state it is in, `session` a region of that state, and
/// so on — so it is walked as it is written, and the innermost state that runs
/// a session machine is the one whose agent names it.
fn agent_at(
    defs: &flywheel_engine::Definitions,
    machine: &str,
    region: &str,
) -> Option<String> {
    let machine = defs.for_object(machine).or_else(|| defs.get(machine))?;
    let mut regions = &machine.regions;
    let mut agent: Option<String> = None;
    let mut parts = region.split('.');
    loop {
        let Some(name) = parts.next() else { break };
        let Some(region) = regions.get(name) else { break };
        let Some(state_name) = parts.next() else { break };
        let Some(state) = region.states.get(state_name) else {
            break;
        };
        if state.machine.as_deref() == Some("session") {
            agent = state
                .params
                .as_ref()
                .and_then(|p| p.get("agent"))
                .and_then(|v| v.as_str())
                .map(String::from);
        }
        regions = &state.regions;
    }
    agent
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

/// The attempt an object is on: one, and one more for each time `bump:
/// attempt` has sent the stage round again. The field is the stage's
/// (`stage.yaml` record.attempt), kept on the object the stage runs under,
/// because one object carries one record; what the bump counts is the times
/// round, so the first attempt is the one no bump has happened for.
pub fn attempt_of_object(object: &flywheel_engine::Object) -> u32 {
    let times_round = object
        .record
        .get("attempt")
        .and_then(|v| v.as_u64())
        .or_else(|| object.counters.get("attempt").map(|n| (*n).max(0) as u64))
        .unwrap_or(0);
    times_round as u32 + 1
}

/// The session a region path refers to, at the attempt the object is on. A
/// session lost sends the stage round again with the attempt one higher, and
/// that is what makes the fresh session a session of its own rather than the
/// multiplexer being asked for a name it already holds (150, `stage.yaml`).
pub fn session_key_of(object: &flywheel_engine::Object, region: &str) -> String {
    session_key_in(None, object, region)
}

/// The same, with the definitions in hand, so a session the machine names an
/// agent for is named for that agent (`session.yaml` id, S08).
pub fn session_key_of_in(
    defs: &flywheel_engine::Definitions,
    object: &flywheel_engine::Object,
    region: &str,
) -> String {
    session_key_in(Some(defs), object, region)
}

fn session_key_in(
    defs: Option<&flywheel_engine::Definitions>,
    object: &flywheel_engine::Object,
    region: &str,
) -> String {
    // Where no stage names the session, the type does (`session.yaml` id,
    // `<owner id>/<stage or type>/<attempt>`): an elaboration's sessions sit
    // in its type's regions, and the operator's own session runs the type its
    // machine fixes and records. An object with no type of its own is named by
    // the agent its session sub-machine gives, and failing that by the region
    // the session sits in.
    let kind = object.record.get("type").and_then(|v| v.as_str());
    let agent = defs.and_then(|defs| agent_of(defs, object, region));
    session_id(
        &session_stem_with(&object.id, region, kind, agent.as_deref()),
        attempt_of_object(object),
    )
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
