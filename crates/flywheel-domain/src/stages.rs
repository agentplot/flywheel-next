//! A stage's evidence, read from the records a real host holds (56, 41,
//! `record-derived.yaml` stage.*, `host.yaml` stage.agents).
//!
//! A work item runs its unit type's stages, and each stage runs a session
//! set; what the stage asks — who works it, whether the set has all reported,
//! what the reports add up to — is answered here from the type's definition
//! and the sessions' threads, so the operator runner and a pane runner read
//! the same thing. Until this existed only the scenario harness answered
//! these names, and on a real host no work item ever left `resolving`.

use crate::regions;
use flywheel_atoms::{Records, ThreadEntry};
use flywheel_engine::defs::State;
use flywheel_engine::{Definitions, Object};
use serde_json::{json, Value};

/// The stage a region path is under: the segment after `stages`
/// (`life.in-type.stages.fix.run` → `fix`).
pub fn stage_of(region: &str) -> Option<&str> {
    let parts: Vec<&str> = region.split('.').collect();
    let i = parts.iter().position(|p| *p == "stages")?;
    parts.get(i + 1).copied()
}

/// The unit type an object runs: its own `type` field, else its unit's, since
/// a work item's type is the unit's at the version the unit recorded (57).
pub fn type_of<S: Records>(store: &S, object: &Object) -> Option<String> {
    if let Some(kind) = object.record.get("type").and_then(|v| v.as_str()) {
        return Some(kind.to_string());
    }
    let parent = object.parent.as_deref()?;
    let unit = store.get(parent).ok().flatten()?;
    unit.record.get("type").and_then(|v| v.as_str()).map(String::from)
}

/// The stage's state in the type's `stages` region, which carries the
/// stage's params: agents, join, on_fail, max_send_backs, deliverables.
pub fn stage_state<'a>(defs: &'a Definitions, kind: &str, stage: &str) -> Option<&'a State> {
    defs.get(kind)?.regions.get("stages")?.states.get(stage)
}

fn param<'a>(state: &'a State, name: &str) -> Option<&'a Value> {
    state.params.as_ref()?.get(name)
}

/// Whether the type needs a change directory, from its record; a type that
/// says nothing needs one (`unit-types/default@5.yaml`).
pub fn needs_change_directory(defs: &Definitions, kind: &str) -> bool {
    defs.get(kind)
        .and_then(|m| m.record.get("needs_change_directory"))
        .and_then(|v| v.as_str())
        .map(|v| v != "false")
        .unwrap_or(true)
}

/// The newest exit entry on a session's thread that still stands: an answer
/// delivered after it clears it, because a blocked session goes back to
/// working when the operator answers, and the exit it stopped on is spent
/// (68, 70, `sessions.yaml` session.exit).
pub fn exit_entry<S: Records>(store: &S, session: &str) -> Option<ThreadEntry> {
    let thread = store.thread(session).ok()?;
    let last = |kind: &str| thread.iter().rposition(|e| e.kind == kind);
    let exit = last("exit")?;
    if last("answer").is_some_and(|answered| answered > exit) {
        return None;
    }
    thread.get(exit).cloned()
}

