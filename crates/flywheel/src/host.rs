//! `flywheel host`: one long-lived process that runs the loop for one instance
//! (D7, 231).
//!
//! A tick fetches and integrates the shared line first, so no host decides on a
//! read older than the bound and no person runs the sync by hand (165). A
//! notify ticks the object that moved and its parent chain; a sweep every 60
//! seconds covers everything, which is what makes `older:` guards fire and what
//! makes a never-notified host converge by reading alone (130, D6). Nothing the
//! process holds in memory decides anything: every state it acts on is
//! derivable from what `read` and `list` return (136, 75, I14).
//!
//! What it binds is what the manifest names and nothing else (D8): the world,
//! the workspace and the sessions, each one implementation in this release,
//! each recorded in the run record so the bytes that ran are provable (139).

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Duration, Utc};
use flywheel_atoms::{
    EffectWrite, LandingPolicy, LeaseOp, LeaseOutcome, LeaseRecord, Listing, Notice, Object,
    Presentation, PutOutcome, ReadPoint, Received, Records, Scope, SessionPresence, StateStore,
    StatusView, ThreadEntry, WorkOrder, Workspace, World, WriteOutcome,
};
use flywheel_domain::derived::{Declaration, Reading};
use flywheel_domain::records::RunEntry;
use flywheel_domain::regions::{place_key, session_stem};
use flywheel_engine::runtime::{EvidenceSource, Response};
use flywheel_engine::{Definitions, PlannedEffect};
use flywheel_scenario::console;
use flywheel_surface::chat::{Chat, Channel};
use flywheel_store_git::GitStore;
use flywheel_workspace_recorded::RecordedWorkspace;
use flywheel_world_host::{HostWorld, Manifest};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// How often the sweep runs, whatever else happens (D7, 130). The default the
/// model states; `flywheel.yaml`'s `intervals.sweep` is what a host actually
/// runs at, and a test's is milliseconds.
pub const SWEEP: f64 = 60.0;

/// How often a laptop asks the git host whether anything moved: one round trip,
/// no history read (D6). The default the model states; `intervals.poll` is the
/// setting behind it.
pub const POLL: f64 = 30.0;

/// How many passes one local cause may cascade over before the loop looks
/// outward again. A cascade inside one host — an effect that moves another
/// object — is a local cause and is ticked at once rather than waited out
/// (model.md 2.1 causes, 130, D6); the bound is here so a machine that never
/// settles cannot hold the loop, and a pass that fires nothing ends the burst
/// long before it.
pub const BURST: usize = 64;

/// How long the loop stands aside between the links of a cascade, so a page
/// request waiting on the same host is answered while the chain runs (D11).
pub const BETWEEN: std::time::Duration = std::time::Duration::from_millis(1);

/// How many passes one sweep settles over before it gives up and leaves the
/// rest to the next one. A pass that moved nothing ends it long before this,
/// and a chain that has not settled in this many has more to read than one
/// sweep decides: the next sweep carries it (D7, model.md the tick).
pub const PASSES: usize = 6;

/// The three bindings a host loads by name, and the one implementation this
/// release carries of each (D8, 139).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bindings {
    pub world: String,
    pub workspace: String,
    pub sessions: String,
}

impl Bindings {
    /// What the manifest names for this host. A name this binary has no
    /// implementation of is refused, and the refusal says which phase carries
    /// it: a host never branches on a binding it cannot load (D8, 299).
    pub fn read(manifest: &Manifest, host: &str) -> Result<Bindings> {
        let entry = manifest.host(host)?;
        let bindings = Bindings {
            world: "host".into(),
            workspace: entry.workspace.clone(),
            sessions: entry.sessions.clone(),
        };
        if !matches!(bindings.workspace.as_str(), "recorded" | "host") {
            bail!(
                "flywheel.yaml hosts.{host}.workspace: `{}` — this release binds `recorded` \
                 (the facts alone) or `host` (branches and worktrees under the host's root) \
                 (93a, D8)",
                bindings.workspace
            );
        }
        if !matches!(bindings.sessions.as_str(), "operator" | "herdr") {
            bail!(
                "flywheel.yaml hosts.{host}.sessions: `{}` — this release binds `operator` (the \
                 session is the operator's to run) or `herdr` (the agent in a pane of the \
                 multiplexer) (93b, 217c, D8)",
                bindings.sessions
            );
        }
        Ok(bindings)
    }

    /// The three names, as the run record carries them.
    pub fn named(&self) -> Vec<(String, String)> {
        vec![
            ("world".into(), self.world.clone()),
            ("workspace".into(), self.workspace.clone()),
            ("sessions".into(), self.sessions.clone()),
        ]
    }
}

/// The state store a host runs on: the git-only profile's, with the evidence
/// the recorded workspace and the operator's sessions answer layered over the
/// record-derived half (D8, `record-derived.yaml`).
pub struct HostStore {
    pub git: GitStore,
    /// The world this host loaded (D8): the repositories, the manifest, the
    /// router and the App's tokens. The blueprints are among them, which is
    /// where the captures, signals and moves live, under the machinery's
    /// prefix — so the evidence about them is read through here (111, 203,
    /// `blueprints.yaml` evidence).
    pub world: Box<dyn World + Send>,
    pub reading: Reading,
    /// Every operation this store was asked for, in order. The run record and
    /// the fetch-first proof read it (79, 165).
    pub trace: RefCell<Vec<String>>,
    /// A lease an effect moved out of band — the lease of a released host,
    /// expired at once by the operator's response (150, `lease.yaml` stale).
    /// The machine never takes that transition, because the effect is what
    /// makes it; the run record is where the host says so (79, 167).
    pub marked: RefCell<Vec<(String, String, String)>>,
    /// What the world reports about the instance — its repositories, its App,
    /// its hosts — read from the manifest this host opened, so the instance
    /// machine's guards are answered on a running host and not only under
    /// `flywheel init` (204, 207, 221; `host.yaml` instance evidence).
    pub instance: BTreeMap<String, Value>,
    /// The definitions the host runs, for the evidence that reads a type's
    /// own parameters — a stage's agents and bound (56, `host.yaml`
    /// stage.agents). None until a host is over the store.
    pub defs: Option<Definitions>,
    /// Which line-and-place binding performs the effects of 42: `recorded`
    /// writes the facts alone, `host` makes real branches and worktrees under
    /// `root` and writes the same facts (93a, D8).
    pub workspace: String,
    /// `<root>/<instance>/`, where this host's clones and worktrees live;
    /// none for a host over a store with no world on disk.
    pub root: Option<std::path::PathBuf>,
    /// What starts a session: `operator` records it for the operator to run,
    /// `herdr` starts the agent in a pane of the multiplexer (93b, 217c, D8).
    pub sessions: String,
    /// The manifest the host was opened on, which a work order hands to the
    /// session commands that read it (`flywheel ask`, 183).
    pub manifest: Option<std::path::PathBuf>,
    /// The signal material as this tick read it, in one pass: every signal's
    /// evidence is answered from it, and it is read again at the next tick or
    /// once an effect may have written (111, 203).
    pub material: RefCell<Option<BTreeMap<String, String>>>,
}

impl HostStore {
    pub fn new(git: GitStore, me: &str, now: DateTime<Utc>) -> HostStore {
        HostStore {
            reading: Reading::new(me, now),
            git,
            // A world with no repositories on disk until `Host::open` binds the
            // manifest's (D1, D8).
            world: Box::new(flywheel_scenario::bindings::FilesWorld::new()),
            trace: RefCell::new(vec![]),
            marked: RefCell::new(vec![]),
            instance: BTreeMap::new(),
            defs: None,
            workspace: "recorded".into(),
            root: None,
            sessions: "operator".into(),
            manifest: None,
            material: RefCell::new(None),
        }
    }

    fn saw(&self, what: &str) {
        self.trace.borrow_mut().push(what.to_string());
    }

    /// The signal material, read once and answered from until it is taken
    /// back (111, 203).
    pub fn material(&self) -> std::cell::Ref<'_, BTreeMap<String, String>> {
        if self.material.borrow().is_none() {
            let read = flywheel_domain::signals::snapshot(&*self.world);
            *self.material.borrow_mut() = Some(read);
        }
        std::cell::Ref::map(self.material.borrow(), |held| held.as_ref().expect("the material was just read"))
    }

    pub fn since(&self) -> Vec<String> {
        self.trace.borrow().clone()
    }

    /// The store and the world it loaded, for a caller that needs both at
    /// once: a capture writes one record to each — the object here and the
    /// material under the machinery's prefix in the blueprints (111, 203).
    ///
    /// The world is taken out for the call and put back, so it lives in one
    /// place and is borrowed from one.
    pub fn with_world<T>(&mut self, act: impl FnOnce(&mut HostStore, &mut dyn World) -> T) -> T {
        let mut world: Box<dyn World + Send> =
            std::mem::replace(&mut self.world, Box::new(flywheel_scenario::bindings::FilesWorld::new()));
        let out = act(self, &mut *world);
        self.world = world;
        out
    }

    /// The sources this host declares enumerators for, from its own host
    /// record (111, 215, 231, `host.yaml` host.adapters_run).
    ///
    /// The shipped adapters are the page's box, the chat forward and the
    /// meeting transcript (D13); only the last is an enumerator a tick runs,
    /// and it is named by the file it reads. A host that declares none runs
    /// none, which is what a fresh instance is.
    pub fn sources_declared(&self, host: &str) -> Vec<String> {
        self.git
            .get(&format!("host/{host}"))
            .ok()
            .flatten()
            .and_then(|o| o.record.get("sources").cloned())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    }

    /// Carry every refusal the session wrote to the run record, where the
    /// report effect beside it takes it to attention (43, 79,
    /// `record-derived.yaml` record_refusals).
    ///
    /// The entries are the session's own, written by `flywheel refuse` or by
    /// the tool server refusing a call; nothing here judges one.
    pub fn refuse_from_session(&self, session: &str, host: &str, now: DateTime<Utc>) {
        for entry in self.git.thread(session).unwrap_or_default() {
            if entry.kind != "refusal" {
                continue;
            }
            self.marked.borrow_mut().push((
                session.to_string(),
                format!("{host}/{}", now.to_rfc3339()),
                entry
                    .fields
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("a session was refused")
                    .to_string(),
            ));
        }
    }

    /// The type an object's sessions are named for, where it has one
    /// (`session.yaml` id).
    fn kind_of(&self, object: &str) -> Option<String> {
        self.git
            .get(object)
            .ok()
            .flatten()?
            .record
            .get("type")
            .and_then(|v| v.as_str())
            .map(String::from)
    }
}

impl Records for HostStore {
    fn get(&self, id: &str) -> Result<Option<Object>> {
        self.saw("get");
        self.git.get(id)
    }

    fn put(&mut self, id: &str, record: &Object, base_seq: u64) -> Result<PutOutcome> {
        self.saw("put");
        // The rail's own record — the register and the counter — belongs to
        // every host of the instance, and a host with no route holds no
        // opinion about it: a number it gave locally would collide with the
        // one another host gave, and a commit on it would meet theirs on the
        // rebase home (15, 151, 161, D4a). Its own work is committed as
        // always; this is the one record it leaves alone.
        if self.git.disconnected && id == flywheel_domain::RAIL {
            return Ok(PutOutcome::Written { seq: base_seq });
        }
        self.git.put(id, record, base_seq)
    }

    fn append(&mut self, id: &str, entry: &ThreadEntry) -> Result<()> {
        self.saw("append");
        self.git.append(id, entry)
    }

    fn list_records(&self, scope: &Scope) -> Result<Vec<Object>> {
        self.saw("list");
        self.git.list_records(scope)
    }

    fn thread(&self, id: &str) -> Result<Vec<ThreadEntry>> {
        self.git.thread(id)
    }

    fn responses(&self, id: &str) -> Result<Vec<Response>> {
        self.git.responses(id)
    }

    fn leases(&self, id: &str) -> Result<Option<LeaseRecord>> {
        self.git.leases(id)
    }

    fn hosts(&self) -> Result<Vec<flywheel_atoms::HostRecord>> {
        self.git.hosts()
    }

    fn put_ask(&mut self, ask: &flywheel_atoms::Ask) -> Result<bool> {
        self.saw("put_ask");
        self.git.put_ask(ask)
    }

    fn asks(&self) -> Result<Vec<flywheel_atoms::Ask>> {
        self.git.asks()
    }
}

impl StateStore for HostStore {
    fn read(&self, id: &str) -> Result<flywheel_atoms::EvidenceRead> {
        self.saw("read");
        self.git.read(id)
    }

    fn list(&self, scope: &Scope) -> Result<Listing> {
        self.saw("list");
        self.git.list(scope)
    }

    fn status(&self) -> Result<StatusView> {
        self.git.status()
    }

    fn write_effect(&mut self, write: &EffectWrite) -> Result<WriteOutcome> {
        self.saw("write_effect");
        self.git.write_effect(write)
    }

