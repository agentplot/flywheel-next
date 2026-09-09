//! Signals, and the one effect that writes a capture's own (113, 19, 112).
//!
//! A signal is immutable: it carries its kind, its asserter, its subject, its
//! assertion and the verbatim excerpt it came from, and nothing rewrites one
//! (113). `ensure_signal` is the capture machine's effect for a capture that is
//! its own excerpt — the page's box and a forwarded single message — where no
//! judgment is involved and the signal is the message (19, 112,
//! `capture.yaml` reading.captured).
//!
//! Turning any other capture into signals is curation's, and never runs
//! unattended (115). Nothing here does that.

use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use flywheel_atoms::{Scope, StateStore, World};
use flywheel_engine::{rec, Definitions};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Every signal object's id begins here.
pub const PREFIX: &str = "signal/";

// ------------------------------------------------ where the material lives

/// The repository the machinery writes its signal material into: the
/// instance's blueprints (203, `blueprints.yaml` layout).
pub const BLUEPRINTS: &str = "flywheel-blueprints";

/// The machinery's own prefix for it. Captures, signals and moves are files
/// here, under the prefix and nowhere else, because they are as much a
/// person's to read and write by hand as the machinery's (203, 110).
pub const UNDER: &str = "flywheel/signals";

/// The capture record's path, by its source-event key (111).
pub fn capture_path(key: &str) -> String {
    format!("{UNDER}/captures/{key}.rec")
}

/// Where one capture's signals live. `capture.signals_present` is whether
/// anything is here (`blueprints.yaml` evidence).
pub fn signals_under(key: &str) -> String {
    format!("{UNDER}/{key}/")
}

/// One signal's record, by the capture's key and the signal's ordinal.
pub fn signal_path(key: &str, ordinal: u64) -> String {
    format!("{UNDER}/{key}/{ordinal}.rec")
}

/// One signal's standing move, by the signal's id (107,
/// `blueprints.yaml` evidence.signal.move).
pub fn move_path(signal: &str) -> String {
    format!("{UNDER}/moves/{}.rec", signal.trim_start_matches(PREFIX))
}

/// The object id a source-event key is ticked under.
///
/// A key names its source first and then the event — `meeting/2026-09-02/
/// willdan-weekly` — and an object id names one object, so the separators are
/// flattened: `capture/meeting-2026-09-02-willdan-weekly` (S21, S22).
pub fn object_of(key: &str) -> String {
    format!("capture/{}", key.replace('/', "-"))
}

/// The signal object id for a capture's key and a signal's ordinal.
pub fn signal_object(key: &str, ordinal: u64) -> String {
    match ordinal {
        1 => format!("{PREFIX}{}", key.replace('/', "-")),
        n => format!("{PREFIX}{}-{n}", key.replace('/', "-")),
    }
}

/// The two names the layout keeps for itself, so a capture key cannot be
/// mistaken for one of them.
const RESERVED: &[&str] = &["captures", "moves"];

/// A source-event key the layout can hold: not empty, not reaching outside the
/// prefix, and not one of the two names the layout keeps (111, 203).
pub fn check_key(key: &str) -> Result<()> {
    let head = key.split('/').next().unwrap_or_default();
    if key.is_empty() || key.starts_with('/') || key.contains("..") {
        bail!("`{key}` is no source-event key; a key names one event (111)");
    }
    if RESERVED.contains(&head) {
        bail!(
            "`{key}` begins with `{head}`, which `{UNDER}/` keeps for itself; a source-event key \
             names its source first (111, 203)"
        );
    }
    Ok(())
}

// ------------------------------------------------------------ the formats

/// The capture record's format, versioned and stable: a record written under an
/// earlier version reads as it stands, with no migration step (114).
pub const CAPTURE_FORMAT: &str = "flywheel-capture/1";
/// The signal record's format (113, 114).
pub const SIGNAL_FORMAT: &str = "flywheel-signal/1";
/// The move record's format (107, 114).
pub const MOVE_FORMAT: &str = "flywheel-move/1";

