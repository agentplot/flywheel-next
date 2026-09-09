//! The work order renderer: the closed set of inputs a session is handed, and
//! nothing else (88, 89).
//!
//! Four inputs and no fifth — the schema instruction, the type skill, the work
//! order proper (the job, the deliverables table, the exit contract) and the
//! artifacts of the change it works. The header names the blueprints commit,
//! the deliverables binding's version, the instruction set version and every
//! schema, instruction and skill it hands in as `name@version` (226), so two
//! sessions started either side of a chore on one instruction are told apart by
//! the header and not by the commit alone (123).
//!
//! `prepare_place` renders this before a session starts; `flywheel
//! render-order` renders the same text with no session started (90, 124).

use crate::context::{Binding, Row};
use crate::deliverables::{Ask, ByType, Deliverables, Resolved};
use crate::instructions::{InstructionFile, Instructions};
use anyhow::{bail, Context, Result};
use flywheel_engine::Definitions;
use serde_json::Value;

/// What the session is being asked to work: the object, the place and the
/// point every input was read at.
#[derive(Debug, Clone, Default)]
pub struct Job {
    pub session: String,
    pub place: String,
    /// The object the session works, and its record as the store holds it.
    pub object: String,
    pub fields: Vec<(String, String)>,
    /// The blueprints commit every input was read at (226).
    pub blueprints_commit: String,
}

/// One input the order hands in, at the version in force.
#[derive(Debug, Clone)]
pub struct Input {
    pub path: String,
    pub name: String,
    pub version: u32,
    pub body: String,
}

impl Input {
    pub fn named(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }

    fn of(file: &InstructionFile) -> Input {
        Input {
            path: file.path.clone(),
            name: file.name.clone(),
            version: file.version,
            body: file.body.clone(),
        }
    }
}

/// One deliverable as the order names it, each part at its version.
#[derive(Debug, Clone)]
pub struct Deliverable {
    pub name: String,
    pub producer: String,
    pub schema: String,
    pub surface: String,
}

/// The rendered order, section by section, so what is in it can be asserted
/// rather than read out of one string.
#[derive(Debug, Clone)]
pub struct Order {
    pub session_type: String,
    pub job: Job,
    pub deliverables_version: u32,
    pub instruction_set: u32,
    /// The schema of each deliverable the type names, and the default
    /// instructions the set says it carries (120, 190).
    pub schema_instruction: Vec<Input>,
    /// The agent's definition and its skill, keyed by the agent name the
    /// machine or the stage names (context.yaml ruling 3).
    pub type_skill: Vec<Input>,
    /// The job in the row's own words, and the deliverables table.
    pub work_order: String,
    pub deliverables: Vec<Deliverable>,
    pub exit_contract: String,
    /// The artifacts of the change the session works.
    pub change_artifacts: String,
}

/// The four sections of 89, in the order they are written. What the order
/// carries is exactly these; a fifth is a fifth input, which 89 forbids.
pub const SECTIONS: &[&str] = &[
    "schema instruction",
    "type skill",
    "work order",
    "the change's artifacts",
];

impl Order {
    /// Every input the order hands in, as the header names them (226).
    pub fn inputs(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .schema_instruction
            .iter()
            .chain(self.type_skill.iter())
            .map(|i| i.named())
            .collect();
        for d in &self.deliverables {
            out.push(d.producer.clone());
            out.push(d.schema.clone());
        }
        out.sort();
        out.dedup();
        out
    }

    /// The order as the session reads it.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# work order · {}\n\n", self.session_type));
        out.push_str(&format!("session: {}\n", self.job.session));
        out.push_str(&format!("place: {}\n", self.job.place));
        out.push_str(&format!(
            "blueprints commit: {}\n",
            self.job.blueprints_commit
        ));
        out.push_str(&format!(
            "deliverables binding: {}\n",
            self.deliverables_version
        ));
        out.push_str(&format!("instruction set: {}\n", self.instruction_set));
        out.push_str(&format!("inputs: {}\n", self.inputs().join(" · ")));

        out.push_str("\n## schema instruction\n");
        for input in &self.schema_instruction {
            out.push_str(&format!("\n### {} · {}\n\n", input.path, input.named()));
            out.push_str(input.body.trim());
            out.push('\n');
        }

