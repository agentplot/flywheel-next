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

/// What a curation session's work order lists, read when it is rendered: the
/// unmoved signals, the standing claims, the open intents, the elaboration
/// types a proposal may name and the repositories a route may offer into
/// (model.md §9; context.yaml sessions.curation).
#[derive(Debug, Clone, Default)]
pub struct Curation {
    pub signals: Vec<crate::signals::Signal>,
    pub claims: Vec<crate::changes::Claim>,
    /// Each open intent: its id, its subject and how many signals it holds.
    pub intents: Vec<(String, String, usize)>,
    pub types: Vec<String>,
    pub repositories: Vec<String>,
}

/// The part of a curation session's work order that is its job: every unmoved
/// signal with what it asserts, the claims and intents a move may name, and the
/// two files the session delivers its judgments in, parsed when it exits (107,
/// 109, 116; `instructions/schemas/move.md`, `intent-proposal.md`).
pub fn curation(inputs: &Curation) -> String {
    let mut out = String::from("## the unmoved signals\n\n");
    if inputs.signals.is_empty() {
        out.push_str("None is waiting.\n");
    }
    out.push_str(&signal_lines(&inputs.signals));
    out.push_str("\n## the standing claims\n\n");
    if inputs.claims.is_empty() {
        out.push_str("None stands yet.\n");
    }
    for claim in &inputs.claims {
        out.push_str(&format!("- `{}` · {} · {}\n", claim.name, claim.title, claim.path));
    }
    out.push_str("\n## the open intents\n\n");
    if inputs.intents.is_empty() {
        out.push_str("None is open.\n");
    }
    for (id, subject, held) in &inputs.intents {
        out.push_str(&format!("- `{id}` · {subject} · {held} signal(s) attached\n"));
    }
    out.push_str(&format!("\n## elaboration types\n\n{}\n", inputs.types.join(", ")));
    out.push_str(&format!("\n## repositories\n\n{}\n", inputs.repositories.join(", ")));
    out.push_str(&format!(
        "\n## the deliverables, as files in this place\n\n\
         `{move_path}`: one record per unmoved signal, records separated by a blank line:\n\n\
         \x20   Signal: <the signal's id>\n\
         \x20   Move: attach | challenge | join | answered | route | drop\n\
         \x20   Target: <the open intent, the claim, the new intent's id, the decision, or the ask or chore offered; none for drop>\n\
         \x20   Reason: <one sentence a stranger could weigh>\n\n\
         `{proposal_path}`: one record per intent the joins propose:\n\n\
         \x20   Intent: intent/<a short name>\n\
         \x20   Subject: <what is unsettled, in one line>\n\
         \x20   Signals: <the ids it rests on, separated by spaces>\n\
         \x20   Elaboration: <a type above>\n\n\
         Then exit done naming both: --deliverable move --deliverable intent-proposal.\n",
        move_path = crate::offers::delivery_path("move"),
        proposal_path = crate::offers::delivery_path("intent-proposal"),
    ));
    out
}

/// Signals as an order lists them: the id, kind, who said it, the subjects and
/// the claims it argues with, then what it asserts and its excerpt quoted.
fn signal_lines(signals: &[crate::signals::Signal]) -> String {
    let mut out = String::new();
    for signal in signals {
        let mut head = format!("- `{}` · {}", signal.id, signal.kind);
        if !signal.asserted_by.is_empty() {
            head.push_str(&format!(" · {}", signal.asserted_by));
        }
        if !signal.subject_tags.is_empty() {
            head.push_str(&format!(" · subjects: {}", signal.subject_tags.join(", ")));
        }
        if !signal.argues_with.is_empty() {
            head.push_str(&format!(" · argues with: {}", signal.argues_with.join(", ")));
        }
        out.push_str(&head);
        out.push('\n');
        if !signal.assertion.trim().is_empty() {
            out.push_str(&format!("  {}\n", signal.assertion.trim()));
        }
        for line in signal.excerpt.lines().filter(|l| !l.trim().is_empty()) {
            out.push_str(&format!("  > {}\n", line.trim()));
        }
    }
    out
}

/// What an elaboration's session works from: the intent's question, the
/// signals it rests on with what each said, the claims they challenge, the
/// other intents a gathering covers, the standing claims its signals argue
/// with and where each stands in the book, the change directory on the
/// intent's line, and what its type delivers (116, 188, 190; context.yaml
/// sessions.self-closing).
#[derive(Debug, Clone, Default)]
pub struct Elaborating {
    pub intent: String,
    pub subject: String,
    pub signals: Vec<crate::signals::Signal>,
    pub challenges: Vec<String>,
    /// Each other intent a gathering covers: its id and its subject.
    pub covers: Vec<(String, String)>,
    pub claims: Vec<crate::changes::Claim>,
    pub change_directory: String,
    pub deliverables: Vec<String>,
}