/// One capture as the blueprints hold it: the unit of provenance (111).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Capture {
    /// The source-event key. Capturing the same event twice yields one capture.
    pub key: String,
    /// Which adapter or surface it came from.
    pub source: String,
    pub event_at: String,
    pub captured_by: String,
    /// The pointer to the raw material, which stays outside every repository
    /// (111).
    pub raw: String,
}

impl Capture {
    fn to_record(&self) -> rec::Record {
        let mut r = rec::Record {
            kind: Some("capture".into()),
            fields: vec![],
        };
        r.set("format", CAPTURE_FORMAT);
        r.set("key", &self.key);
        r.set("source", &self.source);
        r.set("event_at", &self.event_at);
        r.set("captured_by", &self.captured_by);
        // A pointer, never the material: raw transcripts and logs stay outside
        // version control and the capture cites them (111).
        r.set("raw", &self.raw);
        r
    }

    fn from_record(r: &rec::Record) -> Capture {
        // Every field is read as it stands. A record written under an earlier
        // version carries fewer of them and is read without conversion (114).
        Capture {
            key: r.get("key").unwrap_or_default().to_string(),
            source: r.get("source").unwrap_or_default().to_string(),
            event_at: r.get("event_at").unwrap_or_default().to_string(),
            captured_by: r.get("captured_by").unwrap_or_default().to_string(),
            raw: r.get("raw").unwrap_or_default().to_string(),
        }
    }

    /// The object id this capture is ticked under.
    pub fn object(&self) -> String {
        format!("capture/{}", self.key)
    }
}

/// Write one capture under the machinery's prefix in the blueprints, or find
/// the one that is already there (111, 203).
///
/// Capturing the same source event twice yields one capture: the second write
/// finds the record that exists, changes nothing, and says so by returning
/// false.
pub fn write_capture<W: World + ?Sized>(world: &mut W, capture: &Capture) -> Result<bool> {
    check_key(&capture.key)?;
    let path = capture_path(&capture.key);
    if world.read_file(BLUEPRINTS, &path)?.is_some() {
        return Ok(false);
    }
    let body = rec::write(std::slice::from_ref(&capture.to_record()));
    world.write_file(BLUEPRINTS, &path, body.as_bytes(), None)
}

/// The capture the blueprints hold for a source-event key, or none.
pub fn read_capture<W: World + ?Sized>(world: &W, key: &str) -> Result<Option<Capture>> {
    let Some(bytes) = world.read_file(BLUEPRINTS, &capture_path(key))? else {
        return Ok(None);
    };
    let text = String::from_utf8_lossy(&bytes).to_string();
    Ok(rec::parse(&text).first().map(Capture::from_record))
}

/// Every capture the blueprints hold, by key.
pub fn captures<W: World + ?Sized>(world: &W) -> Result<Vec<Capture>> {
    let mut out = Vec::new();
    for path in world.list_files(BLUEPRINTS, &format!("{UNDER}/captures/"))? {
        let Some(bytes) = world.read_file(BLUEPRINTS, &path)? else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes).to_string();
        if let Some(record) = rec::parse(&text).first() {
            out.push(Capture::from_record(record));
        }
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(out)
}

/// The signals a capture yielded, in the order `list` returns them. A signal
/// cites the capture it came from by being owned by it (`capture.yaml` owns).
pub fn of_capture<S: StateStore>(store: &S, capture: &str) -> Result<Vec<String>> {
    Ok(store
        .list_records(&Scope::All)?
        .into_iter()
        .filter(|o| o.machine == "signal" && o.parent.as_deref() == Some(capture))
        .map(|o| o.id)
        .collect())
}