        out.push_str("\n## type skill\n");
        for input in &self.type_skill {
            out.push_str(&format!("\n### {} · {}\n\n", input.path, input.named()));
            out.push_str(input.body.trim());
            out.push('\n');
        }

        out.push_str("\n## work order\n\n");
        out.push_str(&format!("job: {}\n", self.job.object));
        for (name, value) in &self.job.fields {
            out.push_str(&format!("{name}: {value}\n"));
        }
        out.push_str("\ndeliverables:\n");
        for d in &self.deliverables {
            out.push_str(&format!(
                "  {} · producer {} · schema {} · surface {}\n",
                d.name, d.producer, d.schema, d.surface
            ));
        }
        if !self.work_order.is_empty() {
            out.push_str(&format!("\n{}\n", self.work_order));
        }
        out.push_str(&format!("\nexit contract: {}\n", self.exit_contract));

        out.push_str("\n## the change's artifacts\n\n");
        out.push_str(&format!("{}\n", self.change_artifacts));
        out
    }
}

/// Render the order one session type would be handed for one job.
pub fn render(
    session_type: &str,
    job: &Job,
    defs: &Definitions,
    binding: &Binding,
    deliverables: &Deliverables,
    instructions: &Instructions,
) -> Result<Order> {
    let (type_name, stage) = match session_type.split_once('/') {
        Some((t, s)) => (t, Some(s)),
        None => (session_type, None),
    };
    let row = binding.row(type_name)?;
    let (asks, by_type, tier) = asks_of(type_name, stage, &row, defs, deliverables)?;

    let mut resolved: Vec<Resolved> = Vec::new();
    for ask in &asks {
        resolved.push(deliverables.resolve(ask, &by_type)?);
    }

    // The schema instruction: the schema of each deliverable, and the default
    // instructions the set says this type carries (120, 190).
    let mut schema_instruction: Vec<Input> = Vec::new();
    for one in &resolved {
        let file = instructions.require(&one.schema)?;
        if !schema_instruction.iter().any(|i| i.path == file.path) {
            schema_instruction.push(Input::of(file));
        }
    }
    for name in carried(type_name, tier, instructions) {
        let file = instructions.require(&format!("flywheel/instructions/{name}.md"))?;
        if !schema_instruction.iter().any(|i| i.path == file.path) {
            schema_instruction.push(Input::of(file));
        }
    }

    // The type skill: the agent's own definition and its skill (ruling 3).
    let agent = by_type
        .agent
        .clone()
        .with_context(|| format!("no agent is named for `{session_type}`"))?;
    let mut type_skill = Vec::new();
    if let Some(file) = instructions.at(&format!("flywheel/agents/{agent}.md")) {
        type_skill.push(Input::of(file));
    }
    type_skill.push(Input::of(
        instructions.require(&format!("flywheel/skills/{agent}/SKILL.md"))?,
    ));

    let named = |path: &str| -> Result<String> { Ok(instructions.require(path)?.named()) };
    let mut table = Vec::new();
    for one in &resolved {
        table.push(Deliverable {
            name: one.name.clone(),
            producer: named(&one.producer)?,
            schema: named(&one.schema)?,
            surface: match one.surface.is_empty() {
                true => "none".into(),
                false => one.surface.clone(),
            },
        });
    }

    Ok(Order {
        session_type: session_type.to_string(),
        job: job.clone(),
        deliverables_version: deliverables.version,
        instruction_set: instructions.version,
        schema_instruction,
        type_skill,
        work_order: row.work_order.clone().unwrap_or_default(),
        deliverables: table,
        exit_contract: binding.common.exit_contract.clone(),
        change_artifacts: row.change_artifacts.clone().unwrap_or_default(),
    })
}

/// Which tier of type a session type belongs to, which is how the set decides
/// the default instructions it carries (`instructions/set.yaml` carries:).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// A type file whose one session produces the deliverables.
    Elaboration,
    /// A type file whose stages do.
    Unit,
    /// A session the machinery charges with no type file of its own.
    Machinery,
}

