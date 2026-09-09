//! `flywheel.yaml`: what the operator wrote, and the only place a host reads
//! its own configuration from (183).
//!
//! One instance per manifest; a host runs several instances by holding one
//! root per instance and nothing under one names another (218).

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The manifest's own file name, on the blueprints' shared line and beside a
/// host's root.
pub const FILE: &str = "flywheel.yaml";

/// A repository the instance tracks (199, 205, 206). Git details and nothing
/// else: no kind, no capability, no scope (199).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Repository {
    pub remote: String,
    #[serde(default = "main")]
    pub shared_line: String,
    /// The set version the repository was created at (208).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_version: Option<String>,
}

fn main() -> String {
    "main".to_string()
}

/// What one host is told about itself.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Host {
    /// Everything this host clones lives under `<root>/<instance>/` (205, 218).
    pub root: PathBuf,
    /// `host` or `recorded` — which line-and-place binding is in force (93a).
    #[serde(default = "host_workspace")]
    pub workspace: String,
    /// What this host takes leases within (149).
    #[serde(default)]
    pub covers: Vec<String>,
}

fn host_workspace() -> String {
    "host".to_string()
}

/// The GitHub App: the one connection. Its id is here; its key is not, and
/// never is — the operator places it where `key_from` names (207, 207a).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct App {
    pub id: String,
    /// The environment variable the operator put the key in. The key itself
    /// appears in no configuration and no code (204, 207).
    #[serde(default)]
    pub key_from: Option<String>,
    /// The repositories the App's installation covers, as the git host reports
    /// them (207).
    #[serde(default)]
    pub installation_covers: Vec<String>,
}

/// The host's router: a host has one address, its private-network name, with
/// the instance in the path. Never a localhost port (205a, D10a).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Router {
    pub base: String,
}

impl Default for Router {
    fn default() -> Self {
        Router {
            base: "http://localhost".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifest {
    /// The operator's name for the instance; it need not be a git-host
    /// organization (219).
    pub instance: String,
    #[serde(default)]
    pub app: App,
    #[serde(default)]
    pub router: Router,
    pub blueprints: Repository,
    pub state: Repository,
    /// The built repositories the instance tracks (205, 206).
    #[serde(default)]
    pub repositories: BTreeMap<String, Repository>,
    #[serde(default)]
    pub hosts: BTreeMap<String, Host>,
    /// The set version initialization used (208).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_version: Option<String>,
}

impl Manifest {
    pub fn read(path: &Path) -> Result<Manifest> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading {}", path.display()))?;
        Manifest::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Manifest> {
        serde_yaml::from_str(text).context("parsing flywheel.yaml")
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, self.render()?)
            .with_context(|| format!("writing {}", path.display()))
    }

    pub fn render(&self) -> Result<String> {
        serde_yaml::to_string(self).context("rendering flywheel.yaml")
    }

    /// The host's entry, by name.
    pub fn host(&self, name: &str) -> Result<&Host> {
        self.hosts
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("the manifest names no host `{name}`"))
    }

    /// Everything this host keeps for this instance: `<root>/<instance>/`
    /// (205, 218).
    pub fn instance_root(&self, host: &str) -> Result<PathBuf> {
        Ok(self.host(host)?.root.join(&self.instance))
    }

    /// Every repository a host clones: the state, the blueprints and each
    /// tracked built repository (205).
    pub fn all_repositories(&self) -> Vec<(String, &Repository)> {
        let mut out = vec![
            ("flywheel-state".to_string(), &self.state),
            ("flywheel-blueprints".to_string(), &self.blueprints),
        ];
        for (name, repository) in &self.repositories {
            out.push((name.clone(), repository));
        }
        out
    }

    /// A repository record holds git details alone. A kind, a capability or a
    /// scope in one is refused: what a repository holds is read from the map
    /// and its own declarations, never from the manifest (199).
    pub fn check_repository_records(text: &str) -> Result<()> {
        for forbidden in ["kind:", "capability:", "capabilities:", "scope:"] {
            if let Some(at) = text.find(forbidden) {
                let line = text[..at].lines().count();
                bail!(
                    "flywheel.yaml line {line}: a repository record holds git details alone; \
                     `{}` is read from the map and the repository's own declarations (199)",
                    forbidden.trim_end_matches(':')
                );
            }
        }
        Ok(())
    }
}
