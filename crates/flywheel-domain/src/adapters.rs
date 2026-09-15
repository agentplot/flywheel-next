//! The adapters: the enumerator half of 215, which runs unattended (115).
//!
//! An adapter writes one keyed capture per source event with its provenance and
//! a pointer to the raw material, and does nothing else: it makes no judgment
//! about what the material says, starts no session, and reads nothing into
//! signals (111, 115, 215). Turning a capture into signals is curation's, and
//! never runs unattended — except where the capture is its own excerpt, which
//! the capture machine's own `ensure_signal` covers (19, 112).
//!
//! This phase ships (D13): the page's capture box and the chat forward, which
//! are the `capture` tool itself, and here the meeting transcript and the
//! signals folder — captures read before the instance existed, whose signals
//! are carried into the record format with no reader's judgment (114). The
//! pull-request and issue-tracker adapters read the git host's issues and
//! reviews, which C.2 forbids on this profile; the capture endpoint is
//! dispatch's and is phase 4 (215, 216).

use crate::signals::{self, Capture};
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use flywheel_atoms::{StateStore, World};
use flywheel_engine::Definitions;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// What one enumeration did. An adapter reports rather than decides: the
/// counts are what the run record carries and what a repeat import shows as
/// zero (111, 79).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Enumerated {
    /// The source-event keys this run saw.
    pub keys: Vec<String>,
    /// How many capture records it wrote. A repeat writes none (111).
    pub captures_written: usize,
    /// How many signal records it wrote. An enumerator writes none, since
    /// reading a capture into signals is a judgment (115), except a signals
    /// folder, whose signals were read before the instance existed (114).
    pub signals_written: usize,
    /// How many moves a signals folder's `moves.rec` carried in (107, 114).
    pub moves_written: usize,
}

/// The source name a meeting transcript is captured under.
pub const MEETING: &str = "meeting";

