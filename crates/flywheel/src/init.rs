//! `flywheel init`: the instance machine, driven (204).
//!
//! The command writes the manifest and then ticks the machine the binary
//! carries, performing each effect against the git host through
//! `flywheel-world-host` and reading each guard's evidence back from the world.
//! Nothing is a flag the machinery set: every step proves itself, so a second
//! run changes nothing and a half-finished bootstrap is finished by the next
//! tick (204). The App's installation is a decision under attention until it
//! is seen, and no agent ever makes it true (207, 207a, 82).

use anyhow::{Context, Result};
use flywheel_atoms::Records;
use flywheel_engine::runtime::Object;
use flywheel_scenario::{console, Runtime, Store};
use flywheel_world_host::bootstrap::Bootstrap;
use flywheel_world_host::effects::{self, exists};
use flywheel_world_host::manifest::Manifest;
use std::path::PathBuf;

/// What the operator asked for.
pub struct Init {
    pub instance: String,
    pub host: String,
    pub root: PathBuf,
    pub git_host: PathBuf,
    pub app: String,
    pub app_key_from: String,
    /// The key itself, where the caller has already read it from wherever the
    /// operator put it. With none it is read from the environment the manifest
    /// names, which is what the command line does (207, 207a).
    pub app_key: Option<String>,
    /// This host's one address: its name on the operator's private network,
    /// never a localhost port, because every link a delivery carries is
    /// written at it (191, 205a, D10a).
    pub address: String,
    pub manifest: PathBuf,
}

impl Init {
    /// This host's checkout of the state repository: the one place a host keeps
    /// it, `<root>/<instance>/flywheel-state` (205, 218).
    pub fn checkout(&self) -> PathBuf {
        self.root.join(&self.instance).join("flywheel-state")
    }
}

/// What it did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub lines: Vec<String>,
    /// The instance's state when the run settled.
    pub state: String,
    /// The effects this run performed. Empty on a second run (204).
    pub performed: Vec<String>,
    /// A decision standing under attention, if any (82).
    pub attention: Option<String>,
}

/// The instance object's id: one per instance, and the state repository holds
/// one instance's objects and no other's (218).
pub fn object_id(instance: &str) -> String {
    format!("instance/{instance}")
}

