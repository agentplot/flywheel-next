//! The trace: the structured record the assertions evaluate, and the document
//! a person reads, rendered from it (95).
//!
//! `<scenario>.trace.json` is written first and `<scenario>.trace.md` is
//! rendered from that file's own contents, so the document and the evidence a
//! failure cites cannot diverge.

use super::{Run, RunOptions};
use anyhow::{Context, Result};
use flywheel_atoms::conformance::Scenario;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// What a trace holds. The six things a trace lists are its six fields' worth:
/// the ticks, the guards, the transitions, the effects, the decisions and
/// their numbers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trace {
    pub scenario: String,
    pub title: String,
    pub profile: String,
    pub satisfies: Vec<u32>,
    pub ticks: Vec<crate::runner::TickRecord>,
    /// The decisions standing after each step, counting from 1.
    pub decisions_after: Vec<Vec<crate::runner::DecisionRecord>>,
    /// Anything the run could not do as written, said out loud.
    pub skipped_steps: Vec<String>,
}

impl Trace {
    /// What the trace lists, by name; the assertion `trace_lists` reads this.
    pub fn lists(&self) -> Vec<&'static str> {
        vec![
            "ticks",
            "guards",
            "transitions",
            "effects",
            "decisions",
            "numbers",
        ]
    }

    /// The document a person reads, rendered from this record alone.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# {} · {}\n\n", self.scenario, self.title));
        out.push_str(&format!(
            "profile: {} · satisfies: {}\n\n",
            self.profile,
            self.satisfies
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        for tick in &self.ticks {
            out.push_str(&format!("## tick {} · {}\n\n", tick.tick, tick.at.to_rfc3339()));
            if tick.guards.is_empty() {
                out.push_str("no guard matched\n\n");
            } else {
                out.push_str("| guard matched | object | region | from |\n|---|---|---|---|\n");
                for g in &tick.guards {
                    out.push_str(&format!(
                        "| {} | {} | {} | {} |\n",
                        g.matched, g.object, g.region, g.from
                    ));
                }
                out.push('\n');
            }
            if !tick.transitions.is_empty() {
                out.push_str("| transition | by |\n|---|---|\n");
                for t in &tick.transitions {
                    out.push_str(&format!(
                        "| {} {} → {} | {} |\n",
                        t.object,
                        t.from,
                        t.to,
                        t.response.clone().unwrap_or_else(|| "the tick".into())
                    ));
                }
                out.push('\n');
            }
            if !tick.effects.is_empty() {
                out.push_str("| effect | object | effect id |\n|---|---|---|\n");
                for e in &tick.effects {
                    out.push_str(&format!("| {} | {} | {} |\n", e.name, e.object, e.effect_id));
                }
                out.push('\n');
            }
            if !tick.decisions.is_empty() {
                out.push_str("| decision | number |\n|---|---|\n");
                for d in &tick.decisions {
                    out.push_str(&format!(
                        "| {} | {} |\n",
                        d.id,
                        d.number.map(|n| n.to_string()).unwrap_or_else(|| "—".into())
                    ));
                }
                out.push('\n');
            }
        }
        if !self.skipped_steps.is_empty() {
            out.push_str("## not run as written\n\n");
            for s in &self.skipped_steps {
                out.push_str(&format!("- {s}\n"));
            }
        }
        out
    }
}

/// Write the trace. The json is written, then read back and rendered, so the
/// markdown is a rendering of the file and not of what was in memory.
pub fn write(scenario: &Scenario, run: &Run, options: &RunOptions) -> Result<PathBuf> {
    let dir = options.trace_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("making the trace directory {}", dir.display()))?;
    let trace = Trace {
        scenario: scenario.scenario.clone(),
        title: scenario.title.clone(),
        profile: options.profile.name().to_string(),
        satisfies: scenario.satisfies.clone(),
        ticks: run.ticks.clone(),
        decisions_after: run.decisions_after.clone(),
        skipped_steps: run.skipped_steps.clone(),
    };
    let json_path = dir.join(format!("{}.trace.json", scenario.scenario));
    std::fs::write(&json_path, serde_json::to_string_pretty(&trace)?)
        .with_context(|| format!("writing {}", json_path.display()))?;
    render_from(&json_path)?;
    Ok(json_path)
}

/// Render `<name>.trace.md` from `<name>.trace.json`.
pub fn render_from(json_path: &Path) -> Result<PathBuf> {
    let text = std::fs::read_to_string(json_path)
        .with_context(|| format!("reading {}", json_path.display()))?;
    let trace: Trace = serde_json::from_str(&text)
        .with_context(|| format!("reading {} as a trace", json_path.display()))?;
    let md_path = json_path.with_extension("md");
    std::fs::write(&md_path, trace.render())
        .with_context(|| format!("writing {}", md_path.display()))?;
    Ok(md_path)
}
