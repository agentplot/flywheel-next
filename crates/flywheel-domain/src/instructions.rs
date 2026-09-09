//! The shipped instruction set: the schemas an artifact must satisfy, the
//! instructions that shape what a session writes, the producer skills and the
//! skill and definition of each agent (88, 91, 190).
//!
//! Data, versioned like anything else, compiled into the binary beside the
//! machines so it reaches every host with the release (91, D2). No line of it
//! exists in the engine: what is read here is a name, a version and a path,
//! never the text (119). Each file carries front matter — `name`, `kind`,
//! `path`, `version`, `set` — and its `path` is where it sits under
//! `flywheel/` in the blueprints, so nothing resolves by convention.

use anyhow::{bail, Context, Result};
use include_dir::{include_dir, Dir};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

/// `instructions/`, byte for byte, as the release shipped it.
static SET: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../../instructions");

/// One file of the set: its front matter and its text.
#[derive(Debug, Clone)]
pub struct InstructionFile {
    pub name: String,
    /// `schema`, `skill`, `instruction` or `agent`.
    pub kind: String,
    /// Where it sits under `flywheel/` in the blueprints (203).
    pub path: String,
    /// Moves when the text moves and at no other time (123).
    pub version: u32,
    /// The set version it was written against (224).
    pub set: u32,
    pub body: String,
}

impl InstructionFile {
    /// How a work order names an input: `name@version` (226).
    pub fn named(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct FrontMatter {
    name: String,
    kind: String,
    path: String,
    version: u32,
    set: u32,
}

/// What `set.yaml` carries: which default instructions (120) a session type is
/// handed when its type file names none.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Carries {
    #[serde(default, rename = "elaboration-types")]
    pub elaboration_types: Vec<String>,
    #[serde(default, rename = "unit-types")]
    pub unit_types: Vec<String>,
    #[serde(default, rename = "by-name")]
    pub by_name: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub none: NoDefaults,
}

/// The session types that carry no default instruction. The absence is a
/// decision, not an omission (`set.yaml` carries.none).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NoDefaults {
    #[serde(default)]
    pub sessions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct SetFile {
    version: u32,
    #[serde(default)]
    carries: Carries,
}

/// The set, loaded: every file by its blueprints path, and the set's version.
#[derive(Debug, Clone)]
pub struct Instructions {
    /// The release's instruction set version, which a work order's header
    /// names (226).
    pub version: u32,
    pub carries: Carries,
    files: BTreeMap<String, InstructionFile>,
}

impl Instructions {
    /// The set the binary carries.
    pub fn shipped() -> Result<Instructions> {
        let mut files = Vec::new();
        let mut set = None;
        collect(&SET, &mut files);
        let mut held = BTreeMap::new();
        for (path, bytes) in files {
            let text = std::str::from_utf8(bytes)
                .with_context(|| format!("the embedded instructions/{path} is not text"))?;
            take(&path, text, &mut set, &mut held)?;
        }
        finish(set, held)
    }

    /// The set in a directory, instead of the one the binary carries. What
    /// renders a prompt against an instruction version the release does not
    /// ship, so two versions can be told apart (123, 124).
    pub fn load_dir(dir: &Path) -> Result<Instructions> {
        let mut set = None;
        let mut held = BTreeMap::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for entry in std::fs::read_dir(&d)
                .with_context(|| format!("reading {}", d.display()))?
                .flatten()
            {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let relative = path
                    .strip_prefix(dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                let text = std::fs::read_to_string(&path)
                    .with_context(|| format!("reading {}", path.display()))?;
                take(&relative, &text, &mut set, &mut held)?;
            }
        }
        finish(set, held)
    }

    /// The file at a blueprints path, or none.
    pub fn at(&self, path: &str) -> Option<&InstructionFile> {
        self.files.get(path)
    }

    /// The file at a blueprints path, or an error naming it. A work order that
    /// names an input the set does not hold is not rendered at all: a session
    /// is given the versions in force and never a gap (88, 89).
    pub fn require(&self, path: &str) -> Result<&InstructionFile> {
        self.at(path)
            .with_context(|| format!("the instruction set holds no `{path}`"))
    }

    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.files.keys().map(|p| p.as_str())
    }
}

/// Read one file into the set, or note it as `set.yaml`.
fn take(
    relative: &str,
    text: &str,
    set: &mut Option<SetFile>,
    held: &mut BTreeMap<String, InstructionFile>,
) -> Result<()> {
    if relative == "set.yaml" {
        *set = Some(serde_yaml::from_str(text).context("parsing instructions/set.yaml")?);
        return Ok(());
    }
    // README.md is the directory's own prose and carries no front matter.
    if !text.starts_with("---\n") {
        return Ok(());
    }
    let Some(end) = text[4..].find("\n---\n") else {
        bail!("the front matter of instructions/{relative} is not closed");
    };
    let front: FrontMatter = serde_yaml::from_str(&text[4..4 + end])
        .with_context(|| format!("parsing the front matter of instructions/{relative}"))?;
    // The layout mirrors the blueprints prefix: cut `flywheel/` off the path
    // and what is left is where the file sits here. A file whose two disagree
    // is a fault the model's check reports; the loader refuses it rather than
    // resolving by convention.
    let expected = front.path.strip_prefix("flywheel/").unwrap_or(&front.path);
    if expected != relative {
        bail!(
            "instructions/{relative} says its path is `{}`, which is instructions/{expected}",
            front.path
        );
    }
    held.insert(
        front.path.clone(),
        InstructionFile {
            name: front.name,
            kind: front.kind,
            path: front.path,
            version: front.version,
            set: front.set,
            body: text[4 + end + 5..].to_string(),
        },
    );
    Ok(())
}

fn finish(set: Option<SetFile>, files: BTreeMap<String, InstructionFile>) -> Result<Instructions> {
    let set = set.context("the instruction set has no set.yaml")?;
    for file in files.values() {
        if file.set > set.version {
            bail!(
                "`{}` names set {} and the release's is {}",
                file.path,
                file.set,
                set.version
            );
        }
    }
    Ok(Instructions {
        version: set.version,
        carries: set.carries,
        files,
    })
}

fn collect(dir: &'static Dir<'static>, out: &mut Vec<(String, &'static [u8])>) {
    for file in dir.files() {
        out.push((file.path().to_string_lossy().to_string(), file.contents()));
    }
    for sub in dir.dirs() {
        collect(sub, out);
    }
}