/// The part of an elaboration session's work order that is its job.
pub fn elaboration(inputs: &Elaborating) -> String {
    let subject = match inputs.subject.trim().is_empty() {
        true => "The intent states no subject; its signals are the question.",
        false => inputs.subject.trim(),
    };
    let mut out = format!("## the question\n\n{subject} · `{}`\n", inputs.intent);
    out.push_str("\n## the signals it rests on\n\n");
    match inputs.signals.is_empty() {
        true => out.push_str("None is cited.\n"),
        false => out.push_str(&signal_lines(&inputs.signals)),
    }
    if !inputs.challenges.is_empty() {
        out.push_str("\n## the claims it challenges\n\n");
        for claim in &inputs.challenges {
            out.push_str(&format!("- `{claim}`\n"));
        }
    }
    if !inputs.covers.is_empty() {
        out.push_str("\n## the intents it covers as well\n\n");
        for (intent, subject) in &inputs.covers {
            out.push_str(&format!("- `{intent}` · {subject}\n"));
        }
    }
    out.push_str("\n## the chapters to read\n\n");
    match inputs.claims.is_empty() {
        true => out.push_str("No standing claim is argued with; the whole book is on disk in this place.\n"),
        false => {
            for claim in &inputs.claims {
                out.push_str(&format!("- `{}` · {} · {}\n", claim.name, claim.title, claim.path));
            }
        }
    }
    out.push_str(&format!(
        "\n## the change directory\n\n`{}` on the intent's line: earlier elaborations' records are there, \
         and this one's go there too.\n",
        inputs.change_directory
    ));
    if !inputs.deliverables.is_empty() {
        out.push_str(&format!(
            "\n## what the type delivers\n\n{}, written in this place and committed in one commit.\n",
            inputs.deliverables.join(", ")
        ));
    }
    out
}

/// The deliverables an elaboration type's one session asks for, by name, as
/// its type file's parameters state them (190).
pub fn type_deliverables(defs: &Definitions, type_name: &str) -> Vec<String> {
    let Some(machine) = defs.machines.get(type_name) else {
        return vec![];
    };
    machine
        .regions
        .values()
        .flat_map(|region| region.states.values())
        .find(|state| state.machine.as_deref() == Some("session"))
        .and_then(|state| asks_from(state.params.clone().unwrap_or_default().get("deliverables")).ok())
        .map(|asks| asks.into_iter().map(|ask| ask.name).collect())
        .unwrap_or_default()
}

/// What a capture-reader session reads a capture against: the capture's
/// provenance and its pointer, the standing claims a signal may argue with, and
/// the subject tags the instance keeps, where it keeps a vocabulary (111, 113,
/// 115; context.yaml sessions.capture-reader).
#[derive(Debug, Clone, Default)]
pub struct Reading {
    pub capture: crate::signals::Capture,
    pub claims: Vec<crate::changes::Claim>,
    /// `flywheel/signal-tags.yaml` as the blueprints hold it, when they do.
    pub tags: Option<String>,
}

/// The part of a capture-reader's work order that is its job: the capture whose
/// pointer it follows, the claims a signal names when it argues with one, the
/// tags, and the one file it delivers its signals in, parsed when it exits (111,
/// 113, 115; `instructions/schemas/signal.md`).
pub fn capture_reading(inputs: &Reading) -> String {
    let capture = &inputs.capture;
    let mut out = format!(
        "## the capture\n\n\
         - source: {}\n- event: {}\n- said at: {}\n- captured by: {}\n- the material: {}\n\n\
         Follow the pointer and read the material where it lies; copy none of it into this place.\n",
        capture.source, capture.key, capture.event_at, capture.captured_by, capture.raw
    );
    out.push_str("\n## the standing claims\n\n");
    if inputs.claims.is_empty() {
        out.push_str("None stands yet.\n");
    }
    for claim in &inputs.claims {
        out.push_str(&format!("- `{}` · {} · {}\n", claim.name, claim.title, claim.path));
    }
    out.push_str("\n## subject tags\n\n");
    match inputs.tags.as_deref().map(str::trim).filter(|tags| !tags.is_empty()) {
        Some(tags) => out.push_str(&format!("{tags}\n")),
        None => out.push_str("The instance keeps no vocabulary yet: name each subject in a word or two of the material's own.\n"),
    }
    out.push_str(&format!(
        "\n## the deliverable, as a file in this place\n\n\
         `{path}`: one record per signal, in the order the material says them, records separated by a blank line:\n\n\
         \x20   Kind: {kinds}\n\
         \x20   Said-by: <who asserted it>\n\
         \x20   Subjects: <tags, separated by spaces>\n\
         \x20   Assertion: <one sentence in the asserter's terms, saying no more than the excerpt supports>\n\
         \x20   Excerpt: <the words exactly as said; each further line of it begins with \"+ \">\n\
         \x20   Position: <where the excerpt sits in the material: a line range or a timestamp>\n\
         \x20   Argues-with: <the standing claims above it argues with, separated by spaces; empty when none>\n\n\
         Then exit done naming it: --deliverable signal.\n",
        path = crate::offers::delivery_path("signal"),
        kinds = crate::signals::KINDS.join(" | "),
    ));
    out
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
