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

/// The order ids count in: a run of digits against a run of digits is compared
/// as the number it is, anything else as text, so `x-2` comes before `x-10`.
///
/// No object records when it was made and none needs to: a batch's objects,
/// the rows of its fold and the order the engine takes among objects follow
/// their ids as they count (model.md §5.1, S232).
pub fn id_order(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (mut left, mut right) = (a, b);
    loop {
        match (left.is_empty(), right.is_empty()) {
            (true, true) => return a.cmp(b),
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            (false, false) => {}
        }
        let digits = |s: &str| s.starts_with(|c: char| c.is_ascii_digit());
        let run = |s: &str, numeric: bool| {
            s.find(|c: char| c.is_ascii_digit() != numeric).unwrap_or(s.len())
        };
        let numeric = digits(left) && digits(right);
        let (l, r) = match numeric {
            true => (run(left, true), run(right, true)),
            // Text runs to the next digit on either side, so `x-2` and `x-10`
            // part at the same place and their numbers meet.
            false => (run(left, false).max(1), run(right, false).max(1)),
        };
        let (lh, rh) = (&left[..l], &right[..r]);
        let order = match numeric {
            true => {
                let (ln, rn) = (lh.trim_start_matches('0'), rh.trim_start_matches('0'));
                ln.len().cmp(&rn.len()).then_with(|| ln.cmp(rn))
            }
            false => lh.cmp(rh),
        };
        if order != Ordering::Equal {
            return order;
        }
        left = &left[l..];
        right = &right[r..];
    }
}

/// The id of a decision on one object: `<object>/<kind>/<entered_at>`. A
/// re-entered decision state is a new decision.
pub fn decision_id(obj: &Object, region: &str, kind: &str) -> String {
    let since = obj.entered_at.get(region).map(|t| t.to_rfc3339()).unwrap_or_default();
    format!("{}/{}/{}", obj.id, kind, since)
}

/// A fold's id: `<kind>/<batch>/<since>`. It is the fold's and never its first
/// object's, so the fold keeps its number whichever of its objects leaves
/// first (15, model.md §5.1).
pub fn fold_id(kind: &str, batch: &str, since: &chrono::DateTime<chrono::Utc>) -> String {
    format!("{kind}/{batch}/{}", since.to_rfc3339())
}

/// Whether a decision id is a fold of this kind and batch, raised at any point.
pub fn is_fold_of(id: &str, kind: &str, batch: &str) -> bool {
    id.strip_prefix(kind)
        .and_then(|rest| rest.strip_prefix('/'))
        .and_then(|rest| rest.strip_prefix(batch))
        .and_then(|rest| rest.strip_prefix('/'))
        .is_some_and(|since| !since.is_empty() && !since.contains('/'))
}

/// A decision id read as a fold's: its kind and its batch.
///
/// Read as one object's, the same id names something else, so a reader holding
/// the objects says which reading names what is on record (`object_parts`).
pub fn fold_parts(id: &str) -> Option<(&str, &str)> {
    let (kind, rest) = id.split_once('/')?;
    let (batch, since) = rest.rsplit_once('/')?;
    (!kind.is_empty() && !batch.is_empty() && !since.is_empty()).then_some((kind, batch))
}

/// A decision id read as one object's: the object and its kind.
pub fn object_parts(id: &str) -> Option<(&str, &str)> {
    let (rest, since) = id.rsplit_once('/')?;
    let (object, kind) = rest.rsplit_once('/')?;
    (!object.is_empty() && !kind.is_empty() && !since.is_empty()).then_some((object, kind))
}

