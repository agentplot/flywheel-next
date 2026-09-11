//! Deriving the rail: every active state carrying `decision:` is a decision.
//!
//! The rail is a pure function of the active states of every object it is
//! derived over, plus the register that numbers them (model.md §5.1). Nothing
//! here writes: a decision with no entry comes back unnumbered, and the rail
//! machine numbers it this tick. The tail beside it is derived the same way,
//! from the states carrying `tail:` that were entered since a sink's mark, so
//! one mark per sink is the only recorded state behind it (14, 15).

use crate::defs::Definitions;
use crate::runtime::{DecisionInstance, Object, Register, TailEntry};
use crate::tick::state_def;
use std::collections::BTreeMap;

/// The decision id: `<object>/<kind>/<entered_at>`. A re-entered decision state is a new decision.
pub fn decision_id(obj: &Object, region: &str, kind: &str) -> String {
    let since = obj.entered_at.get(region).map(|t| t.to_rfc3339()).unwrap_or_default();
    format!("{}/{}/{}", obj.id, kind, since)
}

/// The record field a decision folds by and the value this object has for it,
/// where its state names one. Equal values fold into one numbered decision
/// (11, `schema.json` decision.batch).
pub fn batch_of(defs: &Definitions, obj: &Object, region: &str) -> Option<(String, String)> {
    let (_reg, st) = state_def(defs, obj, region)?;
    let decision = st.decision.as_ref()?;
    let field = decision.batch.as_ref()?;
    let value = obj.record.get(field)?.as_str()?.to_string();
    if value.is_empty() || value == "none" {
        return None;
    }
    Some((field.clone(), value))
}

/// Whether two objects' decisions of one kind are the same decision: they fold
/// by the same field to the same value (11).
pub fn folds_together(defs: &Definitions, a: &Object, b: &Object, kind: &str) -> bool {
    if a.id == b.id {
        return true;
    }
    let of = |obj: &Object| -> Option<(String, String)> {
        obj.config
            .keys()
            .filter(|region| crate::tick::is_live(obj, region))
            .filter(|region| {
                state_def(defs, obj, region)
                    .and_then(|(_, st)| st.decision.as_ref().map(|d| d.kind == kind))
                    .unwrap_or(false)
            })
            .find_map(|region| batch_of(defs, obj, region))
    };
    match (of(a), of(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// Derive every standing decision and attach the number the register gave it.
///
/// Pure: a decision with no entry is unnumbered, and the rail machine numbers
/// it this tick (model.md §5.1). Nothing here writes, so deriving the rail on
/// a reader — the page, the status view — gives the numbers the register holds
/// and never issues one.
pub fn derive(defs: &Definitions, objects: &BTreeMap<String, Object>, register: &Register) -> Vec<DecisionInstance> {
    let mut out: Vec<DecisionInstance> = Vec::new();
    let mut objs: Vec<&Object> = objects.values().collect();
    objs.sort_by_key(|o| o.created);
    for obj in objs {
        for (region, state) in &obj.config {
            // A decision stands on a state the object is in. One it has left
            // stays readable, but there is nothing to decide about it any more
            // (model.md §1, 9).
            if !crate::tick::is_live(obj, region) { continue; }
            let Some((_reg, st)) = state_def(defs, obj, region) else { continue };
            let Some(d) = &st.decision else { continue };
            // Decisions whose `batch` field is equal are one decision: the
            // first of them stands and names the rest, so "yes" to it is yes
            // to all of them and the rail carries one number, not six (11).
            if let Some(batch) = batch_of(defs, obj, region) {
                if let Some(standing) = out.iter_mut().find(|x| {
                    x.kind == d.kind
                        && objects
                            .get(&x.object)
                            .map(|first| batch_of(defs, first, &x.region).as_ref() == Some(&batch))
                            .unwrap_or(false)
                }) {
                    standing.folds.push(obj.id.clone());
                    continue;
                }
            }
            let id = decision_id(obj, region, &d.kind);
            let number = register.number_of(&id);
            out.push(DecisionInstance {
                id,
                object: obj.id.clone(),
                region: region.clone(),
                state: state.clone(),
                kind: d.kind.clone(),
                group: d.group.clone(),
                answers: d.answers.clone(),
                shows: d.shows.clone(),
                document: d.document.as_ref().and_then(|f| obj.record.get(f).cloned()),
                since: obj.entered_at.get(region).cloned().unwrap_or_else(chrono::Utc::now),
                number,
                folds: vec![obj.id.clone()],
            });
        }
    }
    // The number the register gave, and a decision raised this tick and not
    // yet numbered after the numbered ones, in the order it was derived —
    // which is the order it will be numbered in. The group order §5.1 names is
    // the reader's: the rail, the page and the chat each walk approve, decide,
    // answer, attention over this list.
    out.sort_by_key(|d| d.number.unwrap_or(u32::MAX));
    out
}

/// The tail: every state carrying `tail:` that was entered since a point, in
/// the order it was entered. Derived like the decisions, from the states
/// themselves, so one mark per sink is the only recorded state behind it and
/// no rendering is stored (14, 15).
pub fn tail(
    defs: &Definitions,
    objects: &BTreeMap<String, Object>,
    since: chrono::DateTime<chrono::Utc>,
) -> Vec<TailEntry> {
    let mut out = Vec::new();
    for obj in objects.values() {
        for (region, state) in &obj.config {
            if !crate::tick::is_live(obj, region) {
                continue;
            }
            let Some((_reg, st)) = state_def(defs, obj, region) else { continue };
            let Some(word) = &st.tail else { continue };
            let Some(at) = obj.entered_at.get(region) else { continue };
            if *at <= since {
                continue;
            }
            out.push(TailEntry {
                at: *at,
                object: obj.id.clone(),
                kind: word.clone(),
                state: state.clone(),
                by: obj.applied_responses.last().cloned(),
            });
        }
    }
    out.sort_by(|a, b| a.at.cmp(&b.at).then(a.object.cmp(&b.object)));
    out
}