/// What the type asks for, what a `by-type` entry resolves against, and which
/// tier it is.
///
/// A type file states its deliverables in the parameters of the session or the
/// stage that produces them, which is where they are read from; a session type
/// the machinery charges without a type file — curation, planning, capture
/// reading, the operator's own — names them in its row (226).
fn asks_of(
    type_name: &str,
    stage: Option<&str>,
    row: &Row,
    defs: &Definitions,
    deliverables: &Deliverables,
) -> Result<(Vec<Ask>, ByType, Tier)> {
    if let Some(machine) = defs.machines.get(type_name) {
        // A unit type's deliverables are a stage's, and a stage is named; an
        // elaboration type's are its one session's.
        let staged = machine.regions.contains_key("stages");
        if staged && stage.is_none() {
            bail!(
                "`{type_name}` is a unit type: name the stage its session runs, \
                 as `{type_name}/<stage>`"
            );
        }
        for region in machine.regions.values() {
            for (name, state) in &region.states {
                let is_wanted = match stage {
                    Some(stage) => name == stage,
                    None => state.machine.as_deref() == Some("session"),
                };
                if !is_wanted {
                    continue;
                }
                let params = state.params.clone().unwrap_or_default();
                let asks = asks_from(params.get("deliverables"))?;
                // A stage names the agents it starts, and the first is the one
                // whose skill the order hands in; an elaboration type's
                // `agent: by-type` names the type itself (ruling 3), and
                // `$agent` is the parameter the object fills.
                let agent = params
                    .get("agents")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .or_else(|| match params.get("agent").and_then(|v| v.as_str()) {
                        Some(named) if !named.starts_with('$') && named != "by-type" => {
                            Some(named.to_string())
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| type_name.to_string());
                return Ok((
                    asks,
                    ByType {
                        unit_type: staged.then(|| type_name.to_string()),
                        agent: Some(agent),
                    },
                    match staged {
                        true => Tier::Unit,
                        false => Tier::Elaboration,
                    },
                ));
            }
        }
        if let Some(stage) = stage {
            bail!("the type `{type_name}` has no stage named `{stage}`");
        }
    }

    // No type file: the row names the deliverables, and the type skill names
    // the agent whose skill it is.
    let asks = row
        .deliverables
        .as_deref()
        .map(named_in)
        .unwrap_or_default()
        .into_iter()
        .filter(|name| deliverables.shipped.contains_key(name))
        .map(|name| Ask {
            name,
            ..Default::default()
        })
        .collect();
    let agent = row.type_skill.as_deref().and_then(agent_in);
    Ok((
        asks,
        ByType {
            unit_type: None,
            agent,
        },
        Tier::Machinery,
    ))
}

/// The deliverable entries of a type file's parameters.
fn asks_from(value: Option<&Value>) -> Result<Vec<Ask>> {
    let Some(value) = value else { return Ok(vec![]) };
    let asks: Vec<Ask> = serde_json::from_value(value.clone())
        .context("reading a type file's `deliverables:` parameter")?;
    Ok(asks)
}

/// The deliverable names a row states, which it writes as a comma-separated
/// list before the parenthesis that cites the type file. `none` names none.
fn named_in(text: &str) -> Vec<String> {
    let head = text.split('(').next().unwrap_or(text);
    head.split(';')
        .next()
        .unwrap_or(head)
        .split(',')
        .map(|part| part.trim().trim_matches('`').to_string())
        .filter(|part| !part.is_empty() && part != "none")
        .collect()
}

/// The agent a row's type skill names: the one whose `flywheel/skills/<agent>/`
/// the row points at.
fn agent_in(text: &str) -> Option<String> {
    let at = text.find("flywheel/skills/")? + "flywheel/skills/".len();
    let rest = &text[at..];
    let name = rest.split('/').next()?;
    (!name.is_empty()).then(|| name.to_string())
}

/// The default instructions this type carries (120), from `set.yaml`
/// `carries:`. A type named under `none:` carries none, and the absence is a
/// decision.
fn carried(type_name: &str, tier: Tier, instructions: &Instructions) -> Vec<String> {
    let carries = &instructions.carries;
    if carries.none.sessions.iter().any(|s| s == type_name) {
        return vec![];
    }
    if let Some(named) = carries.by_name.get(type_name) {
        return named.clone();
    }
    match tier {
        Tier::Elaboration => carries.elaboration_types.clone(),
        Tier::Unit => carries.unit_types.clone(),
        Tier::Machinery => vec![],
    }
}