/// `flywheel capture meeting <file>`: one transcript, one capture (111, 215).
///
/// The key is `meeting/<date>/<name>`, read from the file's own name, so the
/// same transcript imported on two days yields one capture (111, S22). The
/// transcript itself stays where it is — the capture holds a pointer to it and
/// no repository holds a line of it (111).
pub fn meeting<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    path: &str,
    by: &str,
    at: DateTime<Utc>,
) -> Result<Enumerated> {
    let key = meeting_key(path)?;
    let capture = Capture {
        key: key.clone(),
        source: MEETING.into(),
        event_at: at.to_rfc3339(),
        captured_by: by.to_string(),
        // The pointer, and never the transcript: raw material stays outside
        // version control and the capture cites it (111).
        raw: path.to_string(),
    };
    let wrote = signals::write_capture(world, &capture)?;
    // The object the engine ticks, beside the record a person reads. A repeat
    // finds both and writes neither (111, 127).
    let id = signals::object_of(&key);
    if store.get(&id)?.is_none() {
        let record: BTreeMap<String, Value> = [
            ("source", json!(MEETING)),
            ("event_key", json!(key)),
            ("event_at", json!(at.to_rfc3339())),
            ("captured_by", json!(by)),
            ("raw", json!(path)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        crate::commands::put_new(store, defs, &id, "capture", None, record, at)?;
    }
    Ok(Enumerated {
        keys: vec![key],
        captures_written: usize::from(wrote),
        // The enumerator makes no judgment, so it reads nothing into signals: a
        // capture-reader session does, and only when the operator's curation
        // charges one (115).
        signals_written: 0,
        moves_written: 0,
    })
}

/// The source-event key a transcript's own name gives it:
/// `meeting/<date>/<name>` from `<date>-<name>.<ext>` (111,
/// `blueprints.yaml` adapters.meeting).
pub fn meeting_key(path: &str) -> Result<String> {
    let file = path.rsplit('/').next().unwrap_or(path);
    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
    // `2026-09-02-willdan-weekly` — the date is the first three parts and the
    // name is the rest.
    let parts: Vec<&str> = stem.splitn(4, '-').collect();
    let [year, month, day, name] = parts.as_slice() else {
        bail!(
            "`{path}` does not name a meeting; a transcript is named \
             `<date>-<name>.<ext>`, as `2026-09-02-willdan-weekly.vtt` is (111)"
        );
    };
    if year.len() != 4 || month.len() != 2 || day.len() != 2 {
        bail!("`{path}` does not begin with a date; a transcript is named `<date>-<name>.<ext>`");
    }
    Ok(format!("{MEETING}/{year}-{month}-{day}/{name}"))
}

/// Run one adapter by the command a person or a scenario names it with, so the
/// binary and the conformance runner drive one implementation (93, D15).
pub fn run<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    command: &str,
    by: &str,
    at: DateTime<Utc>,
) -> Result<Enumerated> {
    let words: Vec<&str> = command.split_whitespace().collect();
    match words.as_slice() {
        [.., "meeting", path] => meeting(store, world, defs, path, by, at),
        [.., "signals", dir] => signals_folder(store, world, defs, dir, by, at),
        _ => bail!(
            "`{command}` names no adapter this release ships; the meeting transcript is \
             `flywheel capture meeting <file>` and a folder of captures already read is \
             `flywheel capture signals <dir>` (215, D13)"
        ),
    }
}

// ------------------------------------------------------- the signals folder

/// The source-event prefix a signals folder's captures are keyed under.
pub const SIGNALS: &str = "signals";

/// `flywheel capture signals <dir>`: a folder of captures already read, in the
/// layout the willdan blueprints' `signals/` was written in, carried into the
/// record format as it is (114, 215).
///
/// One directory per capture holds a `capture.md` whose header names the source,
/// the event date and the raw pointer, and one markdown file per signal whose
/// header names the signal, its kind, who said it, its subjects and the claims
/// it argues with, with the assertion and the quoted excerpt below. Each is one
/// capture record and one signal record per file, keyed
/// `signals/<folder>/<capture directory>`, so importing the folder twice writes
/// nothing (111). Its signals were read before the instance existed, so no
/// reader is charged: the capture machine reads them present and is read at
/// once (114, 217e, `capture.yaml`). A `moves.rec` beside them carries a move by
/// one of the shipped six words; a move by any other word, `new-territory`
/// among them, leaves its signal unmoved for curation (107, 118), and a signal
/// that has a move keeps it, since only the operator's response replaces one.
pub fn signals_folder<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    dir: &str,
    by: &str,
    at: DateTime<Utc>,
) -> Result<Enumerated> {
    let root = std::path::Path::new(dir);
    let folder = folder_name(root).ok_or_else(|| anyhow!("`{dir}` names no folder of captures (114)"))?;
    let mut out = Enumerated::default();
    // A signal as the folder names it — `<capture directory>/<file stem>` —
    // and the object it is here, which is what a move in `moves.rec` names.
    let mut named: BTreeMap<String, String> = BTreeMap::new();
    for read in read_folder(root)? {
        let key = format!("{SIGNALS}/{folder}/{}", read.directory);
        out.keys.push(key.clone());
        let capture = Capture {
            key: key.clone(),
            source: read.source.clone(),
            event_at: read.event_date.clone(),
            captured_by: by.to_string(),
            // The pointer the folder gives, and never the material (111).
            raw: read.raw.clone(),
        };
        out.captures_written += usize::from(signals::write_capture(world, &capture)?);
        let id = signals::object_of(&key);
        if store.get(&id)?.is_none() {
            let record: BTreeMap<String, Value> = [
                ("source", json!(capture.source)),
                ("event_key", json!(key)),
                ("event_at", json!(capture.event_at)),
                ("captured_by", json!(capture.captured_by)),
                ("raw", json!(capture.raw)),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
            crate::commands::put_new(store, defs, &id, "capture", None, record, at)?;
        }
        for (at_file, held) in read.signals.iter().enumerate() {
            let ordinal = at_file as u64 + 1;
            let signal = signals::Signal {
                id: signals::signal_object(&key, ordinal),
                capture: id.clone(),
                kind: held.kind.clone(),
                asserted_by: held.who.clone(),
                subject_tags: held.subject.clone(),
                assertion: held.assertion.clone(),
                excerpt: held.excerpt.clone(),
                position: held.position.clone(),
                argues_with: held.claims.clone(),
            };
            named.insert(held.name.clone(), signal.id.clone());
            named.insert(format!("{}/{}", read.directory, held.stem), signal.id.clone());
            out.signals_written += usize::from(signals::write_signal(world, &key, ordinal, &signal)?);
            if store.get(&signal.id)?.is_none() {
                crate::commands::put_new(store, defs, &signal.id, "signal", Some(&id), signal.fields(), at)?;
            }
        }
    }
    let moves = root.join("moves.rec");
    if moves.is_file() {
        let text = std::fs::read_to_string(&moves).with_context(|| format!("reading {}", moves.display()))?;
        for record in flywheel_engine::rec::parse(&text) {
            let (Some(signal), Some(word)) = (record.get("Signal"), record.get("Move")) else {
                continue;
            };
            if !signals::MOVES.contains(&word) {
                continue;
            }
            let Some(object) = named.get(signal.trim()) else {
                continue;
            };
            if signals::standing_move(world, object)?.is_some() {
                continue;
            }
            let target = match record.get("Target").map(str::trim).filter(|t| !t.is_empty()) {
                Some(names) => format!("{word} {names}"),
                None => word.to_string(),
            };
            let moved = signals::Move {
                signal: object.clone(),
                target,
                reason: record.get("Reason").unwrap_or_default().to_string(),
                at: record.get("Date").unwrap_or_default().to_string(),
            };
            signals::apply_move(store, world, &moved, at)?;
            out.moves_written += 1;
        }
    }
    Ok(out)
}

/// Whether a signals folder holds a capture this instance has not read yet,
/// which is when its source is due (231, `host.yaml` host.adapters_due).
pub fn signals_due<W: World + ?Sized>(world: &W, dir: &str) -> bool {
    let root = std::path::Path::new(dir);
    let Some(folder) = folder_name(root) else {
        return false;
    };
    capture_directories(root).iter().any(|directory| {
        let key = format!("{SIGNALS}/{folder}/{directory}");
        signals::read_capture(world, &key).ok().flatten().is_none()
    })
}

fn folder_name(root: &std::path::Path) -> Option<String> {
    root.file_name().and_then(|n| n.to_str()).filter(|n| !n.is_empty()).map(String::from)
}

/// The directories of a folder that hold a `capture.md`, by name.
fn capture_directories(root: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return vec![];
    };
    let mut out: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.join("capture.md").is_file())
        .filter_map(|path| path.file_name().and_then(|n| n.to_str()).map(String::from))
        .collect();
    out.sort();
    out
}