/// `ensure_signal`: one capture, one signal, and never a second (19, 112,
/// `atoms.yaml` ensure_signal).
///
/// The signal's kind is `ask`, which is what a message with no judgment behind
/// it asserts; its excerpt is the capture's own raw material, verbatim (113).
/// The effect's proof is `capture.signals_present`, so running it again with
/// the signal already there changes nothing (127).
pub fn ensure_signal<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    capture: &str,
    by: &str,
    at: DateTime<Utc>,
) -> Result<Option<String>> {
    if !of_capture(store, capture)?.is_empty() {
        return Ok(None);
    }
    let held = store.get(capture)?;
    let field = |name: &str| {
        held.as_ref()
            .and_then(|o| o.record.get(name))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let key = held
        .as_ref()
        .and_then(|o| o.record.get("event_key"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| capture.trim_start_matches("capture/").to_string());
    let excerpt = field("raw");
    let asserter = match field("captured_by").is_empty() {
        true => by.to_string(),
        false => field("captured_by"),
    };
    let id = signal_object(&key, 1);
    let signal = Signal {
        id: id.clone(),
        capture: capture.to_string(),
        // A message with no judgment behind it asks; the kind an operator's
        // response gives it replaces this and nothing else does (113, 116).
        kind: "ask".into(),
        asserted_by: asserter.clone(),
        subject_tags: vec![],
        assertion: excerpt.clone(),
        excerpt: excerpt.clone(),
        // Its position in the raw material. A whole message is its own
        // excerpt, so it is the whole of it (113, S21).
        position: "whole".into(),
        argues_with: vec![],
    };
    // The record itself goes under the machinery's prefix in the blueprints,
    // where the capture's own is (113, 203); the object is what the engine
    // ticks.
    write_signal(world, &key, 1, &signal)?;
    let record: BTreeMap<String, Value> = signal
        .fields()
        .into_iter()
        .chain([("captured_at".to_string(), json!(at.to_rfc3339()))])
        .collect();
    crate::commands::put_new(store, defs, &id, "signal", Some(capture), record, at)?;
    Ok(Some(id))
}

/// One signal as the blueprints hold it (113).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Signal {
    pub id: String,
    /// The capture it came from, which it cites (113).
    pub capture: String,
    /// One of the small fixed set: constraint, ask, question, commitment,
    /// reaction (113).
    pub kind: String,
    pub asserted_by: String,
    pub subject_tags: Vec<String>,
    /// The assertion in a sentence.
    pub assertion: String,
    /// The verbatim excerpt, and where in the raw material it sits (113).
    pub excerpt: String,
    pub position: String,
    /// The claims it argues with, when any exist (113).
    pub argues_with: Vec<String>,
}

/// The kinds a signal may carry: a small fixed set, and nothing else (113).
pub const KINDS: &[&str] = &[
    "constraint",
    "ask",
    "question",
    "commitment",
    "reaction",
];

impl Signal {
    fn to_record(&self) -> rec::Record {
        let mut r = rec::Record {
            kind: Some("signal".into()),
            fields: vec![],
        };
        r.set("format", SIGNAL_FORMAT);
        r.set("id", &self.id);
        r.set("capture", &self.capture);
        r.set("kind", &self.kind);
        r.set("asserted_by", &self.asserted_by);
        r.set("subject_tags", &self.subject_tags.join(" "));
        r.set("assertion", &self.assertion);
        r.set("excerpt", &self.excerpt);
        r.set("position", &self.position);
        r.set("argues_with", &self.argues_with.join(" "));
        r
    }

    /// Read one, whatever version wrote it: every field is read as it stands
    /// and one that is absent is absent, with no migration step (114).
    pub fn from_record(r: &rec::Record) -> Signal {
        let words = |name: &str| -> Vec<String> {
            r.get(name)
                .unwrap_or_default()
                .split_whitespace()
                .map(String::from)
                .collect()
        };
        Signal {
            id: r.get("id").unwrap_or_default().to_string(),
            capture: r.get("capture").unwrap_or_default().to_string(),
            kind: r.get("kind").unwrap_or_default().to_string(),
            asserted_by: r.get("asserted_by").unwrap_or_default().to_string(),
            subject_tags: words("subject_tags"),
            assertion: r.get("assertion").unwrap_or_default().to_string(),
            excerpt: r.get("excerpt").unwrap_or_default().to_string(),
            position: r.get("position").unwrap_or_default().to_string(),
            argues_with: words("argues_with"),
        }
    }

    /// The same fields, as the object's record carries them.
    pub fn fields(&self) -> BTreeMap<String, Value> {
        [
            ("kind", json!(self.kind)),
            ("asserted_by", json!(self.asserted_by)),
            ("subject_tags", json!(self.subject_tags)),
            ("assertion", json!(self.assertion)),
            ("excerpt", json!(self.excerpt)),
            ("position", json!(self.position)),
            ("argues_with", json!(self.argues_with)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
    }
}

/// Write one signal under the capture's own directory in the blueprints, and
/// never rewrite one: a signal is immutable once written, and a correction is a
/// new signal or a change of move (113).
pub fn write_signal<W: World + ?Sized>(
    world: &mut W,
    key: &str,
    ordinal: u64,
    signal: &Signal,
) -> Result<bool> {
    check_key(key)?;
    let path = signal_path(key, ordinal);
    if world.read_file(BLUEPRINTS, &path)?.is_some() {
        return Ok(false);
    }
    let body = rec::write(std::slice::from_ref(&signal.to_record()));
    world.write_file(BLUEPRINTS, &path, body.as_bytes(), None)
}

/// Every signal record one capture yielded, in the order the blueprints hold
/// them. This is what `capture.signals_present` reads (`blueprints.yaml`).
pub fn signals_of<W: World + ?Sized>(world: &W, key: &str) -> Result<Vec<Signal>> {
    let mut out = Vec::new();
    let mut paths = world.list_files(BLUEPRINTS, &signals_under(key))?;
    paths.sort();
    for path in paths {
        let Some(bytes) = world.read_file(BLUEPRINTS, &path)? else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes).to_string();
        if let Some(record) = rec::parse(&text).first() {
            out.push(Signal::from_record(record));
        }
    }
    Ok(out)
}

// ---------------------------------------------------- reading the material

/// Where the signal material is read from: the blueprints on a host, the
/// scenario's own file map on the stand-in (`blueprints.yaml` evidence).
///
/// The evidence names below are read through this and nothing else, so they
/// answer the same on either — which is what makes the machines byte-identical
/// between bindings (139).
pub trait Reads {
    fn read(&self, path: &str) -> Option<String>;
    fn list(&self, under: &str) -> Vec<String>;
}

impl Reads for BTreeMap<String, String> {
    fn read(&self, path: &str) -> Option<String> {
        self.get(path).cloned()
    }

    fn list(&self, under: &str) -> Vec<String> {
        self.keys()
            .filter(|p| p.starts_with(under))
            .cloned()
            .collect()
    }
}

/// The blueprints as a world reports them.
pub struct Blueprints<'a, W: World + ?Sized>(pub &'a W);

impl<W: World + ?Sized> Reads for Blueprints<'_, W> {
    fn read(&self, path: &str) -> Option<String> {
        self.0
            .read_file(BLUEPRINTS, path)
            .ok()
            .flatten()
            .map(|b| String::from_utf8_lossy(&b).to_string())
    }

    fn list(&self, under: &str) -> Vec<String> {
        self.0.list_files(BLUEPRINTS, under).unwrap_or_default()
    }
}