    fn lease(&mut self, op: &LeaseOp) -> Result<LeaseOutcome> {
        self.saw("lease");
        self.git.lease(op)
    }

    fn notify(&self, since: &ReadPoint) -> Result<Notice> {
        self.git.notify(since)
    }

    fn present(&mut self, presentation: &Presentation) -> Result<()> {
        self.git.present(presentation)
    }

    fn receive(&mut self, response: &Response) -> Result<Received> {
        self.git.receive(response)
    }
}

impl EvidenceSource for HostStore {
    /// The record layer first, because it is the one half every profile shares;
    /// then what the world reports and this profile inherits; then the two
    /// bindings this release carries (`record-derived.yaml`, B.3, D8).
    fn evidence(&self, object: &str, region: &str, name: &str) -> Option<Value> {
        // A dictation whose transition is not active on the object it names is
        // unapplicable, and reported rather than kept (6, 154).
        if let (true, Some(defs)) = (name == "response.decision_present", self.defs.as_ref()) {
            if let Some(held) = self.git.get(object).ok().flatten() {
                let named = held
                    .record
                    .get("object")
                    .and_then(|v| v.as_str())
                    .and_then(|id| self.git.get(id).ok().flatten());
                if let Some(takes) = flywheel_domain::commands::dictation_takes(defs, &held, named.as_ref()) {
                    return Some(json!(takes));
                }
            }
        }
        flywheel_domain::derived::evidence(&self.git, &self.reading, object, name)
            // The stage's own reads — its agents from the type, its join and
            // verdict from the sessions' threads — and the item's retry bound
            // and archive (56, 41, `record-derived.yaml` stage.*).
            .or_else(|| {
                self.defs
                    .as_ref()
                    .and_then(|defs| flywheel_domain::stages::evidence(&self.git, defs, object, region, name))
            })
            // Whether the registry in force holds the object's type, which a
            // unit's and an elaboration's approval waits on (85a,
            // `record-derived.yaml` unit.type_defined, elaboration.type_defined).
            .or_else(|| {
                self.defs
                    .as_ref()
                    .and_then(|defs| flywheel_domain::blueprints::evidence(&self.git, defs, object, name))
            })
            // The signal material, read from the blueprints under the
            // machinery's prefix (111, 107, `blueprints.yaml` evidence).
            .or_else(|| flywheel_domain::signals::evidence(&*self.material(), object, name))
            // The change directory behind an intent, on the same shared line
            // (A.11, 227).
            .or_else(|| {
                flywheel_domain::changes::evidence(
                    &flywheel_domain::signals::Blueprints(&*self.world),
                    object,
                    name,
                )
            })
            // What the manifest says the instance is (204, 207).
            .or_else(|| name.starts_with("instance.").then(|| self.instance.get(name).cloned()).flatten())
            // Curation's proofs read the blueprints and the records together
            // (107, 109, 116).
            .or_else(|| flywheel_domain::signals::proofs(&self.git, &*self.material(), name))
            // `record_refusals` and its proof, read from one comparison: every
            // refusal on the session's thread is in the run record (43, 79).
            // Pending is the complement of recorded and is answered here
            // rather than in the sessions binding, because the run record is
            // the host's. Answered as the thread alone it would be true for
            // ever, and a session that was refused once would keep firing
            // `record_refusals` and never reach its exit.
            .or_else(|| {
                matches!(name, "session.refusals_recorded" | "session.refusals_pending").then(|| {
                    let stem = session_stem(object, region, self.kind_of(object).as_deref());
                    let session = flywheel_sessions_operator::current(&self.git, &stem);
                    let refused = self
                        .git
                        .thread(&session)
                        .unwrap_or_default()
                        .iter()
                        .filter(|e| e.kind == "refusal")
                        .count();
                    let recorded = self
                        .git
                        .run_record()
                        .unwrap_or_default()
                        .iter()
                        .filter(|e| e.kind == "refusal" && e.object == session)
                        .count();
                    json!(match name {
                        "session.refusals_pending" => recorded < refused,
                        _ => recorded >= refused,
                    })
                })
            })
            // A source is due while what it names holds a source event with no
            // capture yet — a transcript not captured, a signals folder with a
            // capture not read in — and run once it has none (111, 114, 215,
            // D13; `host.yaml` host.adapters_due).
            .or_else(|| {
                matches!(name, "host.adapters_due" | "host.adapters_run").then(|| {
                    let host = object.strip_prefix("host/").unwrap_or(object);
                    let due = self.sources_declared(host).iter().any(|source| {
                        let words: Vec<&str> = source.split_whitespace().collect();
                        if let [.., "signals", dir] = words.as_slice() {
                            return flywheel_domain::adapters::signals_due(&*self.world, dir);
                        }
                        let Some(path) = words.last() else {
                            return false;
                        };
                        let Ok(key) = flywheel_domain::adapters::meeting_key(path) else {
                            return false;
                        };
                        std::path::Path::new(path).exists()
                            && flywheel_domain::signals::read_capture(&*self.world, &key)
                                .ok()
                                .flatten()
                                .is_none()
                    });
                    json!(match name {
                        "host.adapters_due" => due,
                        _ => !due,
                    })
                })
            })
            // An offer's pin outlives its place and stands until the record the
            // offer made has ended; this host's clones are read in process (55,
            // 62, `host.yaml` host.no_stale_offer_pins).
            .or_else(|| {
                (name == "host.no_stale_offer_pins")
                    .then(|| {
                        let defs = self.defs.as_ref()?;
                        let stale = flywheel_domain::offers::stale_pins(&self.git, &*self.world, defs).ok()?;
                        Some(json!(stale.is_empty()))
                    })
                    .flatten()
            })
            .or_else(|| {
                flywheel_workspace_recorded::evidence(
                    &self.git,
                    &place_key(object, region),
                    name,
                )
            })
            .or_else(|| flywheel_workspace_recorded::evidence(&self.git, object, name))
            .or_else(|| {
                let session = flywheel_sessions_operator::current(
                    &self.git,
                    &session_stem(object, region, self.kind_of(object).as_deref()),
                );
                match self.sessions.as_str() {
                    "herdr" => flywheel_sessions_herdr::evidence(
                        &self.git,
                        &flywheel_sessions_herdr::Herdr::default(),
                        &session,
                        name,
                    ),
                    _ => flywheel_sessions_operator::evidence(&self.git, &session, name),
                }
            })
            // What the world reports and this profile inherits, last: a
            // binding that answers a name is the answer, and what the world
            // was told stands only where nothing is bound to it (B.3, D8). The
            // other order would let a described world outlive the bindings that
            // moved past it.
            //
            // The world names what it reports about by the thing itself — a
            // session by its id, a place by its key — so all three are asked
            // (`record-derived.yaml` B.3).
            .or_else(|| self.git.evidence(object, region, name))
            .or_else(|| {
                self.git
                    .evidence(&place_key(object, region), region, name)
            })
            .or_else(|| {
                let stem = session_stem(object, region, self.kind_of(object).as_deref());
                self.git.evidence(
                    &flywheel_sessions_operator::current(&self.git, &stem),
                    region,
                    name,
                )
            })
    }
}

/// The sinks this host presents, and what each delivers through (D8, D9,
/// 148).
///
/// A channel is loaded by name the way the workspace and the sessions are: one
/// implementation in this release, replaced rather than branched. A sink whose
/// channel this host has not loaded is one it does not present, whatever lease
/// it holds.
pub struct Sinks {
    /// The address every link a delivery carries is written at (205a, 308,
    /// D10a).
    pub address: String,
    pub channels: std::collections::BTreeMap<String, Box<dyn Channel + Send>>,
}

impl Sinks {
    pub fn at(address: &str) -> Sinks {
        Sinks {
            address: address.to_string(),
            channels: Default::default(),
        }
    }

    /// Load the channel one sink delivers through.
    pub fn bind(&mut self, sink: &str, channel: Box<dyn Channel + Send>) {
        self.channels.insert(sink.to_string(), channel);
    }
}

/// A host, running.
pub struct Host {
    pub name: String,
    pub instance: String,
    pub defs: Definitions,
    pub store: HostStore,
    pub bindings: Bindings,
    pub declaration: Declaration,
    pub bound: u32,
    pub intermittent: bool,
    /// The capture sources this host declares, as the manifest names them
    /// (215, 231).
    pub sources: Vec<String>,
    /// The run record this tick is building (79-82).
    pub run: Vec<RunEntry>,
    /// The refusals already in today's run record, by object, effect and
    /// reason. An effect this release does not bind leaves its proof absent, so
    /// the engine plans it on every tick and the binding refuses it on every
    /// tick; writing that refusal each time put one commit a second on the
    /// shared line for a deferral nothing was going to act on. 81 asks that a
    /// refusal be reported and never dropped, not that it be reported again for
    /// every pass in which it is still true (81, 167).
    said: std::collections::BTreeSet<String>,
    pub last_sweep: Option<DateTime<Utc>>,
    pub last_point: ReadPoint,
    /// An away host's leases stand and its clocks pause; it writes no heartbeat
    /// while it is away (150a).
    pub away_since: Option<DateTime<Utc>>,
    /// When this host last wrote its heartbeat. Liveness is recorded on the
    /// sweep's cadence and not on every pass: a tick that moved nothing writes
    /// nothing, and `cost.yaml`'s quiet tick costs no push (78, 147, 169).
    pub last_heartbeat: Option<DateTime<Utc>>,
    /// The sinks this host presents (148, D9).
    pub sinks: Sinks,
    /// Whether the last pass moved anything that was not a re-entry. A sweep
    /// settles while it did (model.md the tick).
    pub moved: bool,
    /// Whether the last pass of `once` moved anything that was not a re-entry.
    progressed: bool,
    /// What the manifest says curation is charged on, where this host read one
    /// (110, 118). Written onto the curation record as the host declares.
    pub curation: Option<flywheel_world_host::manifest::Curation>,
    /// The manifest this host was opened on, where it was opened on one. A
    /// host stepping a scenario reaches the instance's directories through it
    /// rather than remembering them (205); a host built in a test over a store
    /// alone has none.
    pub manifest: Option<PathBuf>,
    /// How often this host looks, in seconds: the poll it falls back on when
    /// nothing told it anything, and the sweep that fires an `older:` guard
    /// whatever the poll says (D6, D7, 130, 231). The model's own intervals
    /// unless `flywheel.yaml` says otherwise, so a test's backstop can be
    /// milliseconds and a host's stays sixty seconds.
    pub poll: f64,
    pub sweep_every: f64,
}

/// The manifest as this host reads it, with the root the command line names in
/// place of the manifest's own.
///
/// Several hosts run on one computer, told apart by id alone (232), and each
/// keeps its clones under a root of its own; `--root` is how a caller that made
/// those roots — the conformance runner under `--hosts real` — says which is
/// this host's, without writing a manifest per host (D15).
pub fn manifest_with_root(path: &Path, name: &str, root: Option<&Path>) -> Result<Manifest> {
    let mut read = Manifest::read(path)?;
    if let Some(root) = root {
        let entry = read
            .hosts
            .get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("the manifest names no host `{name}`"))?;
        entry.root = root.to_path_buf();
    }
    Ok(read)
}

impl Host {
    /// Open a host on the manifest's terms. Nothing here starts the loop.
    pub fn open(manifest: &Path, name: &str, root: Option<&Path>, now: DateTime<Utc>) -> Result<Host> {
        let read = manifest_with_root(manifest, name, root)?;
        let bindings = Bindings::read(&read, name)?;
        let world = HostWorld::open(read.clone(), name)?;
        let entry = read.host(name)?;
        // The state repository's checkout is the state store's own, and a
        // durable write is one that reached the git host: its origin is the
        // remote the manifest names, never this host's mirror of it (133, 161).
        // Two hosts on one computer therefore push at one shared line, which is
        // what makes the expected-old push a real compare-and-swap (134, 162).
        let git = GitStore::open(
            Path::new(&read.state.remote),
            &world.checkout("flywheel-state"),
            name,
            now,
        )
        .with_context(|| "opening this host's checkout of the state repository")?;
        let declaration = Declaration {
            repositories: match entry.covers.is_empty() {
                true => read.repositories.keys().cloned().collect(),
                false => entry.covers.clone(),
            },
            types: vec![],
            kinds: vec!["all".into()],
        };
        let workspace = bindings.workspace.clone();
        let sessions = bindings.sessions.clone();
        let root = world.root.clone();
        let mut host = Host::over(
            name,
            &read.instance,
            flywheel_domain::set::load()?,
            git,
            bindings,
            declaration,
            now,
        );
        host.store.workspace = workspace;
        host.store.sessions = sessions;
        host.store.root = Some(root);
        // What the host is, as the manifest says: how many sessions it runs at
        // once, and whether it is a laptop (31, 150a, 183).
        host.bound = entry.bound;
        host.intermittent = entry.intermittent;
        host.sources = entry.sources.clone();
        // How often this host looks, as the manifest says (D6, D7, 130, 231).
        host.poll = read.intervals.poll;
        host.sweep_every = read.intervals.sweep;
        host.curation = Some(read.curation.clone());
        // The manifest this host was opened on: what a step of a scenario
        // reaches the instance's own directories through (205).
        host.manifest = Some(manifest.to_path_buf());
        host.store.manifest = Some(manifest.to_path_buf());
        // The host's one address, from the router the manifest names: every
        // link a delivery carries is written at it (191, 205a, D10a).
        host.sinks.address = world.address_of(name)?;
        host.store.instance = flywheel_world_host::effects::evidence(&read, None);
        host.store.world = Box::new(world);
        Ok(host)
    }

