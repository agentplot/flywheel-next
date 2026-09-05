//! Deriving the plan: every active state carrying `decision:` is a decision.

use crate::defs::Definitions;
use crate::runtime::{DecisionInstance, Object, Register};
use crate::tick::state_def;
use std::collections::BTreeMap;

/// The decision id: `<object>/<kind>/<entered_at>`. A re-entered decision state is a new decision.
pub fn decision_id(obj: &Object, region: &str, kind: &str) -> String {
    let since = obj.entered_at.get(region).map(|t| t.to_rfc3339()).unwrap_or_default();
    format!("{}/{}/{}", obj.id, kind, since)
}

/// Derive every standing decision, numbering new ones in the register.
pub fn derive(defs: &Definitions, objects: &BTreeMap<String, Object>, register: &mut Register) -> Vec<DecisionInstance> {
    let mut out = Vec::new();
    let mut objs: Vec<&Object> = objects.values().collect();
    objs.sort_by_key(|o| o.created);
    for obj in objs {
        for (region, state) in &obj.config {
            let Some((_reg, st)) = state_def(defs, obj, region) else { continue };
            let Some(d) = &st.decision else { continue };
            let id = decision_id(obj, region, &d.kind);
            let number = register.number_for(&id);
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
                number: Some(number),
            });
        }
    }
    out.sort_by_key(|d| d.number);
    out
}
