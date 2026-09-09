//! `flywheel init`, as the instance machine drives it: create or adopt the
//! blueprints, create or adopt the state repository, record that the App must
//! be installed, register the first host (204).
//!
//! Every step is an effect with a proof, so running it again changes nothing
//! and a half-finished bootstrap is finished by the next tick (204). Nothing
//! here places a secret: the App's key is the operator's, always (207a).

use crate::git::{self, Repo};
use crate::manifest::{Manifest, Repository};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// What the blueprints template requires. A repository that has these is
/// adopted; one that is missing some has them added, and nothing else of it is
/// touched (204, 220).
pub const BLUEPRINTS_TEMPLATE: &[(&str, &str)] = &[
    (
        "flywheel/README.md",
        "# flywheel\n\nThe machinery's own directory: instructions, schemas, skills and type files.\nWritten by people, read by the machinery (203).\n",
    ),
    ("flywheel/unit-types/.keep", ""),
    ("flywheel/elaboration-types/.keep", ""),
    ("context-map/current.yaml", "nodes: []\n"),
    ("context-map/target.yaml", "nodes: []\n"),
    ("src/SUMMARY.md", "# Summary\n"),
];

/// The state repository's layout (C.2). Empty of objects; the directories are
/// what the profile names.
pub const STATE_LAYOUT: &[(&str, &str)] = &[
    (".flywheel", "state\n"),
    ("objects/.keep", ""),
    ("responses/.keep", ""),
    ("asks/.keep", ""),
    ("runs/.keep", ""),
];

/// Where a bootstrap writes and what it found there.
pub struct Bootstrap {
    /// The git host: a directory of bare repositories, which is what a git host
    /// is to everything below.
    pub git_host: PathBuf,
    /// Where the machinery works while it creates them.
    pub scratch: PathBuf,
    /// The set version this bootstrap stamps (208).
    pub set_version: String,
}

impl Bootstrap {
    pub fn new(git_host: impl Into<PathBuf>, scratch: impl Into<PathBuf>) -> Bootstrap {
        Bootstrap {
            git_host: git_host.into(),
            scratch: scratch.into(),
            set_version: flywheel_domain::set::SET_VERSION.to_string(),
        }
    }

    fn remote(&self, name: &str) -> PathBuf {
        self.git_host.join(format!("{name}.git"))
    }

    /// `create_blueprints`: create the blueprints repository from the template
    /// at the set's version, or adopt one that exists by adding what the
    /// template requires and is missing. The version is stamped in the
    /// instance's record (204, 208, 220).
    pub fn create_blueprints(&self, instance: &str) -> Result<Repository> {
        let name = format!("{instance}-blueprints");
        self.create_or_adopt(&name, BLUEPRINTS_TEMPLATE, "the blueprints template")
    }

    /// `create_state`: the state repository with the profile's layout, empty of
    /// objects; an existing one has what the layout requires added (204, 220,
    /// C.2).
    pub fn create_state(&self, instance: &str) -> Result<Repository> {
        let name = format!("{instance}-state");
        self.create_or_adopt(&name, STATE_LAYOUT, "the state repository's layout")
    }

    /// `create_repository`: a built repository under the instance, from the
    /// built-repository template at the set's version (206, 208).
    pub fn create_repository(&self, instance: &str, name: &str) -> Result<Repository> {
        let full = format!("{instance}-{name}");
        self.create_or_adopt(
            &full,
            &[("flywheel/.keep", ""), ("README.md", "")],
            "the built-repository template",
        )
    }

