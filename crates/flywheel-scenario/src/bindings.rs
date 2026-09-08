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
}
