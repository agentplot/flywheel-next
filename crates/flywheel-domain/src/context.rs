//! The context binding (226): for every session type the machinery may charge,
//! one enumeration of what its work order carries and what must never reach it.
//!
//! `prepare_place` renders the work order from these rows and from nothing else
//! (89). A session type without a row here is not one the machinery may charge.

use crate::profile::Profiles;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;

/// What every row inherits: the header, the exit contract, and what no session
/// ever receives.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Common {
    #[serde(default)]
    pub header: String,
    #[serde(default)]
    pub exit_contract: String,
    #[serde(default)]
    pub never: String,
    #[serde(default)]
    pub skills: String,
}

/// One session type's row.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Row {
    #[serde(default)]
    pub charged_by: String,
    /// The row this one is otherwise read as: `standing` is `self-closing`
    /// with three fields of its own.
    #[serde(default, rename = "as")]
    pub reads_as: Option<String>,
    #[serde(default)]
    pub schema_instruction: Option<String>,
    #[serde(default)]
    pub type_skill: Option<String>,
    #[serde(default)]
    pub work_order: Option<String>,
    #[serde(default)]
    pub change_artifacts: Option<String>,
    #[serde(default)]
    pub chapters_and_claims: Option<String>,
    #[serde(default)]
    pub map: Option<String>,
    #[serde(default)]
    pub surface_specification: Option<String>,
    #[serde(default)]
    pub deliverables: Option<String>,
    #[serde(default)]
    pub identity: Option<String>,
    /// What must never reach the session. It is a rule about the place and the
    /// tools, and it is not written into the work order: what a session is
    /// handed is the closed set of inputs and nothing else (89).
    #[serde(default)]
    pub never: Option<String>,
}

/// The binding, as the file is written.
#[derive(Debug, Clone, Deserialize)]
pub struct Binding {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub common: Common,
    #[serde(default)]
    pub sessions: BTreeMap<String, Row>,
}

impl Binding {
    /// Read `profiles/context.yaml` from the set the binary carries, or from a
    /// directory of definitions.
    pub fn load(profiles: &impl Profiles) -> Result<Binding> {
        let text = profiles.read("context")?;
        serde_yaml::from_str(&text).context("parsing profiles/context.yaml")
    }

    /// One session type's row with what it reads as folded in, field by field:
    /// a row naming `as:` states only what differs (`standing`, `fast`).
    pub fn row(&self, session: &str) -> Result<Row> {
        let held = self
            .sessions
            .get(session)
            .with_context(|| {
                format!(
                    "`{session}` is no session type the machinery may charge; \
                     profiles/context.yaml has no row for it (226)"
                )
            })?
            .clone();
        let Some(base) = &held.reads_as else {
            return Ok(held);
        };
        // `as:` may name the type a stage's row is read as rather than another
        // row — `explore-or-gathered` reads as "the type". Where no row of that
        // name is there, the row stands on its own.
        let Ok(under) = self.row(base) else {
            return Ok(held);
        };
        Ok(Row {
            charged_by: held.charged_by,
            reads_as: held.reads_as,
            schema_instruction: held.schema_instruction.or(under.schema_instruction),
            type_skill: held.type_skill.or(under.type_skill),
            work_order: held.work_order.or(under.work_order),
            change_artifacts: held.change_artifacts.or(under.change_artifacts),
            chapters_and_claims: held.chapters_and_claims.or(under.chapters_and_claims),
            map: held.map.or(under.map),
            surface_specification: held.surface_specification.or(under.surface_specification),
            deliverables: held.deliverables.or(under.deliverables),
            identity: held.identity.or(under.identity),
            never: held.never.or(under.never),
        })
    }
}