pub fn run(ask: Init) -> Result<Report> {
    let defs = flywheel_domain::set::load().context("the set the binary carries")?;
    let bootstrap = Bootstrap::new(&ask.git_host, ask.root.join(".flywheel-scratch"));

    // The manifest an earlier run left, or a fresh one. Reading it first is
    // what makes a second run a no-op and a half-finished one resumable (204).
    let mut manifest = if ask.manifest.is_file() {
        let text = std::fs::read_to_string(&ask.manifest)?;
        Manifest::check_repository_records(&text)?;
        Manifest::parse(&text)?
    } else {
        Manifest {
            instance: ask.instance.clone(),
            ..Default::default()
        }
    };
    manifest.instance = ask.instance.clone();

    // The record goes in the state repository, which is the only store this
    // release binds (92, 125). The instance's own is what this run is creating,
    // so until the machine's `create_state` has proved itself the record is
    // kept in a scratch repository under the host's root and moved into the
    // instance's own the moment it exists. Nothing is lost either way: every
    // step before that proves itself against the world and not against a
    // record (204).
    let mut store = Store::default();
    store.now = chrono::Utc::now();
    store.acting_host = Some(ask.host.clone());
    let checkout = ask.checkout();
    let scratch = ask.root.join(".flywheel-scratch").join("bootstrap-state");
    let mut bound_to_the_instances_own = false;
    if exists(&manifest.state.remote) {
        store.bind_state_repository_at(
            std::path::Path::new(&manifest.state.remote),
            &checkout,
            &ask.host,
        )?;
        bound_to_the_instances_own = true;
    } else {
        store.bind_state_repository(&scratch, &[ask.host.clone()])?;
    }

    let id = object_id(&ask.instance);
    if store.get(&id)?.is_none() {
        let record = [
            ("name".to_string(), serde_json::json!(ask.instance)),
            ("root".to_string(), serde_json::json!(ask.root)),
        ]
        .into_iter()
        .collect();
        let at = store.now;
        console::put_new(&mut store, &defs, &id, "instance", None, record, at)?;
    }

    let mut report = Report::default();
    let mut runtime = Runtime::new(defs, store);
    // The machines that declare one object per instance get theirs. `curation`
    // is one: with no object for it nothing ever reads the unmoved signals, so
    // a capture would land for ever and no intent would be proposed (A.15,
    // 110, `curation.yaml` singleton). Making one is idempotent — a second
    // init finds it and writes nothing (204).
    for name in singletons(&runtime.defs) {
        let id = format!("{name}/{}", ask.instance);
        if runtime.store.get(&id)?.is_some() {
            continue;
        }
        let at = runtime.store.now;
        let defs = runtime.defs.clone();
        console::put_new(&mut runtime.store, &defs, &id, &name, Some(&object_id(&ask.instance)), Default::default(), at)?;
        report.lines.push(format!("{id}: made"));
    }

    // Tick until the machine stops moving. Each pass reads the world again, so
    // an effect that already holds fires nothing.
    for _ in 0..12 {
        for (name, value) in effects::evidence(&manifest, ask.app_key.as_deref()) {
            runtime.store.set_given(&id, &name, value);
        }
        let before = runtime
            .store
            .get(&id)?
            .and_then(|o| o.top_state().map(String::from));
        let record = runtime.tick_settled();

        let mut moved = false;
        for effect in record.effects.iter().filter(|e| e.object == id) {
            let did = effects::perform(
                &effect.name,
                &mut manifest,
                &bootstrap,
                &ask.host,
                &ask.root,
                &ask.app,
                &ask.app_key_from,
                &ask.address,
            )?;
            if did {
                report.performed.push(effect.name.clone());
                report.lines.push(format!("{}: done", effect.name));
                moved = true;
            }
        }
        // The moment the instance's own state repository exists, the record
        // moves into it: what the machine wrote so far is seeded there, and
        // every write after this one is a commit on its shared line (204, C.2).
        if !bound_to_the_instances_own && exists(&manifest.state.remote) {
            runtime.store.durable.clear();
            runtime.store.bind_state_repository_at(
                std::path::Path::new(&manifest.state.remote),
                &checkout,
                &ask.host,
            )?;
            bound_to_the_instances_own = true;
        }
        let after = runtime
            .store
            .get(&id)?
            .and_then(|o| o.top_state().map(String::from));
        if !moved && before == after {
            break;
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);

    // A decision under attention stands until the operator acts; the machinery
    // never answers it for them (82, 207a).
    let standing = runtime.decisions();
    report.attention = standing
        .iter()
        .find(|d| d.object == id && d.group == "attention")
        .map(|d| d.kind.clone());
    report.state = runtime
        .store
        .get(&id)?
        .and_then(|o| o.top_state().map(String::from))
        .unwrap_or_default();

    manifest
        .template_version
        .get_or_insert_with(|| flywheel_domain::set::SET_VERSION.to_string());
    manifest.write(&ask.manifest)?;

    report.lines.push(format!(
        "instance {} · {} · set {}",
        ask.instance,
        report.state,
        manifest
            .template_version
            .clone()
            .unwrap_or_default()
    ));
    if let Some(kind) = &report.attention {
        report.lines.push(format!(
            "under attention: {kind} — the App's key is the operator's to place; no agent does it (207a)"
        ));
    }
    Ok(report)
}

/// Every machine that declares one object per instance, other than the
/// instance's own (`singleton: instance`).
fn singletons(defs: &flywheel_engine::Definitions) -> Vec<String> {
    let mut out: Vec<String> = defs
        .machines
        .iter()
        .filter(|(name, m)| {
            m.singleton.as_deref() == Some("instance")
                && !name.contains('@')
                && m.object.as_deref() != Some("instance")
                // The rail is the machinery's own, kept at one id the whole
                // instance shares and reached through `get` like any record
                // (`record-derived.yaml`, D5); it is not an object init makes.
                && name.as_str() != flywheel_domain::RAIL
        })
        .map(|(name, _)| name.clone())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The object as the store holds it, for a caller that wants to look.
pub fn instance(store: &impl Records, id: &str) -> Result<Option<Object>> {
    store.get(id)
}

/// Everything one flywheel reads or writes: its own root under the host, its
/// own state repository and its own prefix. A second flywheel beside a first
/// shares none of them (96, D14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scope {
    pub root: PathBuf,
    pub state: String,
    pub blueprints: String,
    pub prefix: String,
}

pub fn scope_of(manifest: &Manifest, host: &str) -> Result<Scope> {
    Ok(Scope {
        root: manifest.instance_root(host)?,
        state: manifest.state.remote.clone(),
        blueprints: manifest.blueprints.remote.clone(),
        prefix: flywheel_world_host::prefix::PREFIX.to_string(),
    })
}

impl Scope {
    /// Whether a path is this flywheel's to touch. Anything else belongs to
    /// someone, and it is not this one (96).
    pub fn covers(&self, path: &std::path::Path) -> bool {
        path.starts_with(&self.root)
            || path.starts_with(std::path::Path::new(&self.state))
            || path.starts_with(std::path::Path::new(&self.blueprints))
    }

    /// Whether two flywheels share anything at all.
    pub fn disjoint_from(&self, other: &Scope) -> bool {
        !self.root.starts_with(&other.root)
            && !other.root.starts_with(&self.root)
            && self.state != other.state
            && self.blueprints != other.blueprints
    }
}

/// The instance's removal, by the operator's response and never by deleting
/// files: sessions ended, places removed, the state archived under the root and
/// at the git host, and every git repository left where it was. The decision
/// counter is archived with the state, so no number is ever reused (221, 15).
pub fn remove(ask: &Init) -> Result<Removed> {
    let manifest = Manifest::read(&ask.manifest)?;
    let mut store = Store::default();
    store.acting_host = Some(ask.host.clone());
    store.bind_state_repository_at(
        std::path::Path::new(&manifest.state.remote),
        &ask.checkout(),
        &ask.host,
    )?;
    let counter = console::register(&store)?.next_number;

    let root = manifest.instance_root(&ask.host)?;
    let archive = root.join("archive");
    std::fs::create_dir_all(&archive)?;

    // The state is archived, counter and all; nothing of it is thrown away.
    // The records themselves are the state repository, which stays on disk with
    // every other repository (221) — what is written here is where to find them
    // and the number the register would have given next, so a flywheel made
    // after this one reuses none of its numbers (15).
    std::fs::write(
        archive.join("state"),
        format!("state repository {}\n", manifest.state.remote),
    )?;
    std::fs::write(
        archive.join("counter"),
        format!("next {counter}\n"),
    )?;

    // Every git repository stays on disk exactly where it was (221).
    let kept: Vec<String> = manifest
        .all_repositories()
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| std::path::Path::new(&remote_of(&manifest, name)).exists())
        .collect();

    // Sessions end and places go; in phase 1 the operator is the session and
    // the workspace is recorded, so what there is of either is what the store
    // holds (93a, 93b).
    store.world.sessions.clear();
    store.world.places.clear();

    Ok(Removed {
        archive,
        counter,
        repositories_left: kept,
    })
}

/// What a removal left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    pub archive: PathBuf,
    /// The number the register would give next. Archived with the state, so a
    /// flywheel made after this one reuses none of its numbers (15, 221).
    pub counter: u32,
    pub repositories_left: Vec<String>,
}

fn remote_of(manifest: &Manifest, name: &str) -> String {
    manifest
        .all_repositories()
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, r)| r.remote.clone())
        .unwrap_or_default()
}
