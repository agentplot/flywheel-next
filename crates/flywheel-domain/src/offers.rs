//! What a session offered, and the records the machinery makes of it
//! (`record-derived.yaml` record_offers, 58, 59, 62).
//!
//! A session reports a finding, a chore or a signal through `flywheel offer`,
//! which writes one entry on its thread pointing at a document. It is never
//! interrupted for it (58, I5). The machinery then makes one record per offer,
//! holding the path and the entry it came from and never the text (62): a
//! finding on the session's own intent is a proposed elaboration there, a
//! finding on its own bolt a proposed unit of the fast type, and any other
//! finding a signal citing the path. A signal is a signal citing the path
//! wherever the session stands, since it is about neither its intent nor its
//! bolt (58). A chore is a proposed chore unit on the line its scope names —
//! its bolt's, folding by the bolt, or a repository's shared line, folding by
//! the repository — and never a signal (60, S231).

use crate::{commands, report, signals};
use anyhow::{bail, Result};
use chrono::{DateTime, Utc};
use flywheel_atoms::{Object, Records, Scope, StateStore, ThreadEntry, World};
use flywheel_engine::Definitions;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The source a capture of an offer names: the session's offer is the event
/// (111).
pub const SOURCE: &str = "offer";

/// The scope naming the line of the bolt above the session, which an offer
/// under a bolt may leave off (`sessions.yaml` commands.offer).
pub const BOLT_LINE: &str = "bolt-line";

/// The scope naming the blueprints' shared line, where an instruction change
/// is a chore (123). It is a name the operator's `propose-chore` and `ask`
/// take beside the tracked repositories' (S229).
pub const BLUEPRINTS: &str = "blueprints";

/// Who a refusal the machinery writes on a session's thread is by.
const MACHINERY: &str = "flywheel";

/// Where offers' revisions are pinned on the git host: under the machinery's
/// own prefix, in the repository the offering place is in (62, 232,
/// `record-derived.yaml` record_offers).
pub const PINS: &str = "refs/flywheel/offers/";

/// The pin of an offer, from its entry `<session>#<n>`:
/// `refs/flywheel/offers/<session>/<n>`.
pub fn pin_of(entry: &str) -> String {
    let (session, index) = entry.rsplit_once('#').unwrap_or((entry, "0"));
    format!("{PINS}{session}/{index}")
}

/// The entry a pin was made for, the other way from `pin_of`.
pub fn entry_of_pin(reference: &str) -> Option<String> {
    let (session, index) = reference.strip_prefix(PINS)?.rsplit_once('/')?;
    Some(format!("{session}#{index}"))
}

/// A pin on the git host that stands for nothing any more: the record its
/// offer made has ended, or no record cites the offer and it is not pending
/// (55, 62, `host.yaml` host.no_stale_offer_pins).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StalePin {
    pub repository: String,
    pub reference: String,
    pub revision: String,
    /// The record that ended; none where the offer made nothing.
    pub record: Option<String>,
}

/// Every stale pin this world's clones hold. The pins are read first and the
/// records only when there is one, so a host with no pins reads nothing more.
pub fn stale_pins<S: Records, W: World + ?Sized>(store: &S, world: &W, defs: &Definitions) -> Result<Vec<StalePin>> {
    let mut pinned = Vec::new();
    for repository in world.repositories()? {
        if repository.name == "flywheel-state" {
            continue;
        }
        for (reference, revision) in world.pins(&repository.name, PINS)? {
            pinned.push((repository.name.clone(), reference, revision));
        }
    }
    if pinned.is_empty() {
        return Ok(vec![]);
    }
    let records = store.list_records(&Scope::All)?;
    let mut stale = Vec::new();
    for (repository, reference, revision) in pinned {
        let Some(entry) = entry_of_pin(&reference) else {
            continue;
        };
        let citing = records.iter().find(|object| {
            object
                .record
                .get("sources")
                .and_then(|v| v.as_array())
                .is_some_and(|sources| sources.iter().any(|v| v.as_str() == Some(entry.as_str())))
        });
        let record = match citing {
            Some(object) if ended(defs, object) => Some(object.id.clone()),
            Some(_) => continue,
            None => {
                let session = entry.rsplit_once('#').map(|(s, _)| s).unwrap_or(&entry);
                if pending(store, session)?.iter().any(|offer| offer.entry == entry) {
                    continue;
                }
                None
            }
        };
        stale.push(StalePin { repository, reference, revision, record });
    }
    Ok(stale)
}

