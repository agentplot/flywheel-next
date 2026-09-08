//! The suite's two binding files: `schema.json`, which closes every
//! vocabulary, and `observations.yaml`, which binds every key a scenario may
//! assert under `then.state_store`.
//!
//! A name nothing binds is an error and never a silent pass (D15).

use anyhow::{anyhow, bail, Context, Result};
use flywheel_atoms::conformance::Scenario;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The observation registry.
#[derive(Debug, Clone, Deserialize)]
pub struct Observations {
    pub observations: BTreeMap<String, Observation>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Observation {
    /// `all`, or the profiles that answer this key.
    pub profiles: Vec<String>,
    #[serde(default)]
    pub doc: Option<String>,
}

impl Observation {
    pub fn answered_by(&self, profile: &str) -> bool {
        self.profiles.iter().any(|p| p == "all" || p == profile)
    }
}

/// The conformance directory: the schema, the observations and the fixtures.
pub struct Suite {
    pub root: PathBuf,
    pub schema: jsonschema::Validator,
    pub observations: Observations,
}

impl Suite {
    /// The conformance root a scenario file sits under: the nearest ancestor
    /// holding `schema.json`.
    pub fn root_for(path: &Path) -> Result<PathBuf> {
        let start = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent()
                .ok_or_else(|| anyhow!("{} has no directory", path.display()))?
                .to_path_buf()
        };
        let start = start
            .canonicalize()
            .with_context(|| format!("resolving {}", start.display()))?;
        for dir in start.ancestors() {
            if dir.join("schema.json").is_file() && dir.join("observations.yaml").is_file() {
                return Ok(dir.to_path_buf());
            }
        }
        bail!(
            "no conformance root above {}: none of its directories holds schema.json and observations.yaml",
            path.display()
        )
    }

    pub fn for_scenario(path: &Path) -> Result<Suite> {
        Suite::open(&Suite::root_for(path)?)
    }

    pub fn open(root: &Path) -> Result<Suite> {
        let schema_text = std::fs::read_to_string(root.join("schema.json"))
            .with_context(|| format!("reading {}", root.join("schema.json").display()))?;
        let schema_json: Value = serde_json::from_str(&schema_text).context("schema.json")?;
        let schema = jsonschema::validator_for(&schema_json)
            .map_err(|e| anyhow!("schema.json is not a schema: {e}"))?;
        let obs_text = std::fs::read_to_string(root.join("observations.yaml"))
            .with_context(|| format!("reading {}", root.join("observations.yaml").display()))?;
        let observations: Observations =
            serde_yaml::from_str(&obs_text).context("observations.yaml")?;
        Ok(Suite {
            root: root.to_path_buf(),
            schema,
            observations,
        })
    }

    /// Where every path a scenario names resolves.
    pub fn fixtures(&self) -> PathBuf {
        self.root.join("fixtures")
    }

    /// Validate a scenario document against the schema. Every vocabulary is
    /// closed, so a misspelled key fails here rather than being ignored.
    pub fn validate(&self, document: &Value) -> Result<()> {
        let problems: Vec<String> = self
            .schema
            .iter_errors(document)
            .map(|e| format!("  {}: {e}", e.instance_path()))
            .collect();
        if problems.is_empty() {
            return Ok(());
        }
        bail!(
            "invalid against schema.json:\n{}",
            problems.join("\n")
        )
    }

    /// Every name the scenario uses that something must bind: the observation
    /// keys, the evidence and effect names, and the hooks' one placement rule.
    pub fn check_names(&self, scenario: &Scenario, path: &Path) -> Result<()> {
        let mut problems = Vec::new();

        // Hooks force a race the machinery is built to prevent, so they belong
        // to the proofs of B.2 and nowhere else.
        let under_contract = path.components().any(|c| c.as_os_str() == "contract");
        if !scenario.hooks.is_empty() && !under_contract {
            problems.push(format!(
                "declares hooks {:?} outside contract/",
                scenario.hooks
            ));
        }

        // Every `then.state_store` key is bound in observations.yaml, and every
        // profile the scenario runs on answers it.
        for key in scenario.then.state_store.keys() {
            match self.observations.observations.get(key) {
                None => problems.push(format!(
                    "asserts state_store key `{key}`, which observations.yaml does not bind"
                )),
                Some(o) => {
                    for profile in ["stand-in", "git-only", "tracker"] {
                        if scenario.runs_on(profile) && !o.answered_by(profile) {
                            problems.push(format!(
                                "asserts `{key}`, which the {profile} profile does not answer"
                            ));
                        }
                    }
                }
            }
        }

        // An evidence name outside the atoms file is a name nothing binds. The
        // contract set runs over the toy machine, whose atoms are its own.
        if !under_contract {
            for name in scenario.given.evidence.keys() {
                if !flywheel_atoms::Evidence::contains(name) {
                    problems.push(format!("seeds evidence `{name}`, which is no atom"));
                }
            }
            for step in scenario.when.iter() {
                if let Some(Value::Object(map)) = step.get("evidence") {
                    for name in map.keys() {
                        if !flywheel_atoms::Evidence::contains(name) {
                            problems.push(format!("sets evidence `{name}`, which is no atom"));
                        }
                    }
                }
            }
            for effect in &scenario.then.effects {
                if !flywheel_atoms::Effect::contains(&effect.r#do) {
                    problems.push(format!("asserts effect `{}`, which is no atom", effect.r#do));
                }
            }
        }

        // The steps themselves: an unknown key, a host step with two
        // transitions, a `direct` arm missing its fields.
        match scenario.steps() {
            Ok(steps) => {
                for (n, step) in steps.iter().enumerate() {
                    if let flywheel_atoms::conformance::Step::Host(h) = step {
                        if let Err(e) = h.transition() {
                            problems.push(format!("step {}: {e}", n + 1));
                        }
                    }
                }
            }
            Err(e) => problems.push(format!("{e:#}")),
        }

        if problems.is_empty() {
            return Ok(());
        }
        bail!("{}", problems.join("\n"))
    }

    /// Resolve a path a scenario names against `fixtures/`. A path naming a
    /// file there is materialized; a value that is not a fixture path is taken
    /// as inline content; a path resolving to neither is invalid.
    pub fn resolve_fixture(&self, path: &str, value: &str) -> Result<Vec<u8>> {
        // The value may itself name a fixture.
        for candidate in [value, path] {
            let file = self.fixtures().join(candidate.trim_start_matches('/'));
            if file.is_file() {
                return std::fs::read(&file)
                    .with_context(|| format!("reading the fixture {}", file.display()));
            }
        }
        if !value.is_empty() {
            // Inline content for the path it is keyed by.
            return Ok(value.as_bytes().to_vec());
        }
        bail!(
            "`{path}` resolves to no fixture under {} and carries no inline content",
            self.fixtures().display()
        )
    }
}
