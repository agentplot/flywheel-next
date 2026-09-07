//! Loading definitions from a directory of YAML files.

use crate::defs::{Atoms, Definitions, Machine};
use anyhow::{Context, Result};
use std::path::Path;

/// Load every `*.yaml` machine under `dir` (recursively), and `atoms.yaml` if present.
pub fn load_dir(dir: &Path) -> Result<Definitions> {
    let mut defs = Definitions::default();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let mut entries: Vec<_> = std::fs::read_dir(&d)
            .with_context(|| format!("reading {}", d.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let Some(ext) = p.extension().and_then(|e| e.to_str()) else { continue };
            if ext != "yaml" && ext != "yml" {
                continue;
            }
            let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
            if p.file_name().and_then(|f| f.to_str()) == Some("atoms.yaml") {
                defs.atoms = serde_yaml::from_str::<Atoms>(&text).with_context(|| format!("parsing {}", p.display()))?;
                continue;
            }
            if !text.contains("\nmachine:") && !text.starts_with("machine:") {
                continue;
            }
            let m: Machine = serde_yaml::from_str(&text).with_context(|| format!("parsing {}", p.display()))?;
            // A type file is `<machine>@<version>.yaml`; when several versions of one name are
            // present the highest wins for a bare reference (a pinned `name@N` is the record's).
            match defs.machines.get(&m.machine) {
                Some(have) if have.version >= m.version => {}
                _ => { defs.machines.insert(m.machine.clone(), m); }
            }
        }
    }
    Ok(defs)
}