    /// A host over a store already open: what a test and `open` share.
    pub fn over(
        name: &str,
        instance: &str,
        defs: Definitions,
        git: GitStore,
        bindings: Bindings,
        declaration: Declaration,
        now: DateTime<Utc>,
    ) -> Host {
        let mut store = HostStore::new(git, name, now);
        store.reading.declarations = vec![declaration.clone()];
        store.defs = Some(defs.clone());
        Host {
            store,
            name: name.to_string(),
            instance: instance.to_string(),
            defs,
            bindings,
            declaration,
            bound: 4,
            intermittent: true,
            sources: vec![],
            run: vec![],
            said: Default::default(),
            last_sweep: None,
            last_point: ReadPoint {
                mark: String::new(),
                seq: 0,
                at: now,
            },
            away_since: None,
            last_heartbeat: None,
            moved: false,
            progressed: false,
            curation: None,
            manifest: None,
            poll: POLL,
            sweep_every: SWEEP,
            // The host's own name on the operator's private network, with the
            // instance in the path; `open` replaces it with what the manifest's
            // router gives (205a, D10a).
            sinks: Sinks::at(&format!("http://{name}/{instance}")),
        }
    }

    pub fn now(&self) -> DateTime<Utc> {
        self.store.git.now
    }

    /// Move the host's clock. Every timed behaviour is a guard on the tick, so
    /// this is the only clock there is (D7, 231, D15).
    pub fn set_now(&mut self, now: DateTime<Utc>) {
        self.store.git.now = now;
        self.store.reading.now = now;
    }

    /// This host's declaration and its heartbeat: what it takes leases within,
    /// and that it is alive (147, 149, 163).
    pub fn declare(&mut self) -> Result<()> {
        // An away host writes no heartbeat: away is what its absence means, and
        // its leases stand meanwhile (150a).
        if self.away_since.is_some() {
            return Ok(());
        }
        let now = self.now();
        // The heartbeat is a push on the host's own ref, so it is written on
        // the sweep's cadence: at the first declare of a run and then once per
        // sweep interval, which keeps `last_seen` inside the five-minute
        // window a stale host is read by. Every pass in between writes nothing
        // (78, 130, 147, 150; `cost.yaml` pushes_per_tick).
        let due = self
            .last_heartbeat
            .is_none_or(|last| now - last >= self.sweep_interval());
        if due {
            self.store.git.heartbeat(self.bound, self.intermittent)?;
            self.last_heartbeat = Some(now);
        }
        let id = format!("host/{}", self.name);
        // The host object as it stands. Its own machine's states and the
        // moments they were entered are the machine's, not this writer's: a
        // fresh object here erased them on every tick and `initialise` put them
        // back stamped at that tick, so the host machine started from nothing
        // every pass and every `older:` guard on it read zero
        // (`host.yaml`, 147).
        let mut object = self.store.get(&id)?.unwrap_or_else(|| Object {
            id: id.clone(),
            machine: "host".into(),
            parent: None,
            config: Default::default(),
            entered_at: Default::default(),
            record: Default::default(),
            counters: Default::default(),
            applied_responses: vec![],
            seq: 0,
            created: 0,
        });
        let seq = object.seq;
        let record = &mut object.record;
        let declared = [
            ("bound".to_string(), json!(self.bound)),
            ("intermittent".to_string(), json!(self.intermittent)),
            ("sources".to_string(), json!(self.sources)),
            (
                "declares".to_string(),
                json!({
                    "repositories": self.declaration.repositories,
                    "types": self.declaration.types,
                    "kinds": self.declaration.kinds,
                }),
            ),
        ];
        // The record says what this host declares; the heartbeat says it is
        // alive. Stamping the hour onto the record on every tick made a commit
        // on the shared line out of a host that had done nothing and said
        // nothing new, which is the one thing 78 asks a tick not to do. So the
        // hour rides along with a declaration that changed and is written on
        // its own at the first declare of a run, where a reader with no
        // heartbeat to read needs it (78, 147, 150a, `git-only.yaml`
        // records.hosts).
        let changed = declared.iter().any(|(name, value)| record.get(name) != Some(value))
            || !record.contains_key("last_seen");
        for (name, value) in declared {
            record.insert(name, value);
        }
        if changed {
            record.insert("last_seen".into(), json!(now.to_rfc3339()));
        }
        if object.config.is_empty() {
            flywheel_engine::initialise(&self.defs, &mut object, now);
        }
        // Writing what is already there is not a write, and the store is what
        // keeps that (78, `records.put`).
        self.store.put(&id, &object, seq)?;
        self.settle_settings()?;
        Ok(())
    }

    /// What `flywheel.yaml` says an object of this instance is charged on,
    /// written onto that object's record where the evidence reads it.
    ///
    /// `curation.threshold` and `curation.cadence` are the operator's
    /// settings; the evidence reads them off `curation/<instance>` (110, 118,
    /// `blueprints.yaml` evidence.curation.threshold). Writing them here rather
    /// than at `init` alone is what makes changing the manifest take effect: a
    /// setting nothing reads after the first run is not a setting. A record
    /// that already says this is not written again (78, 127).
    fn settle_settings(&mut self) -> Result<()> {
        let Some(settings) = self.curation.clone() else {
            return Ok(());
        };
        let id = format!("curation/{}", self.instance);
        let Some(mut held) = self.store.get(&id)? else {
            return Ok(());
        };
        let wanted = [
            ("threshold".to_string(), json!(settings.threshold)),
            ("cadence".to_string(), json!(settings.cadence)),
        ];
        if wanted.iter().all(|(name, value)| held.record.get(name) == Some(value)) {
            return Ok(());
        }
        for (name, value) in wanted {
            held.record.insert(name, value);
        }
        let seq = held.seq;
        self.store.put(&id, &held, seq)?;
        Ok(())
    }

    /// Away with since-when: the leases stand, the stall clocks pause, and no
    /// attention line is raised for it (150a).
    pub fn go_away(&mut self, since: DateTime<Utc>) {
        self.away_since = Some(since);
    }

    /// Back, with nothing to answer (150a).
    pub fn come_back(&mut self) -> Result<()> {
        self.away_since = None;
        self.declare()
    }

    /// Renew what this host holds and take what its declaration covers, never
    /// more than its bound (149, 150, 163).
    fn renew_and_take(&mut self, objects: &[Object]) -> Result<()> {
        let me = self.name.clone();
        let now = self.now();
        for object in objects {
            if object.machine == "host" {
                continue;
            }
            // A sink's presenter lease is the declaring host's: the one that
            // loaded its channel. Covering every kind of work is not
            // presenting a sink (148, D8).
            if object.machine == "sink" && !self.sinks.channels.contains_key(&object.id) {
                continue;
            }
            if !self.declaration.covers(object) {
                continue;
            }
            let standing = self.store.leases(&object.id)?;
            let mine = standing.as_ref().is_some_and(|l| l.holder == me);
            // A lease nobody holds is free to take, whatever its record says
            // about when it was last renewed: a released lease is a released
            // lease (128, 150, `lease.yaml` free).
            let expired = standing
                .as_ref()
                .map(|l| {
                    l.holder.is_empty() || now - l.renewed_at > self.store.git.lease_expiry
                })
                .unwrap_or(true);
            if !mine && !expired {
                continue;
            }
            // A host that cannot reach the store keeps ticking what it already
            // holds and takes nothing new (151, D4a). Renewals stand: they push
            // first when the route comes back (165).
            if !mine && self.store.git.disconnected {
                continue;
            }
            let op = match mine {
                true => LeaseOp::Renew {
                    object: object.id.clone(),
                    holder: me.clone(),
                },
                false => LeaseOp::Take {
                    object: object.id.clone(),
                    holder: me.clone(),
                },
            };
            let outcome = self.store.lease(&op)?;
            // A take is a push with expected-old, and the push is the
            // compare-and-swap: two hosts asking for one lease is the race the
            // whole profile rests on, so the record says who asked, who got it,
            // and that the loser read who holds it before deciding anything
            // (128, 134, 162, I15, 167). A renewal is not news and is not
            // recorded: it happens on every tick of everything a host holds.
            if !mine {
                self.run.push(
                    RunEntry::new(now, &me, "lease", &object.id, "take")
                        .with(
                            "outcome",
                            match &outcome {
                                LeaseOutcome::Held(_) => "held",
                                LeaseOutcome::HeldByAnother(_) => "held-by-another",
                                LeaseOutcome::Released => "released",
                            },
                        )
                        .with(
                            "holder",
                            match &outcome {
                                LeaseOutcome::Held(l) | LeaseOutcome::HeldByAnother(l) => {
                                    l.holder.as_str()
                                }
                                LeaseOutcome::Released => "",
                            },
                        )
                        .with(
                            "reread",
                            &self.store.git.reread_after_rejection.to_string(),
                        ),
                );
            }
        }
        Ok(())
    }

    /// Renew every lease this host holds, and say how many stood.
    ///
    /// On reconnect these push first (165, D4a). A renewal the git host rejects
    /// means the object was taken over while the route was down: this host ends
    /// its own session for it, reports, and its local commits on that object are
    /// discarded rather than rebased over the taking host's.
    pub fn renew_held(&mut self) -> Result<usize> {
        let me = self.name.clone();
        let now = self.now();
        let mut stood = 0;
        let objects = self.store.list_records(&Scope::All)?;
        for object in objects {
            let Some(lease) = self.store.leases(&object.id)? else {
                continue;
            };
            if lease.holder != me {
                continue;
            }
            let outcome = self.store.lease(&LeaseOp::Renew {
                object: object.id.clone(),
                holder: me.clone(),
            })?;
            match outcome {
                LeaseOutcome::Held(_) => stood += 1,
                // Taken over while the route was down: this host ends its own
                // session for it and starts nothing again — the fresh attempt
                // is the taking host's (165, 150).
                LeaseOutcome::HeldByAnother(held) => {
                    for session in flywheel_sessions_operator::of_host(&self.store.git, &me)
                        .into_iter()
                        .filter(|s| s.starts_with(&object.id))
                    {
                        flywheel_sessions_operator::end(&mut self.store.git, &session, now)?;
                    }
                    self.run.push(
                        RunEntry::new(now, &me, "lease", &object.id, "this host's renewal was rejected; the object was taken over while its route was down")
                            .with("holder", &held.holder),
                    );
                }
                LeaseOutcome::Released => {}
            }
        }
        Ok(stood)
    }