    /// The one act behind all three: what exists is adopted, what is missing is
    /// added, and a repeat writes nothing (204, 220).
    fn create_or_adopt(
        &self,
        name: &str,
        template: &[(&str, &str)],
        what: &str,
    ) -> Result<Repository> {
        let remote = self.remote(name);
        if !Repo::at(&remote).exists() {
            git::init_bare(&remote).with_context(|| format!("creating {name}"))?;
        }
        let checkout = self.scratch.join(name);
        let _ = std::fs::remove_dir_all(&checkout);
        let repo = git::checkout_line(&remote, &checkout, "main")
            .with_context(|| format!("checking out {name}"))?;

        for (path, content) in template {
            let full = repo.dir.join(path);
            // Adoption adds what is missing and rewrites nothing: a repository
            // an operator already had keeps everything it holds (220).
            if full.exists() {
                continue;
            }
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&full, content)?;
        }
        repo.git(&["add", "-A", "."])?;
        let said = repo.run(&["commit", "--quiet", "-m", &format!("{what} at set {}", self.set_version)])?;
        if said.ok {
            repo.git(&["push", "--quiet", "origin", "main"])?;
        }
        let _ = std::fs::remove_dir_all(&checkout);
        Ok(Repository {
            remote: remote.to_string_lossy().to_string(),
            shared_line: "main".into(),
            template_version: Some(self.set_version.clone()),
        })
    }

    /// `register_app`: the App's id in the manifest and the record that its
    /// installation is required. This installs nothing: the secret is placed by
    /// the operator, never by an agent (204, 207, 207a).
    pub fn register_app(manifest: &mut Manifest, app_id: &str, key_from: &str) {
        manifest.app.id = app_id.to_string();
        manifest.app.key_from = Some(key_from.to_string());
    }

    /// Whether the App's installation is seen through the git host. Until it
    /// is, the instance stands at a decision under attention and no agent acts
    /// (204, 207, 82).
    pub fn app_installed(manifest: &Manifest) -> bool {
        match &manifest.app.key_from {
            Some(from) => std::env::var(from).map(|k| !k.trim().is_empty()).unwrap_or(false),
            None => false,
        }
    }

    /// Every manifest repository the App's installation does not cover. Each is
    /// a decision under attention, and the machinery takes nothing on it until
    /// the installation is extended (207).
    pub fn uncovered_repositories(manifest: &Manifest) -> Vec<String> {
        manifest
            .all_repositories()
            .into_iter()
            .map(|(name, _)| name)
            .filter(|name| !manifest.app.installation_covers.contains(name))
            .collect()
    }

    /// `register_host`: the first host's record and manifest entry. Every later
    /// host joins by its own command (204, 205).
    pub fn register_host(manifest: &mut Manifest, host: &str, root: &Path) {
        manifest.hosts.entry(host.to_string()).or_insert_with(|| {
            crate::manifest::Host {
                root: root.to_path_buf(),
                // What this release binds: the lines and places recorded, the
                // operator as the session (93a, 93b, D8).
                workspace: "recorded".into(),
                sessions: "operator".into(),
                covers: vec![],
            }
        });
    }
}

/// Initialize an instance: the four steps of 204, each proving itself, so a
/// second run changes nothing and a half-finished one is finished here.
pub fn init(
    bootstrap: &Bootstrap,
    instance: &str,
    host: &str,
    root: &Path,
    app_id: &str,
    key_from: &str,
    existing: Option<Manifest>,
) -> Result<Manifest> {
    let mut manifest = existing.unwrap_or_else(|| Manifest {
        instance: instance.to_string(),
        ..Default::default()
    });
    manifest.instance = instance.to_string();
    if manifest.blueprints.remote.is_empty() {
        manifest.blueprints = bootstrap.create_blueprints(instance)?;
    }
    if manifest.state.remote.is_empty() {
        manifest.state = bootstrap.create_state(instance)?;
    }
    Bootstrap::register_app(&mut manifest, app_id, key_from);
    Bootstrap::register_host(&mut manifest, host, root);
    // The set version is stamped at initialization and at repository creation;
    // a newer set upgrades nothing on its own (208).
    manifest
        .template_version
        .get_or_insert_with(|| bootstrap.set_version.clone());
    Ok(manifest)
}