/// Whether an object has ended: every top-level region of its machine it
/// stands in is at a final state.
fn ended(defs: &Definitions, object: &Object) -> bool {
    let Some(machine) = defs.for_object(&object.machine).or_else(|| defs.get(&object.machine)) else {
        return false;
    };
    let mut any = false;
    for (name, region) in &machine.regions {
        let Some(state) = object.config.get(name).and_then(|s| region.states.get(s)) else {
            continue;
        };
        if !state.is_final {
            return false;
        }
        any = true;
    }
    any
}

/// One offer, as the thread holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// `<session>#<index>`: the entry the record cites as its source.
    pub entry: String,
    pub kind: String,
    pub document: String,
    /// Where a chore's fix belongs: `bolt-line`, or a repository's manifest
    /// name. A finding's is not read (60).
    pub scope: Option<String>,
    /// What the offer concerns, as the session said it. Nothing is derived
    /// from it (58, 62).
    pub about: Option<String>,
    /// The offering place's head at the offer, which holds the document (62).
    pub revision: Option<String>,
    /// When the session made it, which is when it judged the document (62).
    pub at: DateTime<Utc>,
}

/// The offers on one session's thread, in the order they were made. A refused
/// report is not an offer, and neither is one a later refusal takes back: the
/// refusal stands in the record and nothing is made of it (60, 66, 80).
pub fn on_thread(session: &str, entries: &[ThreadEntry]) -> Vec<Offer> {
    let taken_back: BTreeSet<&str> = entries
        .iter()
        .filter(|entry| entry.kind == "offer")
        .filter_map(|entry| entry.fields.get("refuses").and_then(|v| v.as_str()))
        .collect();
    entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.kind == "offer" && !entry.fields.contains_key("refused"))
        .filter_map(|(at, entry)| {
            let id = format!("{session}#{at}");
            if taken_back.contains(id.as_str()) {
                return None;
            }
            Some(Offer {
                entry: id,
                kind: entry.fields.get("offer")?.as_str()?.to_string(),
                document: entry.fields.get("document")?.as_str()?.to_string(),
                scope: entry.fields.get("scope").and_then(|v| v.as_str()).map(String::from),
                about: entry.fields.get("about").and_then(|v| v.as_str()).map(String::from),
                revision: entry.fields.get("revision").and_then(|v| v.as_str()).map(String::from),
                at: entry.at,
            })
        })
        .collect()
}

