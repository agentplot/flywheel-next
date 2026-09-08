//! flywheel-scenario: the stand-in state store and the scenario runner.
//! Sessions are played from a script; the engine, the register, the tail and
//! the effects run for real.

pub mod runner;
pub mod scenario;
pub mod store;
pub mod world;

pub use runner::Runtime;
pub use store::Store;

use anyhow::{Context, Result};
use std::path::Path;

pub fn save(store: &Store, path: &Path) -> Result<()> {
    if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; }
    std::fs::write(path, serde_json::to_string_pretty(store)?).with_context(|| format!("writing {}", path.display()))
}

pub fn load(path: &Path) -> Result<Store> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(serde_json::from_str(&text)?)
}
