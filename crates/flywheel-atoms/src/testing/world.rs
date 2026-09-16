//! A `World` of the files it is given and nothing else, for the tests that
//! need a world and not a repository (D17).
//!
//! The page's capture box writes its material into the blueprints under the
//! machinery's prefix (111, 203), so a test that exercises it needs a world to
//! write into. This one holds the files in memory and enforces the same prefix
//! rule a host does, so a test cannot pass on a world looser than the real one.
//! Like `FakeStore` beside it, it behaves rather than being told what to
//! expect, and it is no binding: nothing in a shipped build reaches it.

use anyhow::{bail, Result};
use crate::traits::{Endpoint, RepositoryRef, World};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct FakeWorld {
    pub files: BTreeMap<String, String>,
    /// The built repositories the instance tracks (205, 206).
    pub repositories: Vec<RepositoryRef>,
    /// Every write that changed something, in order. A test that asserts
    /// reading a record wrote nothing back reads this rather than counting
    /// commits (78, 114).
    written: Vec<String>,
    /// What the writes would have cost a world that commits: one per write
    /// that changed something, and one for a delivery written together,
    /// whatever it held (110, 127).
    commits: usize,
    /// The pins on the git host, by repository and reference, each with the
    /// revision it holds (62).
    pub pinned: BTreeMap<(String, String), String>,
    /// Files as they stood at a revision of a repository, by repository,
    /// revision and path: what a place committed (62).
    pub at_revision: BTreeMap<(String, String, String), String>,
}

impl FakeWorld {
    pub fn new() -> FakeWorld {
        FakeWorld::default()
    }

    /// The same world, tracking one more built repository.
    pub fn tracking(mut self, name: &str) -> FakeWorld {
        self.repositories.push(RepositoryRef {
            name: name.to_string(),
            remote: format!("/git-host/{name}.git"),
            shared_line: "main".into(),
        });
        self
    }

    /// How many writes changed the file at this path. Reading a record and
    /// finding it as it stands is not a write (78).
    pub fn writes_to(&self, path: &str) -> usize {
        self.written.iter().filter(|p| p.as_str() == path).count()
    }

    /// How many commits what was written would have cost on a world that
    /// commits, so a test can assert that a delivery is one of them (110, 127).
    pub fn commits(&self) -> usize {
        self.commits
    }

    /// What was written under a path, for a test that reads it back.
    pub fn under(&self, prefix: &str) -> Vec<String> {
        self.files
            .keys()
            .filter(|p| p.starts_with(prefix))
            .cloned()
            .collect()
    }
}

/// What the machinery may write in a tracked repository without being asked
/// (203) — the same rule a host enforces, so a test cannot pass on a world
/// looser than the real one.
fn prefix(repository: &str, path: &str, by_response: Option<&str>) -> Result<()> {
    if repository != "flywheel-state" && !path.starts_with("flywheel/") && by_response.is_none() {
        bail!("{repository}: `{path}` is outside the machinery's prefix (203)");
    }
    Ok(())
}

impl World for FakeWorld {
    fn manifest(&self) -> Result<Value> {
        Ok(json!({}))
    }

    fn repositories(&self) -> Result<Vec<RepositoryRef>> {
        Ok(self.repositories.clone())
    }

    fn clone_repositories(&mut self) -> Result<()> {
        Ok(())
    }

    fn route(&self, name: &str) -> Result<Endpoint> {
        Ok(Endpoint {
            name: name.to_string(),
            url: format!("http://studio.tailnet.ts.net/{name}"),
        })
    }

    fn app_token(&self) -> Result<String> {
        Ok("test-token".into())
    }

    fn read_file(&self, _repository: &str, path: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.files.get(path).map(|s| s.as_bytes().to_vec()))
    }

    fn list_files(&self, _repository: &str, under: &str) -> Result<Vec<String>> {
        Ok(self.under(under))
    }

    fn write_file(
        &mut self,
        repository: &str,
        path: &str,
        body: &[u8],
        by_response: Option<&str>,
    ) -> Result<bool> {
        prefix(repository, path, by_response)?;
        let text = String::from_utf8_lossy(body).to_string();
        if self.files.get(path) == Some(&text) {
            return Ok(false);
        }
        self.files.insert(path.to_string(), text);
        self.written.push(path.to_string());
        self.commits += 1;
        Ok(true)
    }

    /// One commit for the whole delivery, whatever it holds, and the prefix
    /// rule read over every file first, so a refused one leaves nothing
    /// written (110, 127, 203).
    fn write_files(
        &mut self,
        repository: &str,
        files: &[(String, Vec<u8>)],
        by_response: Option<&str>,
    ) -> Result<usize> {
        for (path, _) in files {
            prefix(repository, path, by_response)?;
        }
        let before = self.commits;
        let mut changed = 0;
        for (path, body) in files {
            if self.write_file(repository, path, body, by_response)? {
                changed += 1;
            }
        }
        self.commits = before + usize::from(changed > 0);
        Ok(changed)
    }

    fn pin(&mut self, repository: &str, reference: &str, revision: &str) -> Result<bool> {
        let key = (repository.to_string(), reference.to_string());
        if self.pinned.get(&key).map(String::as_str) == Some(revision) {
            return Ok(false);
        }
        self.pinned.insert(key, revision.to_string());
        Ok(true)
    }

    fn read_pinned(&self, repository: &str, reference: &str, revision: &str, path: &str) -> Result<Option<Vec<u8>>> {
        if self.pinned.get(&(repository.to_string(), reference.to_string())).map(String::as_str) != Some(revision) {
            bail!("{repository}: no pin `{reference}` holds {revision} (62)");
        }
        Ok(self
            .at_revision
            .get(&(repository.to_string(), revision.to_string(), path.to_string()))
            .map(|s| s.as_bytes().to_vec()))
    }

    fn pins(&self, repository: &str, under: &str) -> Result<Vec<(String, String)>> {
        Ok(self
            .pinned
            .iter()
            .filter(|((r, reference), _)| r == repository && reference.starts_with(under))
            .map(|((_, reference), revision)| (reference.clone(), revision.clone()))
            .collect())
    }

    fn unpin(&mut self, repository: &str, reference: &str, revision: &str) -> Result<()> {
        let key = (repository.to_string(), reference.to_string());
        if self.pinned.get(&key).is_some_and(|held| held != revision) {
            bail!("{repository}: `{reference}` holds another revision than {revision}");
        }
        self.pinned.remove(&key);
        Ok(())
    }
}
