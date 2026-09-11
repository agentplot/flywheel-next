//! Fakes for the tests: a `World` of the files it is given and nothing else.
//!
//! The page's capture box writes its material into the blueprints under the
//! machinery's prefix (111, 203), so a test that exercises it needs a world to
//! write into. This one holds the files in memory and enforces the same prefix
//! rule a host does, so a test cannot pass on a world looser than the real one.
#![allow(dead_code)]
// Behind the `testing` feature, and always present for this crate's own unit
// tests (D17).

use anyhow::{bail, Result};
use flywheel_atoms::{Endpoint, RepositoryRef, World};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct Files {
    pub files: BTreeMap<String, String>,
}

impl Files {
    pub fn new() -> Files {
        Files::default()
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

impl World for Files {
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
        Ok(true)
    }
}