/// One capture of a signals folder, as its files say it.
struct FolderCapture {
    directory: String,
    source: String,
    event_date: String,
    raw: String,
    signals: Vec<FolderSignal>,
}

/// One signal file of a capture, as its header and body say it.
struct FolderSignal {
    /// The signal's own name, from its header: `<capture>/<file stem>`.
    name: String,
    stem: String,
    kind: String,
    who: String,
    subject: Vec<String>,
    claims: Vec<String>,
    assertion: String,
    excerpt: String,
    position: String,
}

fn read_folder(root: &std::path::Path) -> Result<Vec<FolderCapture>> {
    if !root.is_dir() {
        bail!("`{}` is no folder of captures (114)", root.display());
    }
    let mut out = Vec::new();
    for directory in capture_directories(root) {
        let path = root.join(&directory);
        let text = std::fs::read_to_string(path.join("capture.md"))
            .with_context(|| format!("reading {}/capture.md", path.display()))?;
        let (head, _) = header(&text).ok_or_else(|| anyhow!("{}/capture.md has no header block", path.display()))?;
        let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&path)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|file| file.extension().is_some_and(|e| e == "md") && file.file_name().is_some_and(|n| n != "capture.md"))
            .collect();
        files.sort();
        let mut signals_read = Vec::new();
        for file in files {
            let text = std::fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
            let Some((head, body)) = header(&text) else {
                continue;
            };
            let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or_default().to_string();
            let (assertion, excerpt, position) = said(body);
            signals_read.push(FolderSignal {
                name: field(&head, "signal").unwrap_or_else(|| format!("{directory}/{stem}")),
                stem,
                kind: field(&head, "kind").unwrap_or_default(),
                who: field(&head, "who").unwrap_or_default(),
                subject: listed(&head, "subject"),
                claims: listed(&head, "claims"),
                assertion,
                excerpt,
                position,
            });
        }
        out.push(FolderCapture {
            source: field(&head, "source").unwrap_or_else(|| SIGNALS.to_string()),
            event_date: field(&head, "event_date").unwrap_or_default(),
            raw: field(&head, "raw").unwrap_or_default(),
            directory,
            signals: signals_read,
        });
    }
    Ok(out)
}

