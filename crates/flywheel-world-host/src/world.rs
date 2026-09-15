//! `World` as a host binds it: the manifest, the repositories cloned under the
//! host's root, the router and the App's tokens (D8, 205, 205a, 207).

use crate::git::{self, Repo};
use crate::manifest::Manifest;
use anyhow::{bail, Context, Result};
use flywheel_atoms::{Endpoint, RepositoryRef, World};
use serde_json::Value;
use std::path::PathBuf;

/// Whether an address is the machine's own rather than a name on the
/// operator's private network (205a, D10a).
pub fn is_localhost(base: &str) -> bool {
    base.contains("localhost") || base.contains("127.0.0.1") || base.contains("[::1]")
}

/// Whether an address names its port: `http://localhost:4242` does,
/// `http://localhost` and `http://[::1]` do not.
pub fn names_a_port(base: &str) -> bool {
    let rest = base.split_once("//").map(|(_, r)| r).unwrap_or(base);
    let authority = rest.split('/').next().unwrap_or_default();
    let after_ipv6 = authority.rsplit_once(']').map(|(_, r)| r).unwrap_or(authority);
    after_ipv6.contains(':')
}

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
        let world = HostWorld {
            manifest,
            host: host.to_string(),
            root,
        };
        for (name, _) in world.manifest.all_repositories() {
            world.move_an_earlier_checkout(&name)?;
        }
        Ok(world)
    }

    /// Where a repository's bare clone lives under the root.
    pub fn bare(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.git"))
    }

    /// Where its one checkout of the shared line lives:
    /// `<root>/<instance>/<repo>/main`, beside which the profile keeps a
    /// repository's `bolts/` and `places/`. Places are worktrees made later;
    /// this is the machinery's own, and no session runs here (205, 93a,
    /// `host.yaml` disk.shared_line).
    pub fn checkout(&self, name: &str) -> PathBuf {
        self.root.join(name).join("main")
    }

    /// A host this binary did not make kept its checkout at `<repo>` itself.
    /// The layout is the machinery's own and not a hand-made one, so it is
    /// moved under `main` once rather than refused. Nothing else is at
    /// `<repo>`: places and lines are worktrees of the bare clone, elsewhere
    /// under the root, and a checkout is a plain clone that names no path of
    /// its own (205).
    fn move_an_earlier_checkout(&self, name: &str) -> Result<()> {
        let earlier = self.root.join(name);
        if !earlier.join(".git").is_dir() || self.checkout(name).exists() {
            return Ok(());
        }
        let aside = self.root.join(format!(".{name}.moving"));
        std::fs::rename(&earlier, &aside)
            .with_context(|| format!("moving {} aside", earlier.display()))?;
        std::fs::create_dir_all(&earlier)?;
        std::fs::rename(&aside, self.checkout(name))
            .with_context(|| format!("moving {name}'s checkout under main"))?;
        Ok(())
    }

    /// The router in force for a host: its own where the manifest gives it one,
    /// the instance's otherwise (191, 205a).
    pub fn router_of(&self, host: &str) -> &crate::manifest::Router {
        self.manifest
            .hosts
            .get(host)
            .and_then(|h| h.router.as_ref())
            .unwrap_or(&self.manifest.router)
    }

    /// A host's one address, with the instance in the path: the name its
    /// router gives it on the operator's private network, or a localhost port
    /// when it serves this computer alone (191, 205a, D10a). A localhost base
    /// that names no port is at the port the page is served on there, so a
    /// link written at it opens (245, 308).
    pub fn address_of(&self, host: &str) -> Result<String> {
        let base = self.router_of(host).base.trim_end_matches('/');
        let base = match is_localhost(base) && !names_a_port(base) {
            true => format!("{base}:{}", self.localhost_port_of(host)),
            false => base.to_string(),
        };
        Ok(format!("{base}/{}", self.manifest.instance))
    }

    /// The port the page is also served on for the operator at this machine
    /// (245).
    pub fn localhost_port_of(&self, host: &str) -> u16 {
        self.manifest
            .hosts
            .get(host)
            .map(|h| h.localhost_port)
            .unwrap_or(4242)
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
                // The state repository's checkout is the state store's, and a
                // durable write is one that reached the git host: it is cloned
                // from the remote so its pushes land there (133, 161). Every
                // other repository is read from the mirror this host keeps and
                // written back through it.
                let from = match name.as_str() {
                    "flywheel-state" => PathBuf::from(&repository.remote),
                    _ => bare.clone(),
                };
                git::checkout_line(&from, &checkout, &repository.shared_line)
                    .with_context(|| format!("checking out {name}"))?;
            }
        }
        Ok(())
    }

    /// A host has one address, with the instance in the path, and every
    /// endpoint is at it (205a, D10a).
    fn route(&self, name: &str) -> Result<Endpoint> {
        Ok(Endpoint {
            name: name.to_string(),
            url: format!("{}/{name}", self.address_of(&self.host)?),
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

    fn line_log(&self, repository: &str, line: &str, limit: usize) -> Result<Vec<flywheel_atoms::CommitRef>> {
        let bare = Repo::at(self.bare(repository));
        if !bare.exists() {
            return Ok(vec![]);
        }
        // A line's own commits: what it carries that the shared line does not
        // yet. The shared line itself is read whole (185).
        let shared = self.repository(repository)?.shared_line.clone();
        let rev = match line == shared {
            true => line.to_string(),
            false => format!("{shared}..{line}"),
        };
        git::log(&bare, &rev, limit)
    }

    fn list_files(&self, repository: &str, under: &str) -> Result<Vec<String>> {
        let repo = self.repository(repository)?;
        let line = repo.shared_line.clone();
        let bare = Repo::at(self.bare(repository));
        if !bare.exists() {
            return Ok(vec![]);
        }
        git::ls_tree(&bare, &line, under)
    }

    /// One commit on the repository's shared line, refused outside the
    /// machinery's prefix unless a response asked for it (203).
    ///
    /// The bytes already there are not written again: reading twice with
    /// nothing changed writes nothing (78, 127).
    fn write_file(
        &mut self,
        repository: &str,
        path: &str,
        body: &[u8],
        by_response: Option<&str>,
    ) -> Result<bool> {
        crate::prefix::check(repository, path, by_response)?;
        if self.read_file(repository, path)?.as_deref() == Some(body) {
            return Ok(false);
        }
        let entry = self.repository(repository)?;
        let line = entry.shared_line.clone();
        let checkout = Repo::at(self.checkout(repository));
        if !checkout.exists() {
            bail!(
                "this host has no checkout of `{repository}`; it joins by one command and never \
                 by hand (205)"
            );
        }
        let text = String::from_utf8_lossy(body).to_string();
        let reason = match by_response {
            Some(response) => format!("{path}\n\nreason: the effect of response {response} (203)"),
            None => format!("{path}\n\nreason: the machinery's own material, under its prefix (203)"),
        };
        git::commit_file(&checkout, &line, path, &text, &reason)?;
        Ok(true)
    }

    /// `git push origin <revision>:<reference>` from the bare clone, whose
    /// objects every place on this host shares, and then the same reference in
    /// the clone: a pin the clone holds is one that reached the git host, so a
    /// repeat pushes nothing (62, 232, `record-derived.yaml` record_offers).
    fn pin(&mut self, repository: &str, reference: &str, revision: &str) -> Result<bool> {
        let bare = Repo::at(self.bare(repository));
        if git::reference(&bare, reference).as_deref() == Some(revision) {
            return Ok(false);
        }
        bare.git(&["push", "--quiet", "origin", &format!("{revision}:{reference}")])
            .with_context(|| format!("pinning {revision} of {repository} as {reference} on the git host"))?;
        bare.git(&["update-ref", reference, revision])?;
        Ok(true)
    }

    /// Read in process from the bare clone. A clone that lacks the revision —
    /// the offer was made on another host — fetches the pin first (62, 232,
    /// `host.yaml` prepare_place).
    fn read_pinned(&self, repository: &str, reference: &str, revision: &str, path: &str) -> Result<Option<Vec<u8>>> {
        let bare = Repo::at(self.bare(repository));
        if !git::holds(&bare, revision) {
            bare.git(&["fetch", "--quiet", "origin", &format!("+{reference}:{reference}")])
                .with_context(|| format!("fetching {reference} of {repository}, which pins {revision} (62, 232)"))?;
        }
        Ok(git::show(&bare, revision, path)?.map(String::into_bytes))
    }

    fn pins(&self, repository: &str, under: &str) -> Result<Vec<(String, String)>> {
        let bare = Repo::at(self.bare(repository));
        if !bare.exists() {
            return Ok(vec![]);
        }
        git::references(&bare, under)
    }

    /// `git push origin :<reference>` leased on the revision it must still
    /// hold, and then the reference out of the clone. A pin the git host no
    /// longer holds — another host's reconciliation removed it — leaves the
    /// clone too (55, 62, `host.yaml` remove_stale_offer_pins).
    fn unpin(&mut self, repository: &str, reference: &str, revision: &str) -> Result<()> {
        let bare = Repo::at(self.bare(repository));
        let pushed = bare.run(&[
            "push",
            "--quiet",
            &format!("--force-with-lease={reference}:{revision}"),
            "origin",
            &format!(":{reference}"),
        ])?;
        if !pushed.ok && !bare.git(&["ls-remote", "origin", reference])?.trim().is_empty() {
            bail!("removing {reference} of {repository} from the git host: {}", pushed.err.trim());
        }
        bare.run(&["update-ref", "-d", reference, revision])?;
        Ok(())
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