/// The evidence names the signal material answers (`blueprints.yaml` evidence).
///
/// `None` means this is not a name the material answers; the record layer, the
/// world, the workspace and the sessions answer the rest.
pub fn evidence<R: Reads + ?Sized>(files: &R, object: &str, name: &str) -> Option<Value> {
    Some(match name {
        // At least one signal record under the capture's own directory (111).
        //
        // Only for a capture the material knows about: one written before this
        // layout existed is not read as having no signals, it is not read here
        // at all, and the next source answers for it.
        "capture.signals_present" => {
            let key = key_of(object);
            if files.read(&capture_path(&key)).is_none() {
                return None;
            }
            json!(!files.list(&signals_under(&key)).is_empty())
        }
        // The move's first word, or none where the file is absent or cleared
        // (107, S24). A signal the material does not hold is answered for
        // elsewhere.
        "signal.move" => {
            let Some(text) = files.read(&move_path(object)) else {
                return None;
            };
            match rec::parse(&text).first().map(Move::from_record) {
                Some(moved) if !moved.target.is_empty() => json!(moved.word()),
                _ => json!("none"),
            }
        }
        // Signal records with no move record, which is what curation runs over
        // (110, 118). With no material at all there is nothing to count here
        // and the next source answers.
        "curation.unmoved_count" => {
            let held = files.list(&format!("{UNDER}/"));
            if held.is_empty() {
                return None;
            }
            json!(unmoved(files).len())
        }
        _ => return None,
    })
}