    /// One tick over a scope: fetch, heartbeat, read, decide, write, perform,
    /// and write the run record (D7, 79).
    pub fn tick(&mut self, scope: &Scope) -> Result<usize> {
        // Fetch first, always: no host decides on a read older than the bound
        // and no person runs the sync by hand (165). What this tick writes is
        // one push at the end of it, carrying every commit the tick made: one
        // write to the central service per tick is what the profile's mechanism
        // spends, and it is what the cost contract asserts
        // (167, 169, `git-only.yaml` cost, audit 8).
        self.store.trace.borrow_mut().clear();
        // The signal material is read afresh each tick, once (111, 203).
        self.store.material.borrow_mut().take();
        flywheel_atoms::StateStore::begin_tick(&mut self.store.git);
        self.store.git.fetch()?;
        self.store.saw("fetch");
        // The fetch's own point is where the next notice is measured from.
        self.last_point = self.store.git.as_of();
        self.declare()?;

        let objects = self.store.list(scope)?.objects;
        let me = self.name.clone();
        let now = self.now();
        // The rail's projection has one writer, and the lease is what decides
        // which host it is: `render_status` is an effect of the rail object, so
        // the holder of the rail's lease is its only writer (D12, 132, 148).
        // The push is the compare-and-swap, so exactly one host holds it.
        let rail = self.store.leases(flywheel_domain::RAIL)?;
        if !self.store.git.disconnected
            && rail.as_ref().is_none_or(|l| {
                l.holder.is_empty()
                    || l.holder == me
                    || now - l.renewed_at > self.store.git.lease_expiry
            })
        {
            let _ = self.store.lease(&LeaseOp::Take {
                object: flywheel_domain::RAIL.to_string(),
                holder: me.clone(),
            });
        }
        self.renew_and_take(&objects)?;
        self.store.reading.register = console::register(&self.store)?;
        // The decisions standing after the last derive, beside the register
        // that numbered them. `response.decision_present` is read against this
        // pair, and a host that left it empty would read every answer the
        // operator gave as one whose decision was already gone — reporting each
        // under attention and putting it back on the rail, against 13
        // (`record-derived.yaml`, `engine/response.yaml`).
        self.store.reading.standing = console::standing(&self.store)?;

        let defs = self.defs.clone();
        // Two closures write here, so the entries are held where both reach
        // them; the order they are written in is the order they happened.
        let run: RefCell<Vec<RunEntry>> = RefCell::new(Vec::new());
        // What this host has already said it could not do, so it says it once
        // (81). Taken for the tick and given back at the end of it.
        let said: RefCell<std::collections::BTreeSet<String>> =
            RefCell::new(std::mem::take(&mut self.said));
        // A self-transition re-enters the state it is in, so a pass that only
        // re-entered has settled (model.md the tick).
        let moved = std::cell::Cell::new(false);
        // What the store had written when this pass began. A self-transition
        // that performs an effect changes the world without changing a state —
        // `merging` re-enters itself to run `merge_place` — and read as "no
        // state moved" the cascade stopped there with the merge owed and the
        // sibling region never told. Since a tick that moves nothing writes
        // nothing (78), the converse is the rule: a pass that wrote has moved.
        let written_before = self.store.git.writes_attempted;
        let sinks = &mut self.sinks;
        let ticked = console::tick_as(
            &mut self.store,
            &defs,
            scope,
            // A host owns what it acts on through a lease: it writes on what it
            // holds and on the machinery's own, and on nothing else (128, 149,
            // 162, I15).
            Some(&me),
            |store, object, region, effect| {
                let did = perform(&defs, store, sinks, &me, now, object, region, effect);
                // What this host performed, with the effect's own identity: a
                // reader of the record knows which act ran on which object and
                // whether it was performed here (79, 127, 167). An effect no
                // binding covers, and one whose binding failed, say so with
                // their reason under attention and are never recorded as
                // performed (81, tasks 16.1, 16.2).
                let mut entry = RunEntry::new(now, &me, "effect", object, &effect.name)
                    .with("region", region)
                    .with("performed", &did.done().to_string());
                if !did.done() && !did.why().is_empty() {
                    entry = entry
                        .with(
                            match did {
                                Performed::Failed(_) => "failed",
                                _ => "refused",
                            },
                            did.why(),
                        )
                        .with("attention", "true");
                    // Said once, and not again while it goes on being true. An
                    // effect no binding covers is refused on every tick for as
                    // long as the object stands there, and a record that
                    // repeats it is a commit a second saying nothing new
                    // (81, 167).
                    let again = format!("{object}\t{}\t{}", effect.name, did.why());
                    if !said.borrow_mut().insert(again) {
                        return did.done();
                    }
                }
                run.borrow_mut().push(entry);
                did.done()
            },
            |store, fired, tail| {
                moved.set(moved.get() || fired.from != fired.to);
                // The evidence the guard read, name by name: what the write was
                // decided on, not everything the object holds (79, 167).
                let evidence: Vec<(String, Value)> = guard_evidence(&defs, store, fired)
                    .into_iter()
                    .map(|name| {
                        let read = store
                            .evidence(&fired.object, &fired.region, &name)
                            .unwrap_or(Value::Null);
                        (name, read)
                    })
                    .collect();
                run.borrow_mut().push(
                    RunEntry::new(now, &me, "write", &fired.object, &format!(
                        "{}: {} → {}{}",
                        fired.region,
                        fired.from,
                        fired.to,
                        fired
                            .note
                            .as_ref()
                            .map(|n| format!(" — {n}"))
                            .unwrap_or_default()
                    ))
                    .with("region", &fired.region)
                    .with("from", &fired.from)
                    .with("to", &fired.to)
                    // The response this write consumed, where one did: what a
                    // reader needs to tell a move the operator asked for from
                    // one the machinery made (79, 137, 167).
                    .with(
                        "response",
                        fired
                            .response
                            .as_ref()
                            .map(|(id, _)| id.as_str())
                            .unwrap_or(""),
                    )
                    .reading(evidence),
                );
                let _ = tail;
            },
        )?;
        // Sampled before the run record and the projection are written, which
        // are this pass's account of itself rather than part of it.
        self.moved = moved.get() || self.store.git.writes_attempted > written_before;
        // A pass of `once` has progressed if any tick under it moved something
        // that was not a re-entry; a sweep settles and leaves `moved` false, so
        // the record is kept here where every tick passes (78).
        self.progressed |= self.moved;
        self.run.extend(run.into_inner());
        self.said = said.into_inner();
        // What an effect moved out of band, said in the same record and after
        // the write that caused it (79, 167).
        for (object, from, to) in self.store.marked.take() {
            self.run.push(
                RunEntry::new(now, &me, "write", &object, &format!("hold: {from} → {to}"))
                    .with("region", "hold")
                    .with("from", &from)
                    .with("to", &to)
                    .with("response", ""),
            );
        }
        self.take_over_released()?;
        self.report_takeovers()?;
        self.report_sessions()?;
        self.rewrite_status()?;
        let entries = std::mem::take(&mut self.run);
        self.store.git.append_run(&entries)?;
        // The tick is over: its commits go at the shared line, once (167, 169).
        flywheel_atoms::StateStore::end_tick(&mut self.store.git)?;
        Ok(ticked.transitions)
    }

    /// The sweep: every scope this host has a lease or a candidate on, which is
    /// what makes `older:` guards fire and a never-notified host converge
    /// (130, D7).
    pub fn sweep(&mut self) -> Result<usize> {
        // A sweep settles: it passes again while something moved, because a
        // region's move is its siblings' to read on the next pass and a host
        // that stopped after one would carry the rest to the next sweep for no
        // reason (model.md the tick, D7). A self-transition re-enters the state
        // it is in, so a pass that only re-entered has settled.
        let mut fired = 0;
        for pass in 0..PASSES {
            let before = self.run.len();
            fired += self.tick(&Scope::All)?;
            if std::env::var("FLYWHEEL_SWEEP_TRACE").is_ok() {
                eprintln!("SWEEP pass {pass} fired {fired} moved {} entries {:?}", self.moved,
                    self.run.iter().skip(before).filter(|e| e.kind=="write").map(|e| format!("{} {}", e.object, e.reason)).collect::<Vec<_>>());
            }
            if !self.moved {
                break;
            }
        }
        self.last_sweep = Some(self.now());
        self.settle_record()?;
        Ok(fired)
    }

    /// Write what this host still owes its run record. A tick's append is one
    /// push at a shared line other hosts are pushing at too, and three
    /// rejections leave the entries owed rather than written
    /// (`git-only.yaml records.put`). What a host did and why is read with no
    /// host running, so a sweep does not end while its own record is unsaid
    /// (79, 81, 167).
    fn settle_record(&mut self) -> Result<()> {
        for _ in 0..flywheel_store_git::store::RETRIES {
            if self.store.git.owed_run() == 0 {
                return Ok(());
            }
            self.store.git.append_run(&[])?;
        }
        if self.store.git.owed_run() > 0 && !self.store.git.disconnected {
            self.report_problem(
                &format!("host/{}", self.name),
                &format!(
                    "{} run-record entr{} could not be written at the shared line",
                    self.store.git.owed_run(),
                    match self.store.git.owed_run() {
                        1 => "y",
                        _ => "ies",
                    }
                ),
            );
        }
        Ok(())
    }

    /// What moved since the last read, so a host re-reads only what a notice
    /// names (130, D6).
    pub fn notified(&self) -> Result<Vec<String>> {
        Ok(self.store.notify(&self.last_point)?.objects)
    }

    /// One pass of the loop: the notify-tick for what moved and its parents,
    /// and the sweep when it is due.
    pub fn once(&mut self) -> Result<usize> {
        let mut fired = 0;
        // Whether this pass moved anything that was not a re-entry: what tells
        // a cascade still running from a machine re-entering the state it is
        // already in. A caller that takes its next pass on progress reads this
        // and never the count, because a self-transition is not progress
        // (model.md the tick, 78).
        self.progressed = false;
        let due = self
            .last_sweep
            .map(|last| self.now() - last >= self.sweep_interval())
            .unwrap_or(true);
        // The notify-tick is what saves a host from reading everything when one
        // object moved; when the sweep is due on the same pass it reads
        // everything anyway, over every scope the notice names and more, so
        // doing both is reading the same store twice (78, 130, D6, D7). It is
        // the whole of a host's first pass, where nothing has been notified
        // since a point this host never held: every object came back as news,
        // each took a tick of its own, and the sweep then read them all again —
        // which is the second or so each of those ticks costs, held against the
        // page waiting on the same host (D11).
        match due {
            true => fired += self.sweep()?,
            false => {
                for object in self.notified()? {
                    for scope in self.chain(&object)? {
                        fired += self.tick(&scope)?;
                    }
                }
            }
        }
        Ok(fired)
    }

    /// Whether the last pass moved anything that was not a re-entry (78).
    pub fn progressed(&self) -> bool {
        self.progressed
    }

    /// The sweep's interval as a duration, from this host's setting (D7, 231).
    pub fn sweep_interval(&self) -> Duration {
        Duration::milliseconds((self.sweep_every * 1000.0) as i64)
    }

