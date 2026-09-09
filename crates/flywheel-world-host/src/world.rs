//! `World` as a host binds it: the manifest, the repositories cloned under the
//! host's root, the router and the App's tokens (D8, 205, 205a, 207).

use crate::git::{self, Repo};
use crate::manifest::Manifest;
use anyhow::{bail, Context, Result};
use flywheel_atoms::{Endpoint, RepositoryRef, World};
use serde_json::Value;
use std::path::PathBuf;

/// One host, serving one instance.
pub struct HostWorld {
    pub manifest: Manifest,
    /// The name this host is known by in the manifest.
    pub host: String,
    /// `<root>/<instance>/`, where every clone lives (205, 218).
    pub root: PathBuf,
}

impl HostWorld {
    pub fn open(manifest: Manifest, host: &str) -> Result<HostWorld> {
        let root = manifest.instance_root(host)?;
        Ok(HostWorld {
            manifest,
            host: host.to_string(),
            root,
        })
    }

    /// Where a repository's bare clone lives under the root.
    pub fn bare(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.git"))
    }

    /// Where its one checkout of the shared line lives. Places are worktrees
    /// made later; this is the machinery's own (205, 93a).
    pub fn checkout(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn repository(&self, name: &str) -> Result<&crate::manifest::Repository> {
        self.manifest
            .all_repositories()
            .into_iter()
            .find(|(n, _)| n == name)
            .map(|(_, r)| r)
            .ok_or_else(|| anyhow::anyhow!("the manifest tracks no repository `{name}`"))
    }
}

impl World for HostWorld {
    fn manifest(&self) -> Result<Value> {
        Ok(serde_json::to_value(&self.manifest)?)
    }

    fn repositories(&self) -> Result<Vec<RepositoryRef>> {
        Ok(self
            .manifest
            .all_repositories()
            .into_iter()
            .map(|(name, repository)| RepositoryRef {
                name,
                remote: repository.remote.clone(),
                shared_line: repository.shared_line.clone(),
            })
            .collect())
    }

    /// Clone the state, the blueprints and every tracked repository bare under
    /// the root, and check out each shared line once for the machinery's own
    /// merges. A repeat adds only what is missing (205).
    fn clone_repositories(&mut self) -> Result<()> {
        std::fs::create_dir_all(&self.root)
            .with_context(|| format!("making {}", self.root.display()))?;
        for (name, repository) in self.manifest.all_repositories() {
            let bare = self.bare(&name);
            if !Repo::at(&bare).exists() {
                git::clone_bare(std::path::Path::new(&repository.remote), &bare)
                    .with_context(|| format!("cloning {name}"))?;
            }
            let checkout = self.checkout(&name);
            if !checkout.join(".git").is_dir() {
                git::checkout_line(&bare, &checkout, &repository.shared_line)
                    .with_context(|| format!("checking out {name}"))?;
            }
        }
        Ok(())
    }

    /// A host has one address — its private-network name — with the instance in
    /// the path, and a link never names a localhost port (205a, D10a).
    fn route(&self, name: &str) -> Result<Endpoint> {
        let base = self.manifest.router.base.trim_end_matches('/');
        if base.contains("localhost") || base.contains("127.0.0.1") {
            bail!(
                "the router base `{base}` is a localhost address; a host's address is its \
                 private-network name, so a link opens on a phone (205a, D10a)"
            );
        }
        Ok(Endpoint {
            name: name.to_string(),
            url: format!("{base}/{}/{name}", self.manifest.instance),
        })
    }

    /// A token for the App. The key is read from where the operator placed it
    /// and is written nowhere else: not into configuration, not into code, not
    /// into the state repository (204, 207, 207a).
    fn app_token(&self) -> Result<String> {
        let Some(from) = &self.manifest.app.key_from else {
            bail!(
                "the manifest names no place for the App's key; the operator places it and the \
                 machinery reads it from there (207, 207a)"
            );
        };
        let key = std::env::var(from).map_err(|_| {
            anyhow::anyhow!(
                "the App's key is not where the manifest says the operator put it (`{from}`); \
                 no agent places it (207a)"
            )
        })?;
        if key.trim().is_empty() {
            bail!("`{from}` is empty; the App's key is the operator's to place (207a)");
        }
        // Short-lived, scoped to this installation, and never written down: the
        // token is handed back and kept nowhere (207).
        Ok(format!("installation:{}:{}", self.manifest.app.id, fingerprint(&key)))
    }

    fn read_file(&self, repository: &str, path: &str) -> Result<Option<Vec<u8>>> {
        let repo = self.repository(repository)?;
        let line = repo.shared_line.clone();
        let bare = Repo::at(self.bare(repository));
        if !bare.exists() {
            return Ok(None);
        }
        Ok(git::show(&bare, &line, path)?.map(|s| s.into_bytes()))
    }
}

/// A short, stable mark for a key, so a token can be told from another without
/// the key appearing anywhere.
fn fingerprint(key: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    format!("{hash:016x}")
}