/// Whether a record already points at this offer: the object carrying its
/// document, wherever the machinery put it. This is what makes an offer
/// recorded and what makes `record_offers` a no-op the second time (62, 127).
pub fn recorded<S: Records>(store: &S, offer: &Offer) -> Result<bool> {
    for object in store.list_records(&Scope::All)? {
        if object.record.get("document").and_then(|v| v.as_str()) == Some(offer.document.as_str()) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The offers on this session's thread that no record points at yet.
pub fn pending<S: Records>(store: &S, session: &str) -> Result<Vec<Offer>> {
    let mut out = Vec::new();
    for offer in on_thread(session, &store.thread(session)?) {
        if !recorded(store, &offer)? {
            out.push(offer);
        }
    }
    Ok(out)
}

/// What the machinery made of an offer, by the entry it came from: the record
/// citing it among its sources. A curation session's route names the offer it
/// made, and this is the unit that offer became (62, 116).
pub fn made_of<S: Records>(store: &S, entry: &str) -> Result<Option<String>> {
    if !entry.contains('#') {
        return Ok(None);
    }
    for object in store.list_records(&Scope::All)? {
        let cites = object
            .record
            .get("sources")
            .and_then(|v| v.as_array())
            .is_some_and(|sources| sources.iter().any(|v| v.as_str() == Some(entry)));
        if cites {
            return Ok(Some(object.id));
        }
    }
    Ok(None)
}

/// The object a session runs under, from the session's id alone: an id is
/// `<owner>/<stage or type>/<attempt>` (`session.yaml` id), so the owner is the
/// longest prefix of it that is an object on record. `flywheel offer` has the
/// session in hand and nothing else.
pub fn owner_of<S: Records>(store: &S, session: &str) -> Result<Option<String>> {
    let mut at = session.strip_prefix("session/").unwrap_or(session);
    while let Some((shorter, _)) = at.rsplit_once('/') {
        if store.get(shorter)?.is_some() {
            return Ok(Some(shorter.to_string()));
        }
        at = shorter;
    }
    Ok(None)
}

/// The built repositories the instance tracks, by manifest name: the state
/// and the blueprints are the machinery's own and are named apart (205, 206).
pub fn tracked<W: World + ?Sized>(world: &W) -> Result<Vec<String>> {
    Ok(world
        .repositories()?
        .into_iter()
        .map(|r| r.name)
        .filter(|name| name != "flywheel-state" && name != "flywheel-blueprints")
        .collect())
}

/// Why a chore offered from under `owner` with this scope has nowhere to land,
/// or nothing where it has (60, `sessions.yaml` commands.offer).
///
/// Under a bolt the scope is `bolt-line` and may be left off. Otherwise it is
/// a repository the instance tracks or the blueprints; anything else is
/// refused with those names, as an ask naming an untracked repository is. The
/// tracked names are read only when the scope needs them.
pub fn chore_refused<S: Records>(
    store: &S,
    owner: Option<&str>,
    scope: Option<&str>,
    tracked: impl FnOnce() -> Result<Vec<String>>,
) -> Result<Option<String>> {
    let scope = scope.map(str::trim).filter(|s| !s.is_empty());
    let bolt = match owner {
        Some(owner) => above(store, owner, "bolt")?,
        None => None,
    };
    if matches!(scope, None | Some(BOLT_LINE)) && bolt.is_some() {
        return Ok(None);
    }
    if scope == Some(BLUEPRINTS) {
        return Ok(None);
    }
    let tracked = tracked()?;
    if let Some(name) = scope.filter(|s| *s != BOLT_LINE) {
        if tracked.iter().any(|t| t == name) {
            return Ok(None);
        }
    }
    let names = tracked
        .iter()
        .map(String::as_str)
        .chain([BLUEPRINTS])
        .collect::<Vec<_>>()
        .join(", ");
    Ok(Some(match scope {
        Some(name) if name != BOLT_LINE => {
            format!("`{name}` is no repository this instance tracks; it tracks {names} (60)")
        }
        _ => format!(
            "a chore offered off every bolt names the repository whose shared line its fix \
             belongs on, with --scope: this instance tracks {names} (60)"
        ),
    }))
}

/// The object a session runs under, and the intent or bolt above it: what a
/// finding is about when a session offers one on its own thread (58).
fn above<S: Records>(store: &S, object: &str, machine: &str) -> Result<Option<String>> {
    let mut at = store.get(object)?;
    while let Some(held) = at {
        if held.machine == machine {
            return Ok(Some(held.id));
        }
        let Some(parent) = held.parent else { break };
        at = store.get(&parent)?;
    }
    Ok(None)
}

/// The id of a new object under a parent: an object's id is
/// `<machine>/<the parent's name>/<its own name>`, so an elaboration of
/// `intent/atlas-provider-limits` is `elaboration/atlas-provider-limits/...`
/// and a unit of `bolt/atlas/plan-rows` is `unit/atlas/plan-rows/...`.
fn id_under(parent: &str, machine: &str, name: &str) -> String {
    let stem = parent.split_once('/').map(|(_, rest)| rest).unwrap_or(parent);
    format!("{machine}/{stem}/{name}")
}

/// The next free ordinal for a family of ids sharing a prefix.
fn next<S: Records>(store: &S, prefix: &str) -> Result<u64> {
    let highest = store
        .list_records(&Scope::All)?
        .iter()
        .filter_map(|o| o.id.strip_prefix(prefix).and_then(|n| n.parse::<u64>().ok()))
        .max()
        .unwrap_or(0);
    Ok(highest + 1)
}

/// Make one record per uncited offer on this session's thread.
///
/// The session is the one that offered; `owner` is the object it runs under.
/// Every record holds the document path and the entry it came from; none holds
/// the text (62). A finding with nothing above it to take it is a signal, and a
/// chore with nowhere to land is refused on the thread, so every offer is dealt
/// with on the pass that finds it and the session reaches its exit.
pub fn record<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    session: &str,
    owner: &str,
    at: DateTime<Utc>,
) -> Result<Vec<String>> {
    let mut made = Vec::new();
    for offer in pending(store, session)? {
        if offer.kind == "chore" {
            // `flywheel offer` refuses a chore with nowhere to land before it
            // writes. A thread written some other way is refused here in the
            // same words, on the thread, and the offer is taken back (60, 80).
            let refused = chore_refused(&*store, Some(owner), offer.scope.as_deref(), || tracked(&*world))?;
            if let Some(reason) = refused {
                let taken_back = report::Report::Offer {
                    kind: offer.kind.clone(),
                    document: offer.document.clone(),
                    scope: offer.scope.clone(),
                    about: offer.about.clone(),
                    revision: offer.revision.clone(),
                };
                report::refuse_offer(store, session, MACHINERY, at, &taken_back, Some(&offer.entry), &reason)?;
                continue;
            }
        }

        // The document is read at the offer's revision on whichever host takes
        // up what the offer becomes, and a place's commits reach the git host
        // only when the place merges. So the revision is pinned there before a
        // record points at it, and a pin that fails leaves the offer pending
        // for the next pass (62, 232).
        if let Some(revision) = &offer.revision {
            let repository = crate::regions::repository_of(&*store, owner)?;
            world.pin(&repository, &pin_of(&offer.entry), revision)?;
        }

        let mut record: BTreeMap<String, Value> = BTreeMap::new();
        record.insert("document".into(), json!(offer.document));
        // Where the document is read, since the record never holds it (62).
        if let Some(revision) = &offer.revision {
            record.insert("revision".into(), json!(revision));
        }
        record.insert("sources".into(), json!([offer.entry]));

        if offer.kind == "chore" {
            made.push(chore(store, defs, owner, &offer, record, at)?);
            continue;
        }

        // A signal is about neither the session's intent nor its bolt, so
        // what stands above the session takes no part: it is never a proposal
        // on its thread, and a signal's scope is not read (58, 62, S231).
        if offer.kind == "signal" {
            made.push(as_signal(store, world, defs, session, owner, &offer, record, at)?);
            continue;
        }

        // A finding on the session's own intent is a proposed elaboration
        // there; on its own bolt, a proposed unit of the fast type (58, 59).
        if let Some(intent) = above(store, owner, "intent")? {
            let id = id_under(&intent, "elaboration", &format!("finding-{}", next(store, &id_under(&intent, "elaboration", "finding-"))?));
            record.insert("type".into(), json!("self-closing"));
            record.insert("type_version".into(), json!(2));
            commands::put_new(store, defs, &id, "elaboration", Some(&intent), record, at)?;
            made.push(id);
            continue;
        }
        if let Some(bolt) = above(store, owner, "bolt")? {
            let id = id_under(&bolt, "unit", &format!("finding-{}", next(store, &id_under(&bolt, "unit", "finding-"))?));
            record.insert("type".into(), json!("fast"));
            record.insert("type_version".into(), json!(3));
            record.insert("target".into(), json!(bolt));
            if let Some(repository) = repository_of(store, &bolt)? {
                record.insert("repository".into(), json!(repository));
            }
            commands::put_new(store, defs, &id, "unit", Some(&bolt), record, at)?;
            made.push(id);
            continue;
        }

        made.push(as_signal(store, world, defs, session, owner, &offer, record, at)?);
    }
    Ok(made)
}

/// A chore as a proposed chore unit on the line its scope names (60, 11).
fn chore<S: StateStore>(
    store: &mut S,
    defs: &Definitions,
    owner: &str,
    offer: &Offer,
    mut record: BTreeMap<String, Value>,
    at: DateTime<Utc>,
) -> Result<String> {
    record.insert("type".into(), json!("chore"));
    record.insert("type_version".into(), json!(2));
    let named = offer
        .scope
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != BOLT_LINE);
    match named {
        // On its bolt's line: a chore unit of the bolt, folded with the bolt's
        // others by the `batch` field the decision names (11).
        None => {
            let Some(bolt) = above(store, owner, "bolt")? else {
                bail!("a chore naming no repository lands on the bolt above its session, and none stands above `{owner}` (60)");
            };
            let id = id_under(&bolt, "unit", &format!("chore-{}", next(store, &id_under(&bolt, "unit", "chore-"))?));
            record.insert("batch".into(), json!(bolt));
            record.insert("scope".into(), json!(BOLT_LINE));
            if let Some(repository) = repository_of(store, &bolt)? {
                record.insert("repository".into(), json!(repository));
            }
            commands::put_new(store, defs, &id, "unit", Some(&bolt), record, at)?;
            Ok(id)
        }
        // On a repository's shared line: a chore unit under that repository,
        // or under the instance with the blueprints named as its repository.
        // Either folds by that name and names no bolt (60, 123, `unit.yaml`
        // parent).
        Some(name) => {
            let parent = match name {
                BLUEPRINTS => instance_of(store)?,
                name => Some(format!("repository/{name}")),
            };
            let prefix = format!("unit/{name}/chore-");
            let id = format!("{prefix}{}", next(store, &prefix)?);
            record.insert("batch".into(), json!(name));
            record.insert("repository".into(), json!(name));
            record.insert("scope".into(), json!("shared-line"));
            commands::put_new(store, defs, &id, "unit", parent.as_deref(), record, at)?;
            Ok(id)
        }
    }
}