/// A markdown file's header block, and what follows it.
fn header(text: &str) -> Option<(serde_yaml::Value, &str)> {
    let rest = text.strip_prefix("---")?.trim_start_matches('\r').strip_prefix('\n')?;
    let end = rest.find("\n---")?;
    let head = serde_yaml::from_str(&rest[..end]).ok()?;
    let body = rest[end + 4..].trim_start_matches(['\r', '\n']);
    Some((head, body))
}

/// One header field as text, whatever the header wrote it as.
fn field(head: &serde_yaml::Value, name: &str) -> Option<String> {
    match head.get(name)? {
        serde_yaml::Value::String(text) => Some(text.trim().to_string()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
    .filter(|text| !text.is_empty())
}

/// One header field as a list of words, whether written as a list or a line.
fn listed(head: &serde_yaml::Value, name: &str) -> Vec<String> {
    match head.get(name) {
        Some(serde_yaml::Value::Sequence(items)) => items
            .iter()
            .filter_map(|item| match item {
                serde_yaml::Value::String(text) => Some(text.trim().to_string()),
                serde_yaml::Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .filter(|text| !text.is_empty())
            .collect(),
        Some(serde_yaml::Value::String(text)) => text.split(',').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect(),
        _ => vec![],
    }
}

/// What a signal file says under its header: the assertion before the first
/// quote, the quoted excerpts verbatim, and where each sits in the raw material
/// (`— lines 88-90`), or `whole` where none says (113).
fn said(body: &str) -> (String, String, String) {
    let mut assertion: Vec<&str> = Vec::new();
    let mut blocks: Vec<Vec<String>> = Vec::new();
    let mut quoting = false;
    for line in body.lines() {
        match line.trim_start().strip_prefix('>') {
            Some(quoted) => {
                if !quoting {
                    blocks.push(Vec::new());
                    quoting = true;
                }
                if let Some(block) = blocks.last_mut() {
                    block.push(quoted.trim().to_string());
                }
            }
            None => {
                quoting = false;
                if blocks.is_empty() {
                    assertion.push(line);
                }
            }
        }
    }
    let mut excerpts = Vec::new();
    let mut positions = Vec::new();
    for block in blocks {
        let joined = block.join(" ");
        match joined.rsplit_once(" — ") {
            Some((excerpt, position)) => {
                excerpts.push(excerpt.trim().to_string());
                positions.push(position.trim().to_string());
            }
            None => excerpts.push(joined.trim().to_string()),
        }
    }
    let assertion = assertion.join(" ").split_whitespace().collect::<Vec<_>>().join(" ");
    let position = match positions.is_empty() {
        true => "whole".to_string(),
        false => positions.join(", "),
    };
    (assertion, excerpts.join("\n\n"), position)
}