/// The kind of that exit — done, blocked, stalled or invalid — where one stands.
pub fn exit_of<S: Records>(store: &S, session: &str) -> Option<String> {
    exit_entry(store, session)?
        .fields
        .get("exit")
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// The stage and item atoms, for the object a region path is under.
///
/// One session per stage attempt in this release: the session id is
/// `<item>/<stage>/<attempt>` (`session.yaml` id) and carries no agent, so a
/// stage naming several agents is read over that one session. The shipped
/// types name one agent per stage.
pub fn evidence<S: Records>(
    store: &S,
    defs: &Definitions,
    object: &str,
    region: &str,
    name: &str,
) -> Option<Value> {
    let asked = name.starts_with("stage.")
        || matches!(name, "item.send_backs" | "item.retry_max" | "item.change_archived");
    if !asked {
        return None;
    }
    let held = store.get(object).ok().flatten()?;
    match name {
        // How many times the item has been sent back, a counter the stage's
        // `sent-back` bumps (41).
        "item.send_backs" => Some(json!(held.counters.get("send_backs").copied().unwrap_or(0))),
        // `host.yaml` item.change_archived: true without reading when the
        // type's record says it needs no change directory. A type that does
        // reads false here: the archive is a directory in the item's place,
        // and the workspace binding that has a place to read is what answers
        // it — until then the act is refused and the item says so.
        "item.change_archived" => {
            let kind = type_of(store, &held)?;
            Some(json!(!needs_change_directory(defs, &kind)))
        }
        _ => {
            let kind = type_of(store, &held)?;
            let stage = stage_of(region)?;
            let state = stage_state(defs, &kind, stage)?;
            match name {
                // A fixed list from the type (`host.yaml` stage.agents); the
                // globbed persona rule is the persona-test type's and waits
                // on a place to glob in.
                "stage.agents" => Some(param(state, "agents").cloned().unwrap_or_else(|| json!([]))),
                "item.retry_max" => Some(json!(param(state, "max_send_backs")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(3))),
                "stage.join_met" | "stage.verdict" => {
                    let stem = regions::session_stem(object, region, Some(&kind));
                    let session = regions::session_id(&stem, regions::attempt_of_object(&held));
                    // A session that has not reported is nothing on the
                    // record, and the record says nothing of it: the guards
                    // read `is: true` and `is: pass`, so an unanswered read
                    // holds the stage exactly as `false` would, and a world a
                    // scenario described can still say what the session did
                    // (B.3, D8, 125).
                    let entry = exit_entry(store, &session)?;
                    let entry = Some(entry);
                    let exit = entry
                        .as_ref()
                        .and_then(|e| e.fields.get("exit"))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    // The join is met when every counted session has exited;
                    // a blocked session stopped only itself and has not (70).
                    let exited = matches!(exit.as_deref(), Some("done" | "stalled" | "invalid"));
                    if name == "stage.join_met" {
                        return Some(json!(exited));
                    }
                    // `record-derived.yaml` stage.verdict: pass when every
                    // counted exit is done with its deliverables; not-done
                    // when a counted exit's deliverables include a not-done
                    // verdict; blocked while any session is blocked; stalled
                    // when a counted exit is stalled or invalid.
                    let not_done = entry
                        .as_ref()
                        .and_then(|e| e.fields.get("deliverables"))
                        .and_then(|v| v.as_array())
                        .is_some_and(|d| {
                            d.iter()
                                .filter_map(|v| v.as_str())
                                .any(|d| d == "verdict:not-done" || d == "not-done")
                        });
                    Some(json!(match exit.as_deref() {
                        Some("done") if not_done => "not-done",
                        Some("done") => "pass",
                        Some("blocked") => "blocked",
                        Some("stalled") | Some("invalid") => "stalled",
                        _ => "none",
                    }))
                }
                _ => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use flywheel_atoms::testing::FakeStore;
    use std::collections::BTreeMap;

    const REGION: &str = "life.in-type.stages.fix.run";

    fn object(id: &str, machine: &str, parent: Option<&str>, record: &[(&str, Value)]) -> Object {
        Object {
            id: id.into(),
            machine: machine.into(),
            parent: parent.map(String::from),
            config: Default::default(),
            entered_at: Default::default(),
            record: record.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
            counters: Default::default(),
            applied_responses: vec![],
            seq: 0,
            created: 0,
        }
    }

    /// A chore unit with one work item under it, as `create_items` makes them.
    fn a_chore_item() -> (FakeStore, Definitions) {
        let defs = crate::set::load().expect("the embedded definitions");
        let mut store = FakeStore::default();
        store.seed(object(
            "unit/atlas/tidy",
            "unit",
            Some("bolt/atlas/plan-rows"),
            &[("repository", json!("atlas")), ("type", json!("chore")), ("type_version", json!(2))],
        ));
        store.seed(object(
            "work-item/atlas/tidy/1",
            "work-item",
            Some("unit/atlas/tidy"),
            &[("repository", json!("atlas")), ("ordinal", json!(1))],
        ));
        (store, defs)
    }

    fn report(store: &mut FakeStore, session: &str, exit: &str, deliverables: &[&str]) {
        let mut fields = BTreeMap::new();
        fields.insert("exit".to_string(), json!(exit));
        if !deliverables.is_empty() {
            fields.insert("deliverables".to_string(), json!(deliverables));
        }
        store
            .append(
                session,
                &ThreadEntry {
                    at: Utc.with_ymd_and_hms(2026, 9, 14, 12, 0, 0).unwrap(),
                    kind: "exit".into(),
                    by: Some("chore-fixer".into()),
                    fields,
                },
            )
            .expect("the exit is on the thread");
    }

    fn read(store: &FakeStore, defs: &Definitions, name: &str) -> Value {
        evidence(store, defs, "work-item/atlas/tidy/1", REGION, name).unwrap_or_else(|| panic!("`{name}` is answered"))
    }

    #[test]
    fn the_stage_names_its_agents_from_the_type() {
        let (store, defs) = a_chore_item();
        assert_eq!(read(&store, &defs, "stage.agents"), json!(["chore-fixer"]));
        assert_eq!(read(&store, &defs, "item.retry_max"), json!(1));
        assert_eq!(read(&store, &defs, "item.send_backs"), json!(0));
    }

    #[test]
    fn nothing_reported_is_nothing_said_of_the_join_or_the_verdict() {
        let (store, defs) = a_chore_item();
        assert_eq!(evidence(&store, &defs, "work-item/atlas/tidy/1", REGION, "stage.join_met"), None);
        assert_eq!(evidence(&store, &defs, "work-item/atlas/tidy/1", REGION, "stage.verdict"), None);
    }

    #[test]
    fn a_done_exit_meets_the_join_and_passes() {
        let (mut store, defs) = a_chore_item();
        report(&mut store, "work-item/atlas/tidy/1/fix/1", "done", &["commits"]);
        assert_eq!(read(&store, &defs, "stage.join_met"), json!(true));
        assert_eq!(read(&store, &defs, "stage.verdict"), json!("pass"));
    }

    #[test]
    fn a_blocked_session_has_not_exited_and_reads_blocked() {
        let (mut store, defs) = a_chore_item();
        report(&mut store, "work-item/atlas/tidy/1/fix/1", "blocked", &[]);
        assert_eq!(read(&store, &defs, "stage.join_met"), json!(false));
        assert_eq!(read(&store, &defs, "stage.verdict"), json!("blocked"));
    }

    #[test]
    fn a_not_done_verdict_among_the_deliverables_sends_back() {
        let (mut store, defs) = a_chore_item();
        report(&mut store, "work-item/atlas/tidy/1/fix/1", "done", &["commits", "verdict:not-done"]);
        assert_eq!(read(&store, &defs, "stage.verdict"), json!("not-done"));
    }

    #[test]
    fn a_stalled_or_invalid_exit_reads_stalled() {
        let (mut store, defs) = a_chore_item();
        report(&mut store, "work-item/atlas/tidy/1/fix/1", "invalid", &[]);
        assert_eq!(read(&store, &defs, "stage.join_met"), json!(true));
        assert_eq!(read(&store, &defs, "stage.verdict"), json!("stalled"));
    }

    #[test]
    fn the_session_read_is_the_attempt_the_item_is_on() {
        let (mut store, defs) = a_chore_item();
        // The first attempt reported and was lost; the stage went round again.
        report(&mut store, "work-item/atlas/tidy/1/fix/1", "done", &["commits"]);
        let mut item = store.get("work-item/atlas/tidy/1").unwrap().unwrap();
        item.counters.insert("attempt".into(), 1);
        let seq = item.seq;
        store.put("work-item/atlas/tidy/1", &item, seq).unwrap();
        assert_eq!(evidence(&store, &defs, "work-item/atlas/tidy/1", REGION, "stage.join_met"), None, "attempt two has not reported");
    }

    #[test]
    fn a_type_with_no_change_directory_is_archived_at_once() {
        let (mut store, defs) = a_chore_item();
        assert_eq!(read(&store, &defs, "item.change_archived"), json!(true));
        let mut unit = store.get("unit/atlas/tidy").unwrap().unwrap();
        unit.record.insert("type".into(), json!("default"));
        let seq = unit.seq;
        store.put("unit/atlas/tidy", &unit, seq).unwrap();
        assert_eq!(read(&store, &defs, "item.change_archived"), json!(false));
    }

    #[test]
    fn a_name_that_is_not_the_stages_is_left_to_others() {
        let (store, defs) = a_chore_item();
        assert!(evidence(&store, &defs, "work-item/atlas/tidy/1", REGION, "place.exists").is_none());
    }
}
