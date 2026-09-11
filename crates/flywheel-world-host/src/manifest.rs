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
    /// `operator` — what starts a session on this host (93b, 217c).
    #[serde(default = "operator_sessions")]
    pub sessions: String,
    /// What this host takes leases within (149).
    #[serde(default)]
    pub covers: Vec<String>,
    /// How many sessions this host may run at once. A setting of the host, and
    /// so a setting of the manifest (31, 183, `host.yaml` record.bound).
    #[serde(default = "four")]
    pub bound: u32,
    /// A laptop by default: past its stale window it is away rather than gone,
    /// its leases standing (150a, `host.yaml` record.intermittent). A host that
    /// is always on says so here.
    #[serde(default = "yes")]
    pub intermittent: bool,
    /// This host's router: the manifest names one per host, and the host's one
    /// address is the private-network name it gives (191, 205a, D10a). Where a
    /// host names none the instance's own stands for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub router: Option<Router>,
    /// The port the page is also served on for the operator sitting at this
    /// machine. What 245 permits rather than requires; a link never names it
    /// (205a, 308, D10a).
    #[serde(default = "localhost_port")]
    pub localhost_port: u16,
}

fn localhost_port() -> u16 {
    4242
}

fn four() -> u32 {
    4
}

fn yes() -> bool {
    true
}

fn host_workspace() -> String {
    "host".to_string()
}

fn operator_sessions() -> String {
    "operator".to_string()
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

/// What curation is charged on: the count of unmoved signals that charges it
/// and the cadence that charges it anyway (110, 118, `curation.yaml`). The
/// operator sets these in `flywheel.yaml`; `init` writes them onto the
/// instance's curation record, where the evidence reads them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Curation {
    /// Unmoved signals that charge a curation. Twelve unless the operator says
    /// otherwise (110, `blueprints.yaml` evidence.curation.threshold).
    pub threshold: u64,
    /// The cadence that charges one whatever the count (110, 231).
    pub cadence: String,
}

impl Default for Curation {
    fn default() -> Self {
        Curation {
            threshold: 12,
            cadence: flywheel_domain::cadence::DEFAULT.to_string(),
        }
    }
}

/// How often the loop looks, in seconds. A tick is caused by a notify for one
/// object and by a sweep over the host's scopes; the poll is what the notify
/// falls back on when nothing told this host anything, and the sweep is what
/// makes an `older:` guard fire and a never-notified host converge
/// (model.md 2.1, 130, D6, D7). A host runs at the model's intervals and a
/// test's backstop is milliseconds, so they are settings and not constants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intervals {
    /// Seconds between one look at the shared line and the next (D6, 130).
    pub poll: f64,
    /// Seconds between sweeps, whatever the poll says (D7, 231).
    pub sweep: f64,
}

impl Default for Intervals {
    fn default() -> Self {
        Intervals {
            poll: 30.0,
            sweep: 60.0,
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
    /// What curation is charged on (110, 118).
    #[serde(default)]
    pub curation: Curation,
    /// How often the loop looks (D6, D7).
    #[serde(default)]
    pub intervals: Intervals,
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
