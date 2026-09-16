//! The rail as an object (148, `engine/rail.yaml`).
//!
//! Its record is the register and the status projection's as-of point; it is
//! not a second store. The object itself is made from that record at the top of
//! every tick, the way a lease object is, so the machine ticks over it while
//! the record stays the one place the register lives.

use chrono::{DateTime, Utc};
use flywheel_atoms::{Object, Records};
use flywheel_engine::Definitions;
use std::collections::BTreeMap;

pub use crate::RAIL;

/// The rail object, initialised where its record carries no state yet.
pub fn attach<S: Records>(
    store: &S,
    defs: &Definitions,
    objects: &mut BTreeMap<String, Object>,
    now: DateTime<Utc>,
) -> anyhow::Result<()> {
    let Some(mut rail) = store.get(RAIL)? else {
        return Ok(());
    };
    if rail.config.is_empty() {
        flywheel_engine::initialise(defs, &mut rail, now);
    }
    objects.insert(RAIL.to_string(), rail);
    Ok(())
}

/// Every standing decision, as a reader must see it: the engine's derivation,
/// with a child folded onto its parent's card where the model says it rides
/// there rather than standing on its own.
///
/// The engine derives a decision from every live state that carries one, which
/// is right and is not the whole rule: requirement 10's first decision kind is
/// "a proposed intent, **with its proposed elaborations**". Deriving is pure
/// and knows no object names, so the fold belongs here, where the domain's do.
/// Every reader of the rail goes through this — the page, the chat, the status
/// projection, the numbering — so a decision that rides on another's card is
/// never numbered, never counted, and never asked of the operator twice.
pub fn standing(
    defs: &Definitions,
    objects: &BTreeMap<String, Object>,
    register: &flywheel_engine::runtime::Register,
) -> Vec<flywheel_engine::DecisionInstance> {
    let mut derived = flywheel_engine::rail::derive(defs, objects, register);
    fold_onto_the_parents_card(objects, &mut derived);
    offer_only_what_applies(objects, &mut derived);
    derived
}

/// A decision offers the answers that apply to the object it stands on, and not
/// every answer its machine names: an elaboration covering one intent has
/// nothing to pick and no intent to take out of a gathering, so it offers
/// neither `pick <intents>` nor the per-intent drop (27, 188, S226,
/// `elaboration.yaml` proposed — "offers only the answers that apply — pick and
/// the per-intent drop only when covers names more than one intent").
///
/// The narrowing is the machinery's and not a rendering's. The page drew the
/// right controls already, but the standing decision carried the machine's
/// whole list, and the chat's controls and the numbered reply grammar read that
/// list too — so an answer the operator could never see on a card was one they
/// could still send, and a reader checking the card against the decision found
/// a control missing that was never the card's to draw.
fn offer_only_what_applies(
    objects: &BTreeMap<String, Object>,
    derived: &mut [flywheel_engine::DecisionInstance],
) {
    for decision in derived.iter_mut() {
        if decision.kind != "elaboration-proposed" || covers(objects, &decision.object) > 1 {
            continue;
        }
        decision
            .answers
            .retain(|answer| !answer.starts_with("pick ") && !per_intent_drop(answer));
    }
}

/// `<intent>: drop`: the answer that takes one intent out of a gathering (188).
fn per_intent_drop(answer: &str) -> bool {
    answer.starts_with('<') && answer.ends_with(": drop")
}

/// How many intents an elaboration covers: the gathering's own list, or the one
/// intent it hangs under where it names none (188).
fn covers(objects: &BTreeMap<String, Object>, id: &str) -> usize {
    let Some(held) = objects.get(id) else {
        return 1;
    };
    held.record
        .get("covers")
        .and_then(|v| v.as_array())
        .map(|covered| covered.len())
        .filter(|covered| *covered > 0)
        .unwrap_or(1)
}

/// An elaboration proposed against an intent that is itself still proposed is
/// part of that intent's decision (10, `intent.yaml` proposed, `elaboration.yaml`
/// proposed, `atoms.yaml` elaboration.shown_with_parent).
///
/// The machines and the evidence both said so already — the elaboration's own
/// `proposed` state carries the transition whose note reads "no decision of its
/// own; the intent's response carries it" — and the rail numbered it anyway. A
/// curation that proposes an intent and the elaboration to understand it put
/// two cards on the rail where the operator has one thing to decide, and the
/// second was not theirs to answer: until the intent is a subject at all,
/// there is nothing to say about how to understand it.
///
/// The response path is unaffected. A number fans out over a fold only among
/// objects whose own state carries a decision of the *same* kind
/// (`eval.rs`, 11), and these are two kinds, so a yes to the intent still does
/// not answer the elaboration. Once the intent opens, `shown_with_parent` goes
/// false, the elaboration's card stands on its own and is numbered then — which
/// is the moment the operator has something to decide.
fn fold_onto_the_parents_card(
    objects: &BTreeMap<String, Object>,
    derived: &mut Vec<flywheel_engine::DecisionInstance>,
) {
    let rides_along = |id: &str| -> Option<String> {
        let held = objects.get(id)?;
        if held.machine != "elaboration" || held.top_state()? != "proposed" {
            return None;
        }
        let parent = objects.get(held.parent.as_deref()?)?;
        (parent.machine == "intent" && parent.top_state()? == "proposed")
            .then(|| parent.id.clone())
    };
    let carried: Vec<(String, String)> = derived
        .iter()
        .filter_map(|d| rides_along(&d.object).map(|parent| (d.object.clone(), parent)))
        .collect();
    for (child, parent) in carried {
        derived.retain(|d| d.object != child);
        // Named on the card it rides on, so the operator reads the intent and
        // the elaboration proposed with it as the one thing they are.
        if let Some(card) = derived.iter_mut().find(|d| d.object == parent) {
            if !card.folds.iter().any(|held| *held == child) {
                card.folds.push(child);
            }
        }
    }
}