/// A finding offered under neither an intent nor a bolt — a capture reader's,
/// a curation session's — is a signal citing the path, and so is an offer of
/// kind signal wherever the session stands (58, 62, 113).
///
/// A signal carries its capture (113). Under a capture it is that capture's
/// next signal; otherwise the offer is a capture of its own, the entry its
/// event and the document the pointer to its material (111). Nothing judged
/// it yet, so it asks, as a capture that is its own excerpt does, until
/// curation or the operator moves it (107, 116). What it asserts is where the
/// document is, and its excerpt stays empty: the record never holds the text
/// (62).
#[allow(clippy::too_many_arguments)]
fn as_signal<S: StateStore, W: World + ?Sized>(
    store: &mut S,
    world: &mut W,
    defs: &Definitions,
    session: &str,
    owner: &str,
    offer: &Offer,
    mut record: BTreeMap<String, Value>,
    at: DateTime<Utc>,
) -> Result<String> {
    let capture = match above(store, owner, "capture")? {
        Some(capture) => capture,
        None => {
            let index = offer.entry.rsplit_once('#').map(|(_, n)| n).unwrap_or_default();
            let key = format!("{SOURCE}/{}/{index}", session.trim_start_matches("session/"));
            let capture = signals::Capture {
                key: key.clone(),
                source: SOURCE.into(),
                event_at: offer.at.to_rfc3339(),
                captured_by: session.to_string(),
                raw: offer.document.clone(),
            };
            signals::write_capture(world, &capture)?;
            let id = signals::object_of(&key);
            if store.get(&id)?.is_none() {
                let fields: BTreeMap<String, Value> = [
                    ("source", json!(capture.source)),
                    ("event_key", json!(key)),
                    ("event_at", json!(capture.event_at)),
                    ("captured_by", json!(capture.captured_by)),
                    ("raw", json!(capture.raw)),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect();
                commands::put_new(store, defs, &id, "capture", None, fields, at)?;
            }
            id
        }
    };
    let key = signals::key_of_capture(store, &capture)?;
    let mut ordinal = 1;
    while world.read_file(signals::BLUEPRINTS, &signals::signal_path(&key, ordinal))?.is_some()
        || store.get(&signals::signal_object(&key, ordinal))?.is_some()
    {
        ordinal += 1;
    }
    let id = signals::signal_object(&key, ordinal);
    let signal = signals::Signal {
        id: id.clone(),
        capture: capture.clone(),
        kind: "ask".into(),
        asserted_by: session.to_string(),
        subject_tags: vec![],
        assertion: offer.document.clone(),
        excerpt: String::new(),
        position: "whole".into(),
        argues_with: vec![],
    };
    signals::write_signal(world, &key, ordinal, &signal)?;
    record.extend(signal.fields());
    commands::put_new(store, defs, &id, "signal", Some(&capture), record, at)?;
    Ok(id)
}

/// The instance object, which owns the chores of the blueprints' shared line
/// (123, `instance.yaml` owns).
fn instance_of<S: Records>(store: &S) -> Result<Option<String>> {
    Ok(store
        .list_records(&Scope::Machine("instance".into()))?
        .into_iter()
        .next()
        .map(|o| o.id))
}

fn repository_of<S: Records>(store: &S, object: &str) -> Result<Option<String>> {
    Ok(store
        .get(object)?
        .and_then(|o| o.record.get("repository").and_then(|v| v.as_str()).map(String::from)))
}
