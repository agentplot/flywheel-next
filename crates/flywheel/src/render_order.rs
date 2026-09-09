//! `flywheel render-order <session type> <instruction version> <scenario>`:
//! the exact prompt a session would be handed, with no session started (90,
//! 124).
//!
//! The renderer is `flywheel-domain`'s and is the one `prepare_place` uses; all
//! this adds is where the job comes from. A scenario describes a state of the
//! stores, so seeding it and reading the object the type works gives the job
//! without starting anything — no place is prepared, no session is recorded and
//! no agent is charged (90).

use anyhow::{Context, Result};
use flywheel_domain::context::Binding;
use flywheel_domain::deliverables::Deliverables;
use flywheel_domain::instructions::Instructions;
use flywheel_domain::order::{self, Job, Order};
use flywheel_domain::profile::Embedded;
use flywheel_scenario::conformance::{drive, Suite};
use std::path::Path;

/// Render the order and return it as the session would read it.
pub fn render_order(
    session_type: &str,
    instruction_version: u32,
    scenario: &Path,
    instructions: Option<&Path>,
) -> Result<String> {
    Ok(render(session_type, instruction_version, scenario, instructions)?.render())
}

/// The same, section by section, so a test asserts what the order carries
/// rather than reading it out of one string.
pub fn render(
    session_type: &str,
    instruction_version: u32,
    scenario: &Path,
    instructions: Option<&Path>,
) -> Result<Order> {
    let set = match instructions {
        Some(dir) => Instructions::load_dir(dir)
            .with_context(|| format!("loading the instruction set at {}", dir.display()))?,
        None => Instructions::shipped().context("loading the instruction set the binary carries")?,
    };
    if set.version != instruction_version {
        anyhow::bail!(
            "the instruction set loaded is version {} and version {instruction_version} was named; \
             `--instructions <dir>` reads another (123)",
            set.version
        );
    }
    let defs = flywheel_domain::set::load().context("loading the set the binary carries")?;
    let binding = Binding::load(&Embedded)?;
    let deliverables = Deliverables::load(&Embedded)?;
    let job = job_from(scenario, session_type, &defs)?;
    order::render(session_type, &job, &defs, &binding, &deliverables, &set)
}

/// The job the scenario describes: the object the session type works, as the
/// seeded stores hold it. Nothing is started and nothing is written.
fn job_from(
    path: &Path,
    session_type: &str,
    defs: &flywheel_engine::Definitions,
) -> Result<Job> {
    let (scenario, _) = flywheel_atoms::conformance::load(path)
        .with_context(|| format!("loading {}", path.display()))?;
    let suite = Suite::for_scenario(path)?;
    let runtime = drive::seed(defs.clone(), &scenario, &suite)?;
    let type_name = session_type.split('/').next().unwrap_or(session_type);

    // The object the type works: the one whose record names the type, else the
    // one whose machine is the type. A scenario that describes neither renders
    // the order with the job named and no record, which is what a type with no
    // object of its own — curation, the operator's session — has.
    let held = runtime
        .store
        .objects
        .values()
        .find(|o| o.record.get("type").and_then(|v| v.as_str()) == Some(type_name))
        .or_else(|| {
            runtime
                .store
                .objects
                .values()
                .find(|o| o.machine == type_name)
        });

    let object = held
        .map(|o| o.id.clone())
        .unwrap_or_else(|| format!("{}/{type_name}", scenario.scenario));
    let fields = held
        .map(|o| {
            o.record
                .iter()
                .map(|(name, value)| (name.clone(), value_of(value)))
                .collect()
        })
        .unwrap_or_default();
    Ok(Job {
        session: flywheel_domain::regions::session_key(&object, "life"),
        place: flywheel_domain::regions::place_key(&object, "life"),
        object,
        fields,
        // Phase 1 has no blueprints line under a scenario, so the order names
        // the point the scenario is as of: its own name. A host renders the
        // commit its place was taken at (226).
        blueprints_commit: format!("scenario:{}", scenario.scenario),
    })
}

fn value_of(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}