    /// The poll's interval as a duration, from this host's setting (D6, 130).
    pub fn poll_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(self.poll.max(0.0))
    }

    /// An object and its parent chain: what a notify ticks (D7, model.md §2.1).
    fn chain(&self, object: &str) -> Result<Vec<Scope>> {
        let mut out = vec![Scope::Under(object.to_string())];
        let mut at = self.store.get(object)?.and_then(|o| o.parent);
        while let Some(parent) = at {
            out.push(Scope::Under(parent.clone()));
            at = self.store.get(&parent)?.and_then(|o| o.parent);
        }
        Ok(out)
    }

    /// A host the operator's response released is one whose work is now this
    /// host's to pick up: its leases are free and the sessions it was running
    /// are closed, so what starts next is a fresh attempt and never the same
    /// session twice (150, `session.yaml` id).
    fn take_over_released(&mut self) -> Result<()> {
        let now = self.now();
        let me = self.name.clone();
        let released: Vec<String> = self
            .store
            .list_records(&Scope::All)?
            .into_iter()
            .filter(|o| o.machine == "host")
            .filter(|o| {
                matches!(
                    o.config.get("life").map(String::as_str),
                    Some("released") | Some("gone")
                )
            })
            .map(|o| o.id.trim_start_matches("host/").to_string())
            .filter(|host| host != &me)
            .collect();
        for host in released {
            for session in flywheel_sessions_operator::of_host(&self.store.git, &host) {
                flywheel_sessions_operator::take_over(&mut self.store.git, &session, &me, now)?;
                self.run.push(
                    RunEntry::new(now, &me, "session", &session, "the host running it was taken over; the next attempt is this host's")
                        .with("was", &host),
                );
            }
        }
        Ok(())
    }

    /// A session this host was running that another host took over: it ends
    /// its own session and reports, and starts nothing again — the fresh
    /// attempt is the taking host's (150).
    fn report_takeovers(&mut self) -> Result<()> {
        let now = self.now();
        let me = self.name.clone();
        let taken: Vec<Object> = self
            .store
            .list_records(&Scope::All)?
            .into_iter()
            .filter(|o| o.id.starts_with("fact/session/"))
            .filter(|o| o.record.get("host").and_then(|v| v.as_str()) == Some(me.as_str()))
            .filter(|o| o.record.get("taken_over_by").is_some_and(|v| !v.is_null()))
            .filter(|o| !o.record.get("reported_by_holder").is_some_and(|v| !v.is_null()))
            .collect();
        for fact in taken {
            let session = fact.id.trim_start_matches("fact/session/").to_string();
            let by = fact
                .record
                .get("taken_over_by")
                .and_then(|v| v.as_str())
                .unwrap_or("another host")
                .to_string();
            flywheel_sessions_operator::end(&mut self.store.git, &session, now)?;
            flywheel_sessions_operator::set(
                &mut self.store.git,
                &session,
                &[("reported_by_holder", json!(now.to_rfc3339()))],
            )?;
            // Ending its own is an act like any other, and the record says so:
            // the returning host closed the session it was running and started
            // nothing again (79, 150, 167).
            self.run.push(
                RunEntry::new(now, &me, "effect", &session, "end_session")
                    .with("region", "life")
                    .with("performed", "true"),
            );
            self.run.push(
                RunEntry::new(now, &me, "session", &session, "this host was taken over; it ended its own session and stopped")
                    .with("taken_over_by", &by),
            );
        }
        Ok(())
    }

    /// What was expected of a session beside what it delivered, difference
    /// first, and every refusal it wrote (80, 4, 79, 81).
    fn report_sessions(&mut self) -> Result<()> {
        let now = self.now();
        let me = self.name.clone();
        // What this host has already said about a session, on the shared line
        // and in the entries this tick still owes. A thread entry is said once:
        // deduplicating against the tick's own pending entries alone made every
        // later tick report the same exit again, so one session's delivery
        // became a run-record entry and a commit on every pass for the rest of
        // the day (78, 79, 127).
        let said: std::collections::HashSet<(String, String)> = self
            .store
            .git
            .run_record()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|r| {
                r.fields
                    .iter()
                    .find(|(n, _)| n == "entry")
                    .map(|(_, v)| (r.object.clone(), v.clone()))
            })
            .collect();
        let sessions: Vec<Object> = self
            .store
            .list_records(&Scope::All)?
            .into_iter()
            .filter(|o| o.id.starts_with("fact/session/"))
            .collect();
        for fact in sessions {
            let session = fact.id.trim_start_matches("fact/session/").to_string();
            let expected: Vec<String> = fact
                .record
                .get("deliverables")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            for entry in self.store.thread(&session)? {
                let reported = format!("{}/{}", entry.kind, entry.at.to_rfc3339());
                if said.contains(&(session.clone(), reported.clone()))
                    || self.run.iter().any(|r| r.object == session
                        && r.fields.iter().any(|(n, v)| n == "entry" && *v == reported))
                {
                    continue;
                }
                match entry.kind.as_str() {
                    "exit" => {
                        let delivered: Vec<String> = entry
                            .fields
                            .get("deliverables")
                            .and_then(|v| serde_json::from_value(v.clone()).ok())
                            .unwrap_or_default();
                        // The difference first: what was asked for and did not
                        // arrive is the thing a person needs to see (80).
                        let missing: Vec<&String> =
                            expected.iter().filter(|d| !delivered.contains(d)).collect();
                        self.run.push(
                            RunEntry::new(now, &me, "session", &session, "a session reported")
                                .with("entry", &reported)
                                .with(
                                    "missing",
                                    &missing
                                        .iter()
                                        .map(|d| d.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", "),
                                )
                                .with("expected", &expected.join(", "))
                                .with("delivered", &delivered.join(", "))
                                .with(
                                    "exit",
                                    entry
                                        .fields
                                        .get("exit")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("none"),
                                ),
                        );
                    }
                    // Every refusal, with the identity, the operation and the
                    // object; the attention line is the machinery's own (4, 79).
                    "refusal" => self.run.push(
                        RunEntry::new(now, &me, "refusal", &session, "a refusal was recorded")
                            .with("entry", &reported)
                            .with("identity", entry.by.as_deref().unwrap_or("unknown"))
                            .with(
                                "operation",
                                entry
                                    .fields
                                    .get("operation")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("the work it was given"),
                            )
                            .with(
                                "reason",
                                entry
                                    .fields
                                    .get("reason")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or(""),
                            )
                            .with("attention", "true"),
                    ),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Rewrite the status projection from its source and report any drift, with
    /// both values (77, 142, D12). Only the rail's lease holder writes the file.
    pub fn rewrite_status(&mut self) -> Result<()> {
        // A host with no route writes no projection: the file is what a reader
        // with no host running reads off the shared line, and this host's copy
        // of it is not a fact (132, 145, 161, D4a, D12).
        if self.store.git.disconnected {
            return Ok(());
        }

        // The projection is written by the host the rail's machine told to
        // render it, which is the host that is ticking. D12 made the rail's
        // lease holder its only writer so two hosts could not conflict on the
        // one file every host writes; `commit_status` is that rule now — the
        // push is a compare-and-swap and a body differing only in the point it
        // is as of is not a write (77, 78, 127, 134). The lease gate outlived
        // its reason and cost 145: a holder that stops ticking left the
        // projection stale with the machine asking for it on every pass.
        // The projection is rewritten whenever the state it projects moved, and
        // not otherwise: reading twice with nothing changed writes nothing, and
        // rendering it is a read of every object (78, `engine/rail.yaml`
        // status, `record-derived.yaml` rail.status_current).
        //
        // Two things have to be true to skip it, and the second is what keeps
        // 77 and 142: the state has not moved *and* what is committed is still
        // what this host wrote. A projection somebody edited by hand is not the
        // truth however still the state is, so a body whose digest is not the
        // one recorded is rewritten from its source and reported as drift.
        let mark = flywheel_domain::commands::state_mark(&self.store.git)?;
        let held = self.store.git.committed_status()?;
        let committed = held
            .as_deref()
            .map(flywheel_store_git::store::without_the_stamp)
            .map(|body| digest(&body));
        if flywheel_domain::commands::status_of(&self.store.git)?.as_deref() == Some(mark.as_str())
            && committed.is_some()
            && committed == flywheel_domain::commands::status_digest(&self.store.git)?
        {
            return Ok(());
        }
        let as_of = self.store.git.as_of();
        let now = self.now();
        let status = flywheel_domain::status::read_with(
            &self.store.git,
            &self.defs,
            &as_of,
            now,
            Duration::minutes(5),
            Duration::minutes(30),
            &flywheel_domain::signals::snapshot(&*self.store.world),
        )?;
        let view = flywheel_domain::status::render(&status);
        let fresh = digest(&flywheel_store_git::store::without_the_stamp(&view.body));
        // Drift is the projection saying something its source does not, and the
        // difference the report is for is the one nothing accounts for. A body
        // that is what was last written and differs from the source is the
        // source having moved on, which is the ordinary case and what the
        // rewrite below is for; a body that is neither what was last written
        // nor what the source now says was put there by a hand or by a host
        // reading something else, and that is what 77 asks be reported with
        // both values. Reading drift off the whole difference reported the
        // ordinary case on every tick, which buried the one case the report is
        // for (77, 78, 142).
        if let Some(committed) = &committed {
            let written = flywheel_domain::commands::status_digest(&self.store.git)?;
            let ours = written.as_deref() == Some(committed.as_str());
            if !ours && committed != &fresh {
                self.run.push(
                    RunEntry::new(now, &self.name, "drift", flywheel_domain::RAIL, "the status projection differed from its source and was rewritten")
                        .with("was", committed)
                        .with("now", &fresh),
                );
            }
        }
        self.store.git.commit_status(&view.body)?;
        // What the next tick reads to know the projection is current: the point
        // the state was at, and the body that was written. It goes on the rail
        // record and not in this process, so a host that restarts reads what
        // the last one wrote rather than calling the file drifted (77, 78, 136,
        // 145, I14).
        flywheel_domain::commands::set_projection(&mut self.store.git, &mark, &fresh)?;
        Ok(())
    }

    /// The status view as this host serves it, from the same read (132, 141).
    pub fn status(&self) -> Result<flywheel_domain::status::Status> {
        flywheel_domain::status::read_with(
            &self.store.git,
            &self.defs,
            &self.store.git.as_of(),
            self.now(),
            Duration::minutes(5),
            Duration::minutes(30),
            &flywheel_domain::signals::snapshot(&*self.store.world),
        )
    }

    /// What stands under attention: the decisions the rail derives into the
    /// attention group, and every refusal this host recorded (4, 79, 81, 82).
    /// A refusal is never dropped and never becomes work.
    pub fn attention(&mut self) -> Result<Vec<String>> {
        let defs = self.defs.clone();
        let mut out: Vec<String> = console::rail(&mut self.store, &defs)?
            .into_iter()
            .filter(|d| d.group == "attention")
            .map(|d| format!("{}: {}", d.kind, d.object))
            .collect();
        // An object no host's declaration covers is under attention and never a
        // silent wait (149) — but it is not a decision the operator answers, so
        // it is read from the lease records and the status view rather than
        // taking a number on the rail: one card per lease buries the work the
        // rail is for (79, 141, 310).
        for object in self.store.list_records(&Scope::All)? {
            if !flywheel_domain::leases::leasable(&object) {
                continue;
            }
            if self
                .store
                .leases(&object.id)?
                .is_some_and(|l| l.state == "uncovered")
            {
                out.push(format!(
                    "uncovered: {}",
                    flywheel_domain::leases::id_for(&object.id)
                ));
            }
        }
        for entry in self.store.git.run_record()? {
            if entry.kind == "refusal" {
                out.push(format!("refusal: {}", entry.object));
            }
        }
        Ok(out)
    }

    /// Load the channel of every sink this host presents, by the name the
    /// sink's manifest entry gives it (D8, D9, 148).
    ///
    /// Each sink is written as the manifest describes it, its mark kept (14),
    /// and `open` makes the channel a `discord` entry names. A sink that cannot
    /// be opened — its token not placed, a channel this release does not bind,
    /// a name `sinks:` does not hold — is refused with its reason under
    /// attention, and the host runs on presenting nothing there (217f, 81).
    /// What it did is returned in lines, for the operator watching the host
    /// start.
    pub fn present_sinks(
        &mut self,
        manifest: &Manifest,
        mut open: impl FnMut(
            &str,
            &flywheel_world_host::manifest::Sink,
        ) -> Result<Box<dyn Channel + Send>>,
    ) -> Result<Vec<String>> {
        let me = self.name.clone();
        let now = self.now();
        let defs = self.defs.clone();
        let mut said = Vec::new();
        for name in &manifest.host(&me)?.presents {
            let id = flywheel_domain::sinks::id_for(name);
            let opened = match manifest.sinks.get(name) {
                None => Err(anyhow::anyhow!(
                    "flywheel.yaml hosts.{me}.presents names `{name}`, which `sinks:` does not \
                     hold (148)"
                )),
                Some(entry) if entry.channel != "discord" => Err(anyhow::anyhow!(
                    "flywheel.yaml sinks.{name}.channel: `{}` — this release binds `discord` (D9)",
                    entry.channel
                )),
                Some(entry) => {
                    flywheel_domain::sinks::ensure(
                        &mut self.store,
                        &defs,
                        &flywheel_domain::sinks::Spec {
                            name: name.clone(),
                            kind: entry.kind.clone(),
                            member: entry.member.clone(),
                            surface: entry.surface.clone(),
                            routes: entry.routes.clone(),
                            filter: "all".into(),
                        },
                    )?;
                    open(name, entry).map(|channel| (channel, entry.channel.clone()))
                }
            };
            match opened {
                Ok((channel, bound)) => {
                    self.sinks.bind(&id, channel);
                    said.push(format!("presents {name} on {bound}"));
                }
                Err(e) => {
                    let reason = format!("{e:#}");
                    self.run.push(
                        RunEntry::new(now, &me, "refusal", &id, &reason).with("attention", "true"),
                    );
                    said.push(format!("under attention: {reason}"));
                }
            }
        }
        Ok(said)
    }

    /// Hand what arrived at each sink this host presents to the sink: a
    /// numbered reply or a press answered through the same tool the page calls,
    /// a forward captured, anything else answered with what the sink takes
    /// (194, 112, 215, D9). Returns how many messages were read.
    ///
    /// Only the presenter hears, so a reply is answered once however many hosts
    /// listen to the channel (148). While another host holds the sink's lease,
    /// what arrives is that host's to hear and is let go here; while nobody
    /// holds it yet, it waits (217f). What stopped a channel hearing, and a
    /// message that could not be read, is reported under attention (81).
    pub fn hear(&mut self) -> Result<usize> {
        let me = self.name.clone();
        let now = self.now();
        let defs = self.defs.clone();
        let address = self.sinks.address.clone();
        let mut read = 0;
        let bound: Vec<String> = self.sinks.channels.keys().cloned().collect();
        for sink in bound {
            let presenter = flywheel_domain::sinks::presenter(&self.store, &sink)?;
            let Some(mut channel) = self.sinks.channels.remove(&sink) else {
                continue;
            };
            let outcome = match presenter.as_deref() {
                None => Ok(()),
                Some(holder) if holder != me => channel.heard().map(|_| ()),
                Some(_) => {
                    let mut chat = Chat::new(&sink, &me, &address, channel);
                    let mut outcome = Ok(());
                    match chat.channel.heard() {
                        Ok(messages) => {
                            for message in &messages {
                                let received = self.store.with_world(|store, world| {
                                    chat.receive(store, world, &defs, message)
                                });
                                match received {
                                    Ok(_) => read += 1,
                                    Err(e) => {
                                        outcome = Err(e.context(format!(
                                            "reading message {} in `{sink}`",
                                            message.id
                                        )))
                                    }
                                }
                            }
                        }
                        Err(e) => outcome = Err(e),
                    }
                    channel = chat.channel;
                    outcome
                }
            };
            self.sinks.channels.insert(sink.clone(), channel);
            if let Err(e) = outcome {
                self.run.push(
                    RunEntry::new(now, &me, "problem", &sink, &format!("{e:#}"))
                        .with("attention", "true"),
                );
            }
        }
        Ok(read)
    }

    /// Report a problem with the machinery. It goes in the run record and makes
    /// no work: a broken tool is not a job for the flywheel (81).
    pub fn report_problem(&mut self, about: &str, what: &str) {
        let now = self.now();
        self.run
            .push(RunEntry::new(now, &self.name, "problem", about, what));
    }

    /// The bindings this host loaded, written where the run record keeps them
    /// (139, 93a).
    pub fn record_bindings(&mut self) {
        let now = self.now();
        let name = self.name.clone();
        let mut entry = RunEntry::new(now, &name, "binding", &format!("host/{name}"), "the bindings this host loaded");
        for (seam, bound) in self.bindings.named() {
            entry = entry.with(&seam, &bound);
        }
        entry = entry.with("definitions", &flywheel_domain::set::SET_VERSION.to_string());
        self.run.push(entry);
    }
}

/// The evidence names the transition that fired was guarded by. The object has
/// already moved by the time this is asked, so the guard is found on the state
/// it came from.
fn guard_evidence(defs: &Definitions, store: &HostStore, fired: &flywheel_engine::tick::Fired) -> Vec<String> {
    let Ok(Some(object)) = store.get(&fired.object) else {
        return vec![];
    };
    let mut probe = object.clone();
    probe.config.insert(fired.region.clone(), fired.from.clone());
    let Some((_region, state)) = flywheel_engine::tick::state_def(defs, &probe, &fired.region) else {
        return vec![];
    };
    let mut names = Vec::new();
    for transition in &state.transitions {
        if transition.to != fired.to {
            continue;
        }
        collect(&transition.when, &mut names);
    }
    names.dedup();
    names
}

/// Every evidence name a guard reads, wherever it sits in the algebra.
fn collect(guard: &flywheel_engine::defs::Guard, into: &mut Vec<String>) {
    use flywheel_engine::defs::Guard;
    match guard {
        Guard::Ev(e) => into.push(e.ev.clone()),
        Guard::All { all } => all.iter().for_each(|g| collect(g, into)),
        Guard::Any { any } => any.iter().for_each(|g| collect(g, into)),
        Guard::Not { not } => collect(not, into),
        _ => {}
    }
}

/// A short, stable name for a body, so drift is reported without pasting two
/// pages into the run record.
fn digest(body: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in body.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

/// What the host does with an effect it is asked to perform.
///
/// A refusal is not a quiet skip: the act did not run, its proof stays absent,
/// and the run record carries the reason under attention (81, 127). The tick
/// carries on either way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Performed {
    /// The act ran. Whether its proof now holds is the proof's question (73).
    Done,
    /// No binding in this release performs it, and here is why.
    Refused(String),
    /// The binding ran and failed. The act did not happen, so its proof stays
    /// absent and the next tick owes it again; the reason is reported and the
    /// tick carries on (81, 127, 148).
    Failed(String),
}

impl Performed {
    pub fn done(&self) -> bool {
        matches!(self, Performed::Done)
    }

    pub fn why(&self) -> &str {
        match self {
            Performed::Done => "",
            Performed::Refused(reason) | Performed::Failed(reason) => reason,
        }
    }
}

/// The effects this release does not perform, each with the reason it does not.
///
/// Every one of them has a proof `profiles/host.yaml` binds to a real place or
/// a real line — a tethered process in a worktree, a declaration file at a
/// place's head, a change directory on the intent's line. Phase 1 records the
/// line-and-place effects rather than performing them (93a), so there is no
/// head and no line for those proofs to read, and the scenarios that would
/// assert them are the ones the proposal defers. Binding them to a fact nothing
/// reads would be the silent done this list exists to prevent (81, 127).
pub const DEFERRED: &[(&str, &str)] = &[
    (
        "declare_services",
        "its proof reads the repository's service data at the bolt's place's head \
         (`host.yaml` bolt.services_declared); phase 1 records places rather than making them \
         (93a), and X09 is deferred with construction",
    ),
    (
        "start_service",
        "its proof is `wt tether status` in the bolt's place (`host.yaml` service.process); \
         the multiplexer is phase 2 and X09 is deferred with it",
    ),
    (
        "stop_service",
        "its proof is `wt tether status` reporting no tether (`host.yaml` \
         service.process_absent); the multiplexer is phase 2 and X09 is deferred with it",
    ),
    (
        "open_intent",
        "its proof is `openspec/changes/<intent-id>/` on the intent's line (`host.yaml` \
         intent.change_open); phase 1 records lines rather than taking them (93a), and S34 \
         and X03 are deferred with construction",
    ),
];

/// One of an effect's arguments, as text.
fn arg(effect: &PlannedEffect, name: &str) -> String {
    effect
        .args
        .get(name)
        .and_then(|v| match v {
            Value::String(text) => Some(text.clone()),
            other => Some(other.to_string()),
        })
        .unwrap_or_default()
}

/// Perform one effect through the bindings this release carries.
///
/// The workspace binding in force, for one act of 42: the recorded facts
/// alone, or real branches and worktrees under the host's root that write the
/// same facts (93a, D8). One body, so an arm cannot pick the wrong one.
fn with_workspace<T>(
    store: &mut HostStore,
    act: impl FnOnce(&mut dyn Workspace) -> Result<T>,
) -> Result<T> {
    match (store.workspace.as_str(), store.root.clone()) {
        ("host", Some(root)) => {
            let mut workspace = flywheel_workspace_host::HostWorkspace::new(&mut store.git, root);
            act(&mut workspace)
        }
        ("host", None) => bail!("workspace `host` is bound and this host has no root on disk (205)"),
        _ => {
            let mut workspace = RecordedWorkspace::new(&mut store.git);
            act(&mut workspace)
        }
    }
}

/// An effect no binding covers is refused with its reason, and one whose
/// binding fails says so: either way the act did not happen, its proof stays
/// absent, it is still owed on the next tick, and the run record carries the
/// reason under attention (81, 127, tasks 16.1, 16.2).
#[allow(clippy::too_many_arguments)]
pub fn perform(
    defs: &Definitions,
    store: &mut HostStore,
    sinks: &mut Sinks,
    host: &str,
    now: DateTime<Utc>,
    object: &str,
    region: &str,
    effect: &PlannedEffect,
) -> Performed {
    match performing(defs, store, sinks, host, now, object, region, effect) {
        Ok(performed) => performed,
        // A binding that failed is not a binding that succeeded: 81 asks the
        // machinery to report a problem with itself, and 127 leaves the proof
        // where it was, so the act is attempted again.
        Err(why) => Performed::Failed(format!("{why:#}")),
    }
}

#[allow(clippy::too_many_arguments)]
fn performing(
    defs: &Definitions,
    store: &mut HostStore,
    sinks: &mut Sinks,
    host: &str,
    now: DateTime<Utc>,
    object: &str,
    region: &str,
    effect: &PlannedEffect,
) -> Result<Performed> {
    // An effect may write the signal material, so what was read of it is read
    // again when next asked (111, 203).
    store.material.borrow_mut().take();
    let place = place_key(object, region);
    let kind = store
        .get(object)
        .ok()
        .flatten()
        .and_then(|o| o.record.get("type").and_then(|v| v.as_str()).map(String::from));
    let stem = session_stem(object, region, kind.as_deref());
    // The one running, for everything but a start; a start takes a fresh
    // attempt when the last is over (`session.yaml` id, 150).
    let session = flywheel_sessions_operator::current(&store.git, &stem);
    match effect.name.as_str() {
        // ---- the workspace, recorded (93a)
        "create_line" => {
            let parent = store
                .get(object)
                .ok()
                .flatten()
                .and_then(|o| o.parent)
                .unwrap_or_default();
            with_workspace(store, |ws| ws.create_line(object, &parent))?;
        }
        "take_parent" => {
            with_workspace(store, |ws| ws.take_parent(object))?;
        }
        "remove_line" => {
            with_workspace(store, |ws| ws.remove_line(object))?;
        }
        "land_line" => {
            with_workspace(store, |ws| ws.land_line(object, LandingPolicy::Direct))?;
        }
        "write_acceptance" => {
            with_workspace(store, |ws| ws.write_acceptance(object, ""))?;
        }
        "prepare_place" => {
            let order = work_order(defs, store, &session, &place, object)?;
            with_workspace(store, |ws| ws.prepare_place(&place, object, &order.body))?;
        }
        "rebase_place" => {
            with_workspace(store, |ws| ws.rebase_place(&place))?;
        }
        "merge_place" => {
            with_workspace(store, |ws| ws.merge_place(&place))?;
        }
        "remove_place" => {
            with_workspace(store, |ws| ws.remove_place(&place))?;
        }
        // ---- the sessions, with the operator as the session (93b)
        "start_session" => {
            let fresh = flywheel_sessions_operator::next_attempt(&store.git, &stem);
            let order = work_order(defs, store, &fresh, &place, object)?;
            // The order in the place, named for this session: `prepare_place`
            // wrote one before the stage's session had a name, and the one the
            // agent reads must say which session to report as (67, 89).
            if let Some(dir) = place_dir_of(store, &place) {
                let file = dir.join(".flywheel").join("work-order.md");
                if let Some(parent) = file.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                std::fs::write(&file, &order.body)
                    .with_context(|| format!("writing the work order at {}", file.display()))?;
            }
            match store.sessions.as_str() {
                "herdr" => {
                    let placement = placement_of(store, object, &place)?;
                    flywheel_sessions_herdr::start(
                        &mut store.git,
                        &flywheel_sessions_herdr::Herdr::default(),
                        host,
                        now,
                        &order,
                        &placement,
                    )?;
                }
                _ => flywheel_sessions_operator::start(&mut store.git, host, now, &order)?,
            }
        }
        "end_session" => match store.sessions.as_str() {
            "herdr" => flywheel_sessions_herdr::end(&mut store.git, &flywheel_sessions_herdr::Herdr::default(), &session, now)?,
            _ => flywheel_sessions_operator::end(&mut store.git, &session, now)?,
        },
        "deliver_answer" => {
            let text = effect
                .args
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            match store.sessions.as_str() {
                "herdr" => flywheel_sessions_herdr::answer(&mut store.git, &flywheel_sessions_herdr::Herdr::default(), &session, host, now, &text)?,
                _ => flywheel_sessions_operator::answer(&mut store.git, &session, host, now, &text)?,
            }
        }
        "tell_moved" => match store.sessions.as_str() {
            "herdr" => flywheel_sessions_herdr::moved(&mut store.git, &flywheel_sessions_herdr::Herdr::default(), &session, host, now)?,
            _ => flywheel_sessions_operator::moved(&mut store.git, &session, host, now)?,
        },
        // One record per uncited offer on the session's thread, pointing at its
        // document; the record never holds the text and the session is not
        // interrupted (58, 62, `atoms.yaml` record_offers).
        "record_offers" => {
            let HostStore { git, world, .. } = store;
            flywheel_domain::offers::record(git, &mut **world, defs, &session, object, now)?;
        }
        // Every refusal the session wrote is carried to the run record, where
        // the report effect beside it takes it to attention (43,
        // `record-derived.yaml` record_refusals).
        "record_refusals" => {
            store.refuse_from_session(&session, host, now);
        }
        // A pin whose offer's record has ended leaves the git host and this
        // host's clone; each removal is on the run record with the revision it
        // held and the record that ended (55, 62, `host.yaml`
        // remove_stale_offer_pins).
        "remove_stale_offer_pins" => {
            let stale = flywheel_domain::offers::stale_pins(&store.git, &*store.world, defs)?;
            let mut removed = Vec::new();
            for pin in &stale {
                store.world.unpin(&pin.repository, &pin.reference, &pin.revision)?;
                removed.push(
                    RunEntry::new(now, host, "write", object, "an offer's pin whose record has ended is removed")
                        .with("repository", &pin.repository)
                        .with("pin", &pin.reference)
                        .with("revision", &pin.revision)
                        .with("ended", pin.record.as_deref().unwrap_or("none: the offer made nothing")),
                );
            }
            if !removed.is_empty() {
                store.git.append_run(&removed)?;
            }
        }

        // ---- the acts whose whole work is records (`flywheel-domain::effects`)
        //
        // Each is the same act in every profile, so it is written once over the
        // state store and the host and the conformance runner perform one
        // implementation (125, D3, task 16.1).
        "create_items" => {
            flywheel_domain::effects::create_items(&mut store.git, defs, object, now)?;
        }
        "create_bolt" => {
            let name = arg(effect, "name");
            let repository = arg(effect, "repository");
            flywheel_domain::effects::create_bolt(
                &mut store.git,
                defs,
                object,
                &name,
                &repository,
                now,
            )?;
        }
        "route_unit" => {
            let bolt = arg(effect, "bolt");
            flywheel_domain::effects::route_unit(&mut store.git, object, &bolt)?;
        }
        "rename_bolt" => {
            let name = arg(effect, "name");
            flywheel_domain::effects::rename_bolt(&mut store.git, object, &name)?;
        }
        "set_type" => {
            let kind = arg(effect, "type");
            flywheel_domain::effects::set_type(&mut store.git, object, &kind)?;
        }
        "propose_elaboration" => {
            let kind = arg(effect, "type");
            flywheel_domain::effects::propose_elaboration(
                &mut store.git,
                defs,
                object,
                &kind,
                now,
            )?;
        }
        "split_intent" => {
            let partition = arg(effect, "partition");
            let _ =
                flywheel_domain::effects::split_intent(&mut store.git, defs, object, &partition, now);
        }
        "gather_elaborations" => {
            let gatherings =
                flywheel_domain::effects::gatherings_of(&store.git, &session).unwrap_or_default();
            let _ =
                flywheel_domain::effects::gather_elaborations(&mut store.git, defs, &gatherings, now);
        }
        // For each source this host declares whose cadence fired, run its
        // enumerator: one keyed capture per source event, and the same key
        // twice writes nothing (111, 215, 231). The manifest of this release
        // declares none, so a host with no source runs none.
        "run_adapters" => {
            let sources = store.sources_declared(host);
            let HostStore { git, world, .. } = store;
            for source in &sources {
                flywheel_domain::adapters::run(git, &mut **world, defs, source, host, now)?;
            }
        }
        // The leases a gone host held are released, and the sessions it was
        // running are closed: the host that takes over starts a fresh attempt,
        // and the returning host reads that it lost (150, `session.yaml` id).
        "expire_leases" => {
            let gone = object.strip_prefix("host/").unwrap_or(object).to_string();
            let objects = store.list_records(&Scope::All).unwrap_or_default();
            for held in &objects {
                let Ok(Some(lease)) = store.git.leases(&held.id) else {
                    continue;
                };
                if lease.holder != gone {
                    continue;
                }
                let was = lease.state.clone();
                store.git.lease(&LeaseOp::Release {
                    object: held.id.clone(),
                    holder: gone.clone(),
                })?;
                store.git.lease(&LeaseOp::Mark {
                    object: held.id.clone(),
                    state: "expired".into(),
                })?;
                // The lease reached `expired` because this effect put it
                // there, which is the rule the machine names for work a session
                // is behind (150, `lease.yaml` stale). A move nothing recorded
                // would be a move no reader could find (79, 167).
                store.marked.borrow_mut().push((
                    flywheel_domain::leases::id_for(&held.id),
                    was,
                    "expired".to_string(),
                ));
            }
            for session in flywheel_sessions_operator::of_host(&store.git, &gone) {
                flywheel_sessions_operator::take_over(&mut store.git, &session, host, now)?;
            }
        }
        // A capture that is its own excerpt — the page's box, a forwarded
        // message — writes its one signal here, and never a second: no
        // judgment is involved (19, 112, `atoms.yaml` ensure_signal).
        "ensure_signal" => {
            let by = store
                .get(object)
                .ok()
                .flatten()
                .and_then(|o| o.record.get("captured_by").and_then(|v| v.as_str()).map(String::from))
                .unwrap_or_else(|| "operator".to_string());
            flywheel_domain::signals::ensure_signal(
                &mut store.git,
                &mut *store.world,
                defs,
                object,
                &by,
                now,
            )?;
        }
        // Every judged signal gets its one standing move, and each join becomes
        // or grows a proposed intent (107, 109, 116, `curation.yaml` applying).
        //
        // The moves are what the curation session delivered, and in this phase
        // that session is the operator working on the page (93b, D16): the
        // curator's surface writes one move record per signal it judged and
        // reports the exit, and this reads those records back. Applying a move
        // already applied writes the same bytes and cites the same signal, so a
        // second pass changes nothing (127, 137).
        "record_moves" => {
            // What the curator session delivered, parsed onto its thread, beside
            // the moves the operator's surface wrote into the blueprints (107,
            // 116, 93b).
            let session = flywheel_domain::offers::newest_session(&store.git, object)?;
            let HostStore { git, world, .. } = store;
            let mut delivered = flywheel_domain::signals::moves(&flywheel_domain::signals::snapshot(&**world));
            if let Some(session) = &session {
                delivered.extend(flywheel_domain::offers::delivered(git, session)?.0);
            }
            flywheel_domain::signals::record_moves(git, &mut **world, &delivered, now)?;
        }
        // Curation never opens an intent: what its joins make stands as a
        // proposal on the rail and becomes work only on the operator's
        // response (20, 110, 5).
        "propose_intents" => {
            // One proposed intent per join and per proposal the curator stated,
            // with its subject and the elaborations to work it (109, 188).
            let session = flywheel_domain::offers::newest_session(&store.git, object)?;
            let HostStore { git, world, .. } = store;
            let moves = flywheel_domain::signals::moves(&flywheel_domain::signals::snapshot(&**world));
            let stated = match &session {
                Some(session) => flywheel_domain::offers::delivered(git, session)?.1,
                None => vec![],
            };
            let proposals = flywheel_domain::offers::proposals_with(&moves, &stated);
            flywheel_domain::signals::propose_intents(git, defs, &proposals, now)?;
        }
        // Every signal a dropped intent cited keeps a move naming the drop, and
        // they are not clustered again unless new signals join them (117).
        "drop_signals" => {
            let reason = effect
                .args
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("intent dropped")
                .to_string();
            let HostStore { git, world, .. } = store;
            flywheel_domain::signals::drop_signals(
                git,
                &mut **world,
                defs,
                object,
                &reason,
                now,
            )?;
        }
        // The sink delivers: the numbered decisions routed here, the tail since
        // its mark and the notices, with the mark advancing in the same write
        // (14, 18, 82, `surfaces.yaml` effects.deliver_rail).
        //
        // Only the sink's presenter delivers, which `Chat::deliver` reads from
        // the lease, so the operator sees each delivery once (148). A sink this
        // host loaded no channel for is one it does not present, and the
        // effect's proof stays absent so another host's tick performs it (127,
        // 148).
        "deliver_rail" => return Ok(deliver(defs, store, sinks, host, object)),
        // The status projection is the rail's own effect (D12); the tick writes
        // it after every pass, so nothing to do here.
        "render_status" => {}
        // The OpenSpec archive at the item's merge (`host.yaml` archive_change):
        // nothing to do, and the proof holds at once, for a type that needs no
        // change directory (chore). A type that does needs a place to run
        // `openspec archive` in, which the workspace binding with real places
        // is what provides; until then the act is refused with that reason.
        "archive_change" => {
            let kind = kind.clone().or_else(|| {
                store
                    .get(object)
                    .ok()
                    .flatten()
                    .and_then(|o| o.parent)
                    .and_then(|unit| store.get(&unit).ok().flatten())
                    .and_then(|u| u.record.get("type").and_then(|v| v.as_str()).map(String::from))
            });
            match kind {
                Some(kind) if !flywheel_domain::stages::needs_change_directory(defs, &kind) => {}
                Some(kind) => {
                    return Ok(Performed::Refused(format!(
                        "`archive_change` runs `openspec archive` in the item's place, and the `{kind}` type needs one; this release has no place to run it in"
                    )))
                }
                None => return Ok(Performed::Refused("`archive_change`: the item names no unit type".into())),
            }
        }
        // No binding covers it. It did not happen, whatever the machines
        // expected of it (81, 127).
        other => {
            let reason = DEFERRED
                .iter()
                .find(|(name, _)| *name == other)
                .map(|(_, why)| (*why).to_string())
                .unwrap_or_else(|| {
                    format!("no binding in this release performs `{other}`")
                });
            return Ok(Performed::Refused(reason));
        }
    }
    Ok(Performed::Done)
}

/// Deliver to one sink, through the channel this host loaded for it (D8, D9).
///
/// The channel is taken out of the binding for the call and put back, because a
/// `Chat` holds its channel: nothing is copied and nothing about the sink lives
/// in two places.
fn deliver(
    defs: &Definitions,
    store: &mut HostStore,
    sinks: &mut Sinks,
    host: &str,
    sink: &str,
) -> Performed {
    let Some(channel) = sinks.channels.remove(sink) else {
        // No channel bound: this host presents nothing here, so nothing was
        // delivered and the proof stays absent for another host's tick, which
        // is not a refusal (148, 127).
        return Performed::Refused(String::new());
    };
    let mut chat = Chat::new(sink, host, &sinks.address, channel);
    let delivered = chat.deliver(store, defs);
    sinks.channels.insert(sink.to_string(), chat.channel);
    match delivered {
        Ok(Some(_)) => Performed::Done,
        // Another host holds the sink's lease and delivers (148).
        Ok(None) => Performed::Refused(String::new()),
        // The channel failed: nothing reached the platform, the mark stays
        // where it was, and the reason is reported under attention (81, 127).
        Err(e) => Performed::Failed(format!("{e:#}")),
    }
}

/// The Discord channel a sink's manifest entry names (D9): its channel the
/// entry's surface, its token read with `read` from the variable the entry
/// names (204, 207), and its API Discord's own unless `api` says otherwise.
///
/// A response given there is recorded as given by the sink's member, or by the
/// instance's operator where the sink is a shared channel's (D10, 153).
pub fn discord_for(
    name: &str,
    entry: &flywheel_world_host::manifest::Sink,
    read: &dyn Fn(&str) -> Option<String>,
    api: Option<&str>,
    operator: Option<&str>,
) -> Result<flywheel_surface::discord::Discord> {
    use flywheel_surface::discord::{Discord, Settings, Token};
    let id = flywheel_domain::sinks::id_for(name);
    let channel = entry.surface.trim().parse::<u64>().with_context(|| {
        format!(
            "flywheel.yaml sinks.{name}.surface: `{}` is no Discord channel's id",
            entry.surface
        )
    })?;
    let token = Token::from_variable(&id, &entry.token_from, read)?;
    Discord::open(
        Settings {
            sink: id,
            channel,
            member: entry.member.clone().or_else(|| operator.map(String::from)),
            api: api.map(String::from),
        },
        token,
    )
}

/// The work order a session is given: the closed set of inputs, rendered, and
/// the name it reports under (88, 89).
/// The worktree of a place, where the workspace binding made one (93a). A
/// session's region names the object, and the place the session works in is
/// the object's own — `<object>#own`, the key its `place` region is under
/// (`regions::place_key`) — so both are tried.
fn place_dir_of(store: &HostStore, place: &str) -> Option<std::path::PathBuf> {
    let own = format!("{}#own", place.trim_end_matches("#own"));
    [place.to_string(), own].iter().find_map(|key| {
        store
            .get(&flywheel_workspace_recorded::place_fact(key))
            .ok()
            .flatten()
            .and_then(|f| f.record.get("dir").and_then(|v| v.as_str()).map(std::path::PathBuf::from))
            .filter(|d| d.join(".git").exists())
    })
}

/// Where a session's pane goes and what runs in it: the workspace for the
/// bolt or the intent above the object, the tab for the unit or the
/// elaboration, the place's own directory, and the agent kind the manifest's
/// role gives (174, 196, 173; `sessions.yaml` layout). Everything is read
/// from the records; nothing is remembered.
fn placement_of(store: &HostStore, object: &str, place: &str) -> Result<flywheel_sessions_herdr::Placement> {
    let Some(cwd) = place_dir_of(store, place) else {
        bail!(
            "`{place}` has no worktree on disk to start a session in; a pane runs in a real place, \
             which the `host` workspace binding makes (93a, 196)"
        );
    };
    // The chain above the object: the nearest bolt or intent names the
    // workspace; the object's parent, or itself, names the tab.
    let mut chain = vec![object.to_string()];
    let mut at = object.to_string();
    while let Some(parent) = store.get(&at).ok().flatten().and_then(|o| o.parent) {
        chain.push(parent.clone());
        at = parent;
        if chain.len() > 8 {
            break;
        }
    }
    let workspace_label = chain
        .iter()
        .find(|id| id.starts_with("bolt/") || id.starts_with("intent/"))
        .cloned()
        .unwrap_or_else(|| chain.last().cloned().unwrap_or_default());
    let tab_label = chain.get(1).cloned().filter(|p| p != &workspace_label).unwrap_or_else(|| object.to_string());
    Ok(flywheel_sessions_herdr::Placement {
        workspace_label,
        tab_label,
        cwd,
        kind: "claude".into(),
    })
}

/// The closed set of inputs a session is handed (88, 89), rendered as the
/// file `prepare_place` writes into the place: the job in the operator's own
/// words, what to deliver, the exact command to report with, the rules a
/// session never crosses, and the agent's definition and skill from the
/// shipped instruction set (119, 190). Nothing here is state; it is what the
/// session reads (67).
pub(crate) fn work_order(
    defs: &Definitions,
    store: &HostStore,
    session: &str,
    place: &str,
    object: &str,
) -> Result<WorkOrder> {
    let state = store.git.repo.dir.display().to_string();
    let host = store.reading.me.clone();
    let held = store.get(object).ok().flatten();
    let kind = held
        .as_ref()
        .map(|o| o.machine.clone())
        .unwrap_or_else(|| "work".into());
    let field = |name: &str| -> Option<String> {
        held.as_ref()
            .and_then(|o| o.record.get(name))
            .and_then(|v| v.as_str().map(String::from))
    };
    // The unit above a work item carries the job; the item carries its type.
    let unit = held
        .as_ref()
        .filter(|o| o.machine == "work-item")
        .and_then(|o| o.parent.clone())
        .and_then(|p| store.get(&p).ok().flatten());
    let unit_field = |name: &str| -> Option<String> {
        unit.as_ref()
            .and_then(|u| u.record.get(name))
            .and_then(|v| v.as_str().map(String::from))
    };
    // A chore a session offered carries no words of its own: its job is the
    // document the offer pointed at, as it stood at the offer's revision (62,
    // 89; chore@2 params.job).
    let offered = match unit.as_ref().or(held.as_ref()) {
        Some(chore) => offered_document(store, chore)?,
        None => None,
    };
    let job = offered
        .or_else(|| field("subject"))
        .or_else(|| unit_field("subject"))
        .or_else(|| field("title"))
        .unwrap_or_default();
    let stage = session_stage(session);
    let agent = kind_of_agent(defs, store, object, held.as_ref(), stage.as_deref());
    // Curation's deliverables are the row's, its moves and its intent proposals
    // (context.yaml sessions.curation); every other session's are its stage's.
    let deliverables = match kind.as_str() {
        "curation" => flywheel_domain::offers::PARSED.iter().map(|d| d.to_string()).collect(),
        _ => stage_deliverables(defs, store, held.as_ref(), stage.as_deref()),
    };

    let mut body = String::new();
    body.push_str(&format!("# work order · {session}\n\n"));
    body.push_str("## the job\n\n");
    if !job.is_empty() {
        body.push_str(&format!("{job}\n\n"));
    }
    body.push_str(&format!("object: {object}\nplace: {place}\nsession: {session}\n"));
    if let Some(stage) = &stage {
        body.push_str(&format!("stage: {stage}\n"));
    }
    if let Some(object) = &held {
        for (name, value) in &object.record {
            if !matches!(name.as_str(), "subject" | "title") {
                body.push_str(&format!("{name}: {value}\n"));
            }
        }
    }
    // A curation session's job is the signals nothing has moved, read against
    // the standing claims and the open intents (model.md §9, 110).
    if kind == "curation" {
        let open: Vec<(String, String, usize)> = store
            .list_records(&Scope::All)?
            .into_iter()
            .filter(|o| o.machine == "intent" && o.config.get("life").map(String::as_str) == Some("open"))
            .map(|o| {
                let subject = ["subject", "title"]
                    .iter()
                    .find_map(|f| o.record.get(*f).and_then(|v| v.as_str()).map(String::from))
                    .unwrap_or_default();
                let held = o.record.get("signals").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
                (o.id, subject, held)
            })
            .collect();
        let mut repositories: Vec<String> = store
            .world
            .repositories()
            .unwrap_or_default()
            .into_iter()
            .map(|r| r.name)
            .filter(|name| name != "flywheel-state" && name != "flywheel-blueprints")
            .collect();
        repositories.push(flywheel_domain::offers::BLUEPRINTS.to_string());
        let inputs = flywheel_domain::order::Curation {
            signals: flywheel_domain::signals::unmoved(&*store.material()),
            claims: flywheel_domain::changes::standing_claims(&flywheel_domain::signals::Blueprints(&*store.world)),
            intents: open,
            types: flywheel_domain::blueprints::elaboration_types(),
            repositories,
        };
        body.push('\n');
        body.push_str(&flywheel_domain::order::curation(&inputs));
    }
    body.push_str("\n## what to deliver\n\n");
    match deliverables.is_empty() {
        true => body.push_str("what the job asks for, committed in this place\n"),
        false => {
            for d in &deliverables {
                body.push_str(&format!("- {d}\n"));
            }
        }
    }
    let named: Vec<String> = match deliverables.is_empty() {
        true => vec!["commits".into()],
        false => deliverables.clone(),
    };
    // The exact command, with the two things it reads from the environment: the
    // session it reports on and this host's checkout of the state repository
    // (67, 89, 92).
    // The binary this host runs, by its full path: a session's shell need not
    // have it on the path, and the order is exact (67, 89).
    let flywheel = std::env::current_exe()
        .ok()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "flywheel".into());
    let manifest = store
        .manifest
        .as_ref()
        .map(|m| std::fs::canonicalize(m).unwrap_or_else(|_| m.clone()));
    body.push_str(&how_to_report(&Reporting {
        session,
        state: &state,
        manifest: manifest.as_deref(),
        flywheel: &flywheel,
        host: &host,
        deliverables: &named,
    }));
    body.push_str("\n## rules\n\n");
    body.push_str(
        "Commit in this place, on the branch you are on. Never create, merge, rebase, push or land a \
         branch, never touch another place or the state repository, and never message another \
         session: those are the machinery's (43, 197, I12).\n",
    );
    if let Some((definition, skill)) = agent.as_deref().and_then(agent_text) {
        body.push_str(&format!("\n## the agent · {}\n\n{definition}\n", agent.as_deref().unwrap_or_default()));
        body.push_str(&format!("\n## the skill\n\n{skill}\n"));
    }
    Ok(WorkOrder {
        session: session.to_string(),
        kind,
        place: place.to_string(),
        body,
    })
}

/// The document a chore was offered with, as it stood at the offer's revision,
/// or none for anything that is not a chore made of an offer.
///
/// It is read from the git host's pin in the repository the offer was made in
/// — the offering session's place's, found through the entry the chore came
/// from — since a chore has no change directory and its session may stand on
/// another repository's line or another host (62, 89, 232; `host.yaml`
/// prepare_place).
fn offered_document(store: &HostStore, chore: &Object) -> Result<Option<String>> {
    let text = |name: &str| chore.record.get(name).and_then(|v| v.as_str());
    if text("type") != Some("chore") {
        return Ok(None);
    }
    let (Some(document), Some(revision)) = (text("document"), text("revision")) else {
        return Ok(None);
    };
    let Some(entry) = chore
        .record
        .get("sources")
        .and_then(|v| v.as_array())
        .and_then(|sources| sources.first())
        .and_then(|v| v.as_str())
    else {
        return Ok(None);
    };
    let session = entry.rsplit_once('#').map(|(s, _)| s).unwrap_or(entry);
    let Some(owner) = flywheel_domain::offers::owner_of(&store.git, session)? else {
        bail!(
            "`{document}` was offered by `{session}`, which runs under nothing on record, so the \
             repository it was offered in is unknown (62)"
        );
    };
    let repository = flywheel_domain::regions::repository_of(&store.git, &owner)?;
    let pin = flywheel_domain::offers::pin_of(entry);
    let Some(body) = store.world.read_pinned(&repository, &pin, revision, document)? else {
        bail!("`{document}` is not in {repository} at {revision}, the revision its offer names (62)");
    };
    Ok(Some(String::from_utf8_lossy(&body).into_owned()))
}

/// What a session's report commands are made of: the session, this host's
/// checkout of the state repository, the manifest, the page, the binary by its
/// full path and the host (67, 89).
pub(crate) struct Reporting<'a> {
    pub session: &'a str,
    pub state: &'a str,
    pub manifest: Option<&'a Path>,
    pub flywheel: &'a str,
    pub host: &'a str,
    pub deliverables: &'a [String],
}

