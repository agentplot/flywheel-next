//! The stand-in `World` and the recorded `Workspace`.
//!
//! `Workspace` recorded performs every effect of 42 by writing the fact the
//! effect's proof reads — `place.exists`, `place.merged`, `place.absent`,
//! `line.landed` — so the machines tick unchanged over those facts while
//! nothing they do reaches a repository (93a, D8). `World` stand-in answers
//! the manifest, the repositories and the router from the scenario's own
//! description.
//!
//! Both hold the store because that is where the facts live; the phase-2
//! implementations hold a checkout instead and answer the same methods.

use crate::store::{LineFact, Store};
use anyhow::Result;
use flywheel_atoms::{Endpoint, LandingPolicy, RepositoryRef, TakeOutcome, Workspace, World};
use serde_json::{json, Value};

/// The recorded workspace: the effects of 42, written as facts (93a).
pub struct RecordedWorkspace<'a> {
    pub store: &'a mut Store,
}

impl<'a> RecordedWorkspace<'a> {
    pub fn new(store: &'a mut Store) -> Self {
        RecordedWorkspace { store }
    }

    fn line(&mut self, line: &str) -> &mut LineFact {
        self.store.world.lines.entry(line.to_string()).or_default()
    }
}

impl Workspace for RecordedWorkspace<'_> {
    fn create_line(&mut self, line: &str, _parent: &str) -> Result<()> {
        self.line(line).exists = true;
        Ok(())
    }

    fn take_parent(&mut self, line: &str) -> Result<TakeOutcome> {
        // Nothing is merged, so nothing can conflict: what is unproved under
        // 93a is exactly the conflict, and the scenarios asserting one carry
        // `requires: [real-workspace]` and are skipped.
        self.line(line).exists = true;
        Ok(TakeOutcome::Done)
    }

    fn remove_line(&mut self, line: &str) -> Result<()> {
        let l = self.line(line);
        l.absent = true;
        l.exists = false;
        Ok(())
    }

    fn land_line(&mut self, line: &str, policy: LandingPolicy) -> Result<()> {
        let l = self.line(line);
        l.exists = true;
        match policy {
            LandingPolicy::Direct => {
                l.landed = true;
                l.landing = "passed".into();
            }
            LandingPolicy::PullRequest => l.landing = "open".into(),
        }
        Ok(())
    }

    fn write_acceptance(&mut self, line: &str, _body: &str) -> Result<()> {
        self.line(line).exists = true;
        Ok(())
    }

    fn prepare_place(&mut self, place: &str, _line: &str, _work_order: &str) -> Result<()> {
        let p = self.store.world.places.entry(place.to_string()).or_default();
        p.exists = true;
        p.absent = false;
        Ok(())
    }

    fn rebase_place(&mut self, place: &str) -> Result<TakeOutcome> {
        self.store.world.places.entry(place.to_string()).or_default();
        Ok(TakeOutcome::Done)
    }

    fn merge_place(&mut self, place: &str) -> Result<TakeOutcome> {
        self.store.world.places.entry(place.to_string()).or_default().merged = true;
        Ok(TakeOutcome::Done)
    }

    fn remove_place(&mut self, place: &str) -> Result<()> {
        let p = self.store.world.places.entry(place.to_string()).or_default();
        p.absent = true;
        p.exists = false;
        Ok(())
    }

    fn endpoints(&self, place: &str) -> Result<Vec<Endpoint>> {
        Ok(self
            .store
            .world
            .places
            .get(place)
            .map(|p| {
                p.endpoints
                    .iter()
                    .map(|url| Endpoint {
                        name: place.to_string(),
                        url: url.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }
}

/// The stand-in world: the manifest, the repositories, the router and the
/// files, as the scenario described them.
pub struct StandInWorld<'a> {
    pub store: &'a mut Store,
    /// What `given.files` and `files:` steps put on disk, by path.
    pub manifest: Value,
}

impl<'a> StandInWorld<'a> {
    pub fn new(store: &'a mut Store) -> Self {
        StandInWorld {
            store,
            manifest: json!({"hosts": {"local": {"workspace": "recorded", "sessions": "operator"}}}),
        }
    }
}

impl World for StandInWorld<'_> {
    fn manifest(&self) -> Result<Value> {
        Ok(self.manifest.clone())
    }

    fn repositories(&self) -> Result<Vec<RepositoryRef>> {
        Ok(self
            .store
            .world
            .declarations
            .keys()
            .map(|name| RepositoryRef {
                name: name.clone(),
                remote: format!("stand-in:{name}"),
                shared_line: "main".into(),
            })
            .collect())
    }

    fn clone_repositories(&mut self) -> Result<()> {
        // Nothing is cloned: the stand-in world has no disk.
        Ok(())
    }

    fn route(&self, name: &str) -> Result<Endpoint> {
        Ok(Endpoint {
            name: name.to_string(),
            url: format!("http://local.stand-in/{name}"),
        })
    }

    fn app_token(&self) -> Result<String> {
        Ok("stand-in-token".into())
    }

    fn read_file(&self, _repository: &str, path: &str) -> Result<Option<Vec<u8>>> {
        Ok(self
            .store
            .world
            .files
            .get(path)
            .map(|s| s.as_bytes().to_vec()))
    }

    fn list_files(&self, _repository: &str, under: &str) -> Result<Vec<String>> {
        Ok(self
            .store
            .world
            .files
            .keys()
            .filter(|path| path.starts_with(under))
            .cloned()
            .collect())
    }

    fn write_file(
        &mut self,
        repository: &str,
        path: &str,
        body: &[u8],
        by_response: Option<&str>,
    ) -> Result<bool> {
        // The prefix rule holds on the stand-in too: a scenario that writes
        // outside it fails here rather than passing on a world that let it
        // (203).
        prefix_check(repository, path, by_response)?;
        let text = String::from_utf8_lossy(body).to_string();
        if self.store.world.files.get(path) == Some(&text) {
            return Ok(false);
        }
        self.store.world.files.insert(path.to_string(), text);
        Ok(true)
    }
}

/// What the machinery may write in a tracked repository without being asked
/// (203). The same rule `flywheel-world-host` enforces, stated once here so the
/// stand-in cannot be looser than the host.
fn prefix_check(repository: &str, path: &str, by_response: Option<&str>) -> Result<()> {
    if repository == "flywheel-state" || path.starts_with("flywheel/") || by_response.is_some() {
        return Ok(());
    }
    anyhow::bail!(
        "{repository}: `{path}` is outside the machinery's prefix `flywheel/` and no response \
         asked for it (203)"
    )
}


/// A world with no repositories on disk: the files it is given and nothing
/// else (D1).
///
/// It owns its map rather than borrowing the store's, so a caller may hold it
/// beside a store — which is what the page does, one write to each on a
/// capture (111, 203). The prefix rule holds here as it does on a host.
#[derive(Debug, Default)]
pub struct FilesWorld {
    pub files: std::collections::BTreeMap<String, String>,
}

impl FilesWorld {
    pub fn new() -> FilesWorld {
        FilesWorld::default()
    }
}

impl World for FilesWorld {
    fn manifest(&self) -> Result<Value> {
        Ok(json!({}))
    }

    fn repositories(&self) -> Result<Vec<RepositoryRef>> {
        Ok(vec![])
    }

    fn clone_repositories(&mut self) -> Result<()> {
        Ok(())
    }

    fn route(&self, name: &str) -> Result<Endpoint> {
        Ok(Endpoint {
            name: name.to_string(),
            url: format!("http://local.stand-in/{name}"),
        })
    }

    fn app_token(&self) -> Result<String> {
        Ok("stand-in-token".into())
    }

    fn read_file(&self, _repository: &str, path: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.files.get(path).map(|s| s.as_bytes().to_vec()))
    }

    fn list_files(&self, _repository: &str, under: &str) -> Result<Vec<String>> {
        Ok(self
            .files
            .keys()
            .filter(|path| path.starts_with(under))
            .cloned()
            .collect())
    }

    fn write_file(
        &mut self,
        repository: &str,
        path: &str,
        body: &[u8],
        by_response: Option<&str>,
    ) -> Result<bool> {
        prefix_check(repository, path, by_response)?;
        let text = String::from_utf8_lossy(body).to_string();
        if self.files.get(path) == Some(&text) {
            return Ok(false);
        }
        self.files.insert(path.to_string(), text);
        Ok(true)
    }
}


/// The stand-in world's files live on the store, so a caller that needs both at
/// once — the capture tool writes one record to each (111, 203) — takes them
/// out for the call and puts them back.
pub fn with_files<T>(
    store: &mut Store,
    act: impl FnOnce(&mut Store, &mut FilesWorld) -> T,
) -> T {
    let mut world = FilesWorld {
        files: std::mem::take(&mut store.world.files),
    };
    let out = act(store, &mut world);
    store.world.files = std::mem::take(&mut world.files);
    out
}