/// The source-event key a capture object's id names: the object id with its
/// separators flattened, read back through the record it points at.
fn key_of(object: &str) -> String {
    let flat = object.strip_prefix("capture/").unwrap_or(object);
    // The key is the flattened id with its source restored: `meeting-2026-...`
    // is `meeting/2026-...`. The capture record carries the key outright, so
    // this is only how the path is found before it is read.
    match flat.split_once('-') {
        Some((source, rest)) => format!("{source}/{}", rest.replacen('-', "/", 1)),
        None => flat.to_string(),
    }
}

/// Every signal with no standing move: what curation sees, and what the status
/// view counts and ages by source (107, 118).
pub fn unmoved<R: Reads + ?Sized>(files: &R) -> Vec<Signal> {
    let mut out = Vec::new();
    let mut paths = files.list(&format!("{UNDER}/"));
    paths.sort();
    for path in paths {
        let rest = path.trim_start_matches(&format!("{UNDER}/")).to_string();
        if RESERVED.contains(&rest.split('/').next().unwrap_or_default()) {
            continue;
        }
        let Some(text) = files.read(&path) else {
            continue;
        };
        let Some(record) = rec::parse(&text).first().map(Signal::from_record) else {
            continue;
        };
        if record.id.is_empty() {
            continue;
        }
        let moved = files
            .read(&move_path(&record.id))
            .and_then(|t| rec::parse(&t).first().map(Move::from_record))
            .is_some_and(|m| !m.target.is_empty());
        if !moved {
            out.push(record);
        }
    }
    out
}

// -------------------------------------------------------------- the moves

/// The moves a signal may take, and no others (107, 116).
pub const MOVES: &[&str] = &[
    "attach",
    "challenge",
    "join",
    "answered",
    "route",
    "drop",
];

/// One signal's standing move: what became of it, why, and when (107).
///
/// Every signal has exactly one, and only the operator's response replaces one.
/// The target's first word is the move; what follows names what it moved to —
/// the intent it attached to, the claim it challenges, the offer a route names
/// (`blueprints.yaml` evidence.signal.move).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Move {
    pub signal: String,
    /// `attach <intent>`, `challenge <claim>@<version>`, `join <intent>`,
    /// `answered <claim>`, `route <offer entry id>`, `drop`.
    pub target: String,
    pub reason: String,
    pub at: String,
}

impl Move {
    /// The move itself: the target's first word (107).
    pub fn word(&self) -> &str {
        self.target.split_whitespace().next().unwrap_or_default()
    }

    /// What the move names: the intent, the claim, the offer. Empty for a drop.
    pub fn names(&self) -> &str {
        self.target
            .split_once(char::is_whitespace)
            .map(|(_, rest)| rest.trim())
            .unwrap_or_default()
    }