/// The order's `how to report`: the exact command for every report the session
/// may make, from its place (65, 67, 89).
///
/// Every session may offer what is outside its job — a finding, a small
/// necessary fix as a chore, or a signal about neither its intent nor its bolt
/// — so every order gives the offer's command, naming the three, with
/// the manifest a chore's scope is checked against (58, 60, 62, `sessions.yaml`
/// commands.offer). The ask is the curation session's and the operator's own
/// session's, so theirs alone carry its command (116, 197, `sessions.yaml`
/// commands.ask).
pub(crate) fn how_to_report(r: &Reporting) -> String {
    let Reporting { session, state, flywheel, host, .. } = r;
    let env = format!("FLYWHEEL_SESSION={session} FLYWHEEL_STATE={state}");
    let manifest = r
        .manifest
        .map(|m| format!(" FLYWHEEL_MANIFEST={}", m.display()))
        .unwrap_or_default();
    let mut body = String::from("\n## how to report\n\n");
    body.push_str("When the work is done, from this directory:\n\n");
    body.push_str(&format!(
        "    {env} {flywheel} exit done{} --host {host}\n\n",
        r.deliverables.iter().map(|d| format!(" --deliverable {d}")).collect::<String>()
    ));
    body.push_str("When you cannot go on without the operator's answer:\n\n");
    body.push_str(&format!(
        "    {env} {flywheel} exit blocked --question \"<the question>\" --host {host}\n\n"
    ));
    body.push_str(
        "To offer what is outside the job, pointing at a document you committed here: a finding when it \
         is about the intent or bolt you work under, a chore when it is a small necessary fix, and a \
         signal when what you saw is about neither your intent nor your bolt. --about names what it \
         concerns. A chore says where its fix belongs with --scope: bolt-line, which a session under a \
         bolt may leave off, or a repository the instance tracks, blueprints among them:\n\n",
    );
    body.push_str(&format!(
        "    {env}{manifest} {flywheel} offer finding|chore|signal --document <path> --about <object> [--scope bolt-line|<repository>] --host {host}\n\n"
    ));
    if flywheel_domain::asks::granted(session) {
        body.push_str(
            "To ask for something to be done in a repository; it prints the ask's id, which the \
             signal's route move names:\n\n",
        );
        body.push_str(&format!(
            "    {env}{manifest} {flywheel} ask <repository> \"<the words>\" --host {host}\n\n"
        ));
    }
    body.push_str("The machinery reads the report and nothing else you leave here; what you leave here is your work (66, 67).\n");
    body
}

