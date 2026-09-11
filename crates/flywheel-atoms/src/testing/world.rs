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
    /// Every write that changed something, in order. A test that asserts
    /// reading a record wrote nothing back reads this rather than counting
    /// commits (78, 114).
    written: Vec<String>,
}

impl FakeWorld {
    pub fn new() -> FakeWorld {
        FakeWorld::default()
    }

    /// How many writes changed the file at this path. Reading a record and
    /// finding it as it stands is not a write (78).
    pub fn writes_to(&self, path: &str) -> usize {
        self.written.iter().filter(|p| p.as_str() == path).count()
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

impl World for FakeWorld {
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
        if repository != "flywheel-state" && !path.starts_with("flywheel/") && by_response.is_none()
        {
            bail!("{repository}: `{path}` is outside the machinery's prefix (203)");
        }
        let text = String::from_utf8_lossy(body).to_string();
        if self.files.get(path) == Some(&text) {
            return Ok(false);
        }
        self.files.insert(path.to_string(), text);
        self.written.push(path.to_string());
        Ok(true)
    }
}