    fn to_record(&self) -> rec::Record {
        let mut r = rec::Record {
            kind: Some("move".into()),
            fields: vec![],
        };
        r.set("format", MOVE_FORMAT);
        r.set("signal", &self.signal);
        r.set("target", &self.target);
        r.set("reason", &self.reason);
        r.set("at", &self.at);
        r
    }

    /// Read one, whatever version wrote it (114).
    pub fn from_record(r: &rec::Record) -> Move {
        Move {
            signal: r.get("signal").unwrap_or_default().to_string(),
            target: r.get("target").unwrap_or_default().to_string(),
            reason: r.get("reason").unwrap_or_default().to_string(),
            at: r.get("at").unwrap_or_default().to_string(),
        }
    }
}

/// Write one signal's standing move, replacing whatever stood there (107).
///
/// One move per signal, and the file's own existence is the move: curation sees
/// only signals with no file here, and the operator's response is what replaces
/// or clears one.
pub fn write_move<W: World + ?Sized>(world: &mut W, moved: &Move) -> Result<bool> {
    let word = moved.word();
    if !MOVES.contains(&word) {
        bail!(
            "`{}` is no move; a signal takes one of {} (107)",
            moved.target,
            MOVES.join(", ")
        );
    }
    let body = rec::write(std::slice::from_ref(&moved.to_record()));
    world.write_file(BLUEPRINTS, &move_path(&moved.signal), body.as_bytes(), None)
}

/// The move standing on one signal, or none — which is what makes it unmoved
/// (107, `blueprints.yaml` evidence.signal.move).
pub fn standing_move<W: World + ?Sized>(world: &W, signal: &str) -> Result<Option<Move>> {
    let Some(bytes) = world.read_file(BLUEPRINTS, &move_path(signal))? else {
        return Ok(None);
    };
    let text = String::from_utf8_lossy(&bytes).to_string();
    Ok(rec::parse(&text).first().map(Move::from_record))
}

/// Clear a signal's move, which is what `revive` does: the drop is removed, the
/// signal is unmoved again, and the next curation run clusters it (107, S24).
pub fn clear_move<W: World + ?Sized>(world: &mut W, signal: &str) -> Result<()> {
    world.write_file(BLUEPRINTS, &move_path(signal), b"", None)?;
    Ok(())
}

/// What applying a move did, so the run record can say it and a caller can read
/// it back (116, 79).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Applied {
    pub moved: Move,
    /// The object the move named — the intent it attached to or joined, the
    /// record an answer names, the offer a route names.
    pub target: Option<String>,
    /// For a challenge: the claim it argues with, by name and version (116).
    pub claim: Option<String>,
    pub claim_version: Option<u32>,
}

/// A claim named as `<name>@<version>`, which is how a challenge records the
/// one it argues with (116, 101).
pub fn claim_named(named: &str) -> (String, Option<u32>) {
    match named.rsplit_once('@') {
        Some((name, version)) => match version.parse::<u32>() {
            Ok(v) => (name.to_string(), Some(v)),
            Err(_) => (named.to_string(), None),
        },
        None => (named.to_string(), None),
    }
}

