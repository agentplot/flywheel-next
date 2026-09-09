//! The deliverables binding (190): the shipped set of deliverables and, for
//! each, the skill that produces it, the schema it must satisfy and the surface
//! that reviews it.
//!
//! Versioned as one thing, and a work order's header names that version beside
//! the blueprints commit, so a session started before a change and one after
//! are told apart (123). The engine reads the resolved names and paths and
//! never the text (119).

use crate::profile::Profiles;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;

/// One shipped deliverable.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Entry {
    #[serde(default)]
    pub producer: String,
    #[serde(default)]
    pub schema: String,
    #[serde(default)]
    pub surface: String,
    #[serde(default)]
    pub store: String,
    #[serde(default)]
    pub feeds: Vec<String>,
}

/// The binding, as the file is written.
#[derive(Debug, Clone, Deserialize)]
pub struct Deliverables {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub shipped: BTreeMap<String, Entry>,
}

/// What a type or a stage asks for: an entry naming the deliverable and, for
/// each of the producer, the schema and the surface, either a path, `default`
/// or `by-type`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Ask {
    pub name: String,
    #[serde(default)]
    pub producer: Option<String>,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub surface: Option<String>,
    /// The OpenSpec step that produces it, where a stage names one (190).
    #[serde(default)]
    pub step: Option<String>,
}

/// One deliverable as the work order names it: the paths in force.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub name: String,
    pub producer: String,
    pub schema: String,
    pub surface: String,
}

/// What a `by-type` entry resolves against: the unit type and the agent whose
/// skill the stage names (190, context.yaml ruling 3).
#[derive(Debug, Clone, Default)]
pub struct ByType {
    pub unit_type: Option<String>,
    pub agent: Option<String>,
}

impl Deliverables {
    pub fn load(profiles: &impl Profiles) -> Result<Deliverables> {
        let text = profiles.read("deliverables")?;
        serde_yaml::from_str(&text).context("parsing profiles/deliverables.yaml")
    }

    /// Resolve one ask. An explicit path is taken as written; `default` reads
    /// the shipped entry; `by-type` reads the stage's type skill and the type's
    /// schema instruction for the step. A name the shipped set does not know,
    /// asked with `default`, is refused rather than resolved by convention
    /// (deliverables.yaml resolution, 227).
    pub fn resolve(&self, ask: &Ask, by_type: &ByType) -> Result<Resolved> {
        let entry = self.shipped.get(&ask.name);
        let one = |given: &Option<String>, shipped: fn(&Entry) -> String, what: &str| -> Result<String> {
            match given.as_deref() {
                None | Some("default") => {
                    let entry = entry.with_context(|| {
                        format!(
                            "no deliverable named `{}` is shipped, and `{what}: default` \
                             names one (227)",
                            ask.name
                        )
                    })?;
                    Ok(shipped(entry))
                }
                Some("by-type") => by_type_path(what, ask, by_type),
                Some(path) => Ok(path.to_string()),
            }
        };
        Ok(Resolved {
            name: ask.name.clone(),
            producer: one(&ask.producer, |e| e.producer.clone(), "producer")?,
            schema: one(&ask.schema, |e| e.schema.clone(), "schema")?,
            surface: match ask.surface.as_deref() {
                None | Some("default") => entry.map(|e| e.surface.clone()).unwrap_or_default(),
                Some(other) => other.to_string(),
            },
        })
    }
}

/// `by-type`: the type's schema instruction for the step, and the agent's own
/// skill as the producer (`instructions/set.yaml` resolution).
fn by_type_path(what: &str, ask: &Ask, by_type: &ByType) -> Result<String> {
    let unit_type = by_type
        .unit_type
        .as_deref()
        .with_context(|| format!("`{}` asks for `{what}: by-type` outside a unit type", ask.name))?;
    match what {
        "schema" => {
            let step = ask.step.as_deref().with_context(|| {
                format!("`{}` asks for `schema: by-type` and names no step (190)", ask.name)
            })?;
            Ok(format!("flywheel/schemas/units/{unit_type}/{step}.md"))
        }
        _ => {
            let agent = by_type.agent.as_deref().with_context(|| {
                format!("`{}` asks for `producer: by-type` and no agent is named", ask.name)
            })?;
            Ok(format!("flywheel/skills/{agent}/SKILL.md"))
        }
    }
}