/// The stage a session id names, where it names one: `<item>/<stage>/<n>`.
fn session_stage(session: &str) -> Option<String> {
    let stem = flywheel_domain::regions::stem_of(session);
    stem.rsplit('/').next().map(String::from).filter(|s| !s.is_empty())
}

/// The agent the stage names for a work item's session, from the type's
/// definition (`stage.yaml` params.agents); for anything else, the last
/// segment of the session's stem, which is the type or the agent
/// (`session.yaml` id).
fn kind_of_agent(
    defs: &Definitions,
    store: &HostStore,
    _object: &str,
    held: Option<&Object>,
    stage: Option<&str>,
) -> Option<String> {
    let held = held?;
    if held.machine == "work-item" {
        let kind = flywheel_domain::stages::type_of(&store.git, held)?;
        let state = flywheel_domain::stages::stage_state(defs, &kind, stage?)?;
        return state
            .params
            .as_ref()
            .and_then(|p| p.get("agents"))
            .and_then(|a| a.as_array())
            .and_then(|a| a.first())
            .and_then(|a| a.as_str().map(String::from).or_else(|| a.get("name").and_then(|n| n.as_str()).map(String::from)));
    }
    stage.map(String::from)
}

/// What the stage asks for, by name (`stage.yaml` params.deliverables).
fn stage_deliverables(defs: &Definitions, store: &HostStore, held: Option<&Object>, stage: Option<&str>) -> Vec<String> {
    let Some(held) = held.filter(|o| o.machine == "work-item") else {
        return vec![];
    };
    let Some(kind) = flywheel_domain::stages::type_of(&store.git, held) else {
        return vec![];
    };
    let Some(state) = stage.and_then(|s| flywheel_domain::stages::stage_state(defs, &kind, s)) else {
        return vec![];
    };
    state
        .params
        .as_ref()
        .and_then(|p| p.get("deliverables"))
        .and_then(|d| d.as_array())
        .map(|d| d.iter().filter_map(|x| x.get("name").and_then(|n| n.as_str()).map(String::from)).collect())
        .unwrap_or_default()
}

/// The agent's definition and skill from the shipped instruction set (119,
/// 190; `instructions/set.yaml` resolution). None where the set has no such
/// agent.
fn agent_text(agent: &str) -> Option<(String, String)> {
    static SET: std::sync::OnceLock<Option<flywheel_domain::instructions::Instructions>> = std::sync::OnceLock::new();
    let set = SET.get_or_init(|| flywheel_domain::instructions::Instructions::shipped().ok()).as_ref()?;
    let skill = set.at(&format!("flywheel/skills/{agent}/SKILL.md"))?.body.clone();
    let definition = set
        .at(&format!("flywheel/agents/{agent}.md"))
        .map(|f| f.body.clone())
        .unwrap_or_default();
    Some((definition, skill))
}

/// Whether a session is one this host is running, for the bound (31, 32).
pub fn running(store: &HostStore, session: &str) -> bool {
    flywheel_sessions_operator::running(&store.git, session)
}

/// A session's presence, as the machinery observes it (72, 196).
pub fn presence(store: &HostStore, session: &str) -> SessionPresence {
    match flywheel_sessions_operator::running(&store.git, session) {
        true => SessionPresence::Alive,
        false => SessionPresence::Absent,
    }
}