/// Apply one move and its stated consequence (107, 116).
///
/// Every move has one. `attach` and `join` put the signal on the intent the
/// move names, as evidence and as material; `challenge` records the claim it
/// argues with, by name and version, on the signal; `answered` and `route`
/// record what settled it or what was offered for it; `drop` is the move and
/// its reason and nothing more.
///
/// A challenge attempts no ledger effect. The consequence 101 states — every
/// cell of that claim falling stale — belongs to the phase that has a ledger,
/// and recording the claim here is what lets that phase find them (116, 101,
/// A.14 is phase 3).
pub fn apply_move<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    moved: &Move,
    at: DateTime<Utc>,
) -> Result<Applied> {
    write_move(world, moved)?;
    let named = moved.names().to_string();
    let mut applied = Applied {
        moved: moved.clone(),
        ..Default::default()
    };
    match moved.word() {
        // Evidence on an open intent, and material on a proposed one: either
        // way the intent cites the signal and says how many it has (109, 116).
        "attach" | "join" if !named.is_empty() => {
            cite(store, &named, &moved.signal, at)?;
            applied.target = Some(named);
        }
        // The claim by name and version, on the signal that argues with it.
        // Nothing of the ledger is read or written (116, 101).
        "challenge" if !named.is_empty() => {
            let (name, version) = claim_named(&named);
            set(store, &moved.signal, "argues_with", json!([named.clone()]))?;
            applied.claim = Some(name);
            applied.claim_version = version;
            applied.target = Some(named);
        }
        // What settled it, and what was offered for it (116).
        "answered" | "route" if !named.is_empty() => {
            set(store, &moved.signal, moved.word(), json!(named.clone()))?;
            applied.target = Some(named);
        }
        _ => {}
    }
    // The move and its reason are on the signal too, so the object the engine
    // ticks says what became of it without a second read (107).
    set(store, &moved.signal, "move", json!({
        "target": moved.target,
        "reason": moved.reason,
        "at": moved.at,
    }))?;
    Ok(applied)
}

/// Put a signal on the intent that cites it, and say how many it has (109).
fn cite<S: StateStore>(
    store: &mut S,
    intent: &str,
    signal: &str,
    at: DateTime<Utc>,
) -> Result<()> {
    let Some(mut object) = store.get(intent)? else {
        // The intent a join names may not exist yet: `propose_intents` makes
        // it, and the move stands either way (110, 116).
        let _ = at;
        return Ok(());
    };
    let mut cited: Vec<String> = object
        .record
        .get("signals")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    if !cited.iter().any(|s| s == signal) {
        cited.push(signal.to_string());
    }
    let base = object.seq;
    object.record.insert("signals_count".into(), json!(cited.len()));
    object.record.insert("signals".into(), json!(cited));
    store.put(intent, &object, base)?;
    Ok(())
}

/// One field on an object's record, left as it was where the object is not
/// there.
fn set<S: StateStore>(store: &mut S, id: &str, name: &str, value: Value) -> Result<()> {
    let Some(mut object) = store.get(id)? else {
        return Ok(());
    };
    let base = object.seq;
    object.record.insert(name.to_string(), value);
    store.put(id, &object, base)?;
    Ok(())
}

/// Every move the blueprints hold, by signal.
pub fn moves<W: World + ?Sized>(world: &W) -> Result<Vec<Move>> {
    let mut out = Vec::new();
    let mut paths = world.list_files(BLUEPRINTS, &format!("{UNDER}/moves/"))?;
    paths.sort();
    for path in paths {
        let Some(bytes) = world.read_file(BLUEPRINTS, &path)? else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes).to_string();
        // A cleared move is an empty file: the signal is unmoved (107, S24).
        if let Some(record) = rec::parse(&text).first() {
            let moved = Move::from_record(record);
            if !moved.target.is_empty() {
                out.push(moved);
            }
        }
    }
    Ok(out)
}

/// Every signal record the blueprints hold, whatever capture it came from.
pub fn all_signals<W: World + ?Sized>(world: &W) -> Result<Vec<Signal>> {
    let mut out = Vec::new();
    let mut paths = world.list_files(BLUEPRINTS, &format!("{UNDER}/"))?;
    paths.sort();
    for path in paths {
        // The two directories the layout keeps for itself are not signals.
        let rest = path.trim_start_matches(&format!("{UNDER}/")).to_string();
        let head = rest.split('/').next().unwrap_or_default();
        if RESERVED.contains(&head) {
            continue;
        }
        let Some(bytes) = world.read_file(BLUEPRINTS, &path)? else {
            continue;
        };
        let text = String::from_utf8_lossy(&bytes).to_string();
        if let Some(record) = rec::parse(&text).first() {
            out.push(Signal::from_record(record));
        }
    }
    Ok(out)
}