/// The fold of a kind and batch the register holds without `retracted_at`,
/// with the point it was raised at: on every tick after the one that numbered
/// it, that entry is the fold (model.md §5.1, §5.2).
fn standing_fold(
    register: &Register,
    kind: &str,
    batch: &str,
) -> Option<(String, chrono::DateTime<chrono::Utc>)> {
    let (id, entry) = register
        .entries
        .iter()
        .filter(|(id, entry)| entry.retracted_at.is_none() && is_fold_of(id, kind, batch))
        .min_by_key(|(_, entry)| entry.number)?;
    let since = entry.since.or_else(|| {
        let (_, at) = id.rsplit_once('/')?;
        chrono::DateTime::parse_from_rfc3339(at).ok().map(|t| t.with_timezone(&chrono::Utc))
    })?;
    Some((id.clone(), since))
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
    // Beside each decision, the batch it folds by where it has one, and the
    // earliest point any object folded into it entered its state.
    let mut batches: Vec<Option<((String, String), chrono::DateTime<chrono::Utc>)>> = Vec::new();
    let mut objs: Vec<&Object> = objects.values().collect();
    objs.sort_by(|a, b| id_order(&a.id, &b.id));
    for obj in objs {
        for (region, state) in &obj.config {
            // A decision stands on a state the object is in. One it has left
            // stays readable, but there is nothing to decide about it any more
            // (model.md §1, 9).
            if !crate::tick::is_live(obj, region) { continue; }
            let Some((_reg, st)) = state_def(defs, obj, region) else { continue };
            let Some(d) = &st.decision else { continue };
            let entered = obj.entered_at.get(region).cloned().unwrap_or_else(chrono::Utc::now);
            // Decisions whose `batch` field is equal are one decision: the
            // first of them by id stands and names the rest, so "yes" to it is
            // yes to all of them and the rail carries one number, not six (11).
            let batch = batch_of(defs, obj, region);
            if let Some(batch) = &batch {
                let folded = out
                    .iter()
                    .zip(&batches)
                    .position(|(x, held)| x.kind == d.kind && held.as_ref().is_some_and(|(b, _)| b == batch));
                if let Some(at) = folded {
                    out[at].folds.push(obj.id.clone());
                    if let Some((_, earliest)) = batches[at].as_mut() {
                        *earliest = (*earliest).min(entered);
                    }
                    continue;
                }
            }
            out.push(DecisionInstance {
                id: decision_id(obj, region, &d.kind),
                object: obj.id.clone(),
                region: region.clone(),
                state: state.clone(),
                kind: d.kind.clone(),
                group: d.group.clone(),
                answers: d.answers.clone(),
                shows: d.shows.clone(),
                document: d.document.as_ref().and_then(|f| obj.record.get(f).cloned()),
                since: entered,
                number: None,
                folds: vec![obj.id.clone()],
            });
            batches.push(batch.map(|batch| (batch, entered)));
        }
    }
    // A fold is named for its kind and batch and the point it was raised: the
    // entry the register holds for it while any of its objects stands, and
    // otherwise the earliest point one of them entered, so a fold emptied and
    // refilled is a new decision with a new number (15, model.md §5.1, §5.2).
    for (decision, held) in out.iter_mut().zip(&batches) {
        if let Some(((_, batch), earliest)) = held {
            let (id, since) = standing_fold(register, &decision.kind, batch)
                .unwrap_or_else(|| (fold_id(&decision.kind, batch, earliest), *earliest));
            decision.id = id;
            decision.since = since;
        }
        decision.number = register.number_of(&decision.id);
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

/// The groups, in the order every reader walks them: approve, decide, answer,
/// then attention (model.md §5.1, S3).
///
/// `derive` returns the decisions in number order, because the number is what a
/// response names and what the register gives. The grouping is the reader's,
/// and it is not decoration: 11 asks that the decisions be grouped so that
/// "yes to all" is a meaningful answer, and a list that walks approve, decide,
/// approve, decide, attention offers no group for a yes-to-all to mean.
pub const GROUPS: [&str; 4] = ["approve", "decide", "answer", "attention"];

/// Whether a group counts toward the number of decisions standing.
///
/// Attention is shown on the rail and stands outside the count: it is what the
/// machinery could not do, reported and never dropped, rather than a choice the
/// operator is being asked to make (S3, S8, 6).
pub fn counted(group: &str) -> bool {
    group != "attention"
}

/// The decisions as a reader shows them: the groups in the model's order, each
/// sorted by the number the register gave, and any group the model does not
/// name after them, in the order they derived.
pub fn in_reading_order(decisions: &[DecisionInstance]) -> Vec<&DecisionInstance> {
    let mut out: Vec<&DecisionInstance> = Vec::with_capacity(decisions.len());
    for group in GROUPS {
        let mut mine: Vec<&DecisionInstance> =
            decisions.iter().filter(|d| d.group == group).collect();
        mine.sort_by_key(|d| d.number.unwrap_or(u32::MAX));
        out.extend(mine);
    }
    out.extend(decisions.iter().filter(|d| !GROUPS.contains(&d.group.as_str())));
    out
}
