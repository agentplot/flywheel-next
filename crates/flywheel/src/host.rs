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
use std::path::Path;

/// How often the sweep runs, whatever else happens (D7, 130).
pub const SWEEP: i64 = 60;

/// How often a laptop asks the git host whether anything moved: one round trip,
/// no history read (D6).
pub const POLL: i64 = 30;

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
        if bindings.workspace != "recorded" {
            bail!(
                "flywheel.yaml hosts.{host}.workspace: `{}` — this release binds `recorded` \
                 alone; the workspace over `wt` and git is phase 2 (93a, D8)",
                bindings.workspace
            );
        }
        if bindings.sessions != "operator" {
            bail!(
                "flywheel.yaml hosts.{host}.sessions: `{}` — this release binds `operator` \
                 alone; the runner and the multiplexer are phase 2 (93b, D8)",
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
    pub world: Box<dyn World>,
    pub reading: Reading,
    /// Every operation this store was asked for, in order. The run record and
    /// the fetch-first proof read it (79, 165).
    pub trace: RefCell<Vec<String>>,
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
        }
    }

    fn saw(&self, what: &str) {
        self.trace.borrow_mut().push(what.to_string());
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
        let mut world: Box<dyn World> =
            std::mem::replace(&mut self.world, Box::new(flywheel_scenario::bindings::FilesWorld::new()));
        let out = act(self, &mut *world);
        self.world = world;
        out
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
        flywheel_domain::derived::evidence(&self.git, &self.reading, object, name)
            // The signal material, read from the blueprints under the
            // machinery's prefix (111, 107, `blueprints.yaml` evidence).
            .or_else(|| {
                flywheel_domain::signals::evidence(
                    &flywheel_domain::signals::Blueprints(&*self.world),
                    object,
                    name,
                )
            })
            .or_else(|| self.git.evidence(object, region, name))
            .or_else(|| {
                flywheel_workspace_recorded::evidence(
                    &self.git,
                    &place_key(object, region),
                    name,
                )
            })
            .or_else(|| flywheel_workspace_recorded::evidence(&self.git, object, name))
            .or_else(|| {
                flywheel_sessions_operator::evidence(
                    &self.git,
                    &flywheel_sessions_operator::current(
                        &self.git,
                        &session_stem(object, region, self.kind_of(object).as_deref()),
                    ),
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
    pub channels: std::collections::BTreeMap<String, Box<dyn Channel>>,
}

impl Sinks {
    pub fn at(address: &str) -> Sinks {
        Sinks {
            address: address.to_string(),
            channels: Default::default(),
        }
    }

    /// Load the channel one sink delivers through.
    pub fn bind(&mut self, sink: &str, channel: Box<dyn Channel>) {
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
    /// The run record this tick is building (79-82).
    pub run: Vec<RunEntry>,
    pub last_sweep: Option<DateTime<Utc>>,
    pub last_point: ReadPoint,
    /// An away host's leases stand and its clocks pause; it writes no heartbeat
    /// while it is away (150a).
    pub away_since: Option<DateTime<Utc>>,
    /// The sinks this host presents (148, D9).
    pub sinks: Sinks,
}

impl Host {
    /// Open a host on the manifest's terms. Nothing here starts the loop.
    pub fn open(manifest: &Path, name: &str, now: DateTime<Utc>) -> Result<Host> {
        let read = Manifest::read(manifest)?;
        let bindings = Bindings::read(&read, name)?;
        let world = HostWorld::open(read.clone(), name)?;
        let entry = read.host(name)?;
        let git = GitStore::open(
            &world.bare("flywheel-state"),
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
        let mut host = Host::over(
            name,
            &read.instance,
            flywheel_domain::set::load()?,
            git,
            bindings,
            declaration,
            now,
        );
        // The host's one address, from the router the manifest names: every
        // link a delivery carries is written at it (191, 205a, D10a).
        host.sinks.address = world.address_of(name)?;
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
        Host {
            store,
            name: name.to_string(),
            instance: instance.to_string(),
            defs,
            bindings,
            declaration,
            bound: 4,
            intermittent: true,
            run: vec![],
            last_sweep: None,
            last_point: ReadPoint {
                mark: String::new(),
                seq: 0,
                at: now,
            },
            away_since: None,
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
        self.store.git.heartbeat(self.bound, self.intermittent)?;
        let now = self.now();
        let id = format!("host/{}", self.name);
        let held = self.store.get(&id)?;
        let seq = held.as_ref().map(|o| o.seq).unwrap_or(0);
        let mut record = held.map(|o| o.record).unwrap_or_default();
        record.insert("bound".into(), json!(self.bound));
        record.insert("intermittent".into(), json!(self.intermittent));
        record.insert("last_seen".into(), json!(now.to_rfc3339()));
        record.insert(
            "declares".into(),
            json!({
                "repositories": self.declaration.repositories,
                "types": self.declaration.types,
                "kinds": self.declaration.kinds,
            }),
        );
        let mut object = Object {
            id: id.clone(),
            machine: "host".into(),
            parent: None,
            config: Default::default(),
            entered_at: Default::default(),
            record,
            counters: Default::default(),
            applied_responses: vec![],
            seq,
            created: 0,
        };
        if object.config.is_empty() {
            flywheel_engine::initialise(&self.defs, &mut object, now);
        }
        self.store.put(&id, &object, seq)?;
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
            if !self.declaration.covers(object) {
                continue;
            }
            let standing = self.store.leases(&object.id)?;
            let mine = standing.as_ref().is_some_and(|l| l.holder == me);
            let expired = standing
                .as_ref()
                .map(|l| now - l.renewed_at > self.store.git.lease_expiry)
                .unwrap_or(true);
            if !mine && !expired {
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
            self.store.lease(&op)?;
        }
        Ok(())
    }

    /// One tick over a scope: fetch, heartbeat, read, decide, write, perform,
    /// and write the run record (D7, 79).
    pub fn tick(&mut self, scope: &Scope) -> Result<usize> {
        // Fetch first, always: no host decides on a read older than the bound
        // and no person runs the sync by hand (165).
        self.store.trace.borrow_mut().clear();
        self.store.git.fetch()?;
        self.store.saw("fetch");
        // The fetch's own point is where the next notice is measured from.
        self.last_point = self.store.git.as_of();
        self.declare()?;

        let objects = self.store.list(scope)?.objects;
        self.renew_and_take(&objects)?;
        self.store.reading.register = console::register(&self.store)?;

        let defs = self.defs.clone();
        let me = self.name.clone();
        let now = self.now();
        let mut run: Vec<RunEntry> = Vec::new();
        let sinks = &mut self.sinks;
        let ticked = console::tick(
            &mut self.store,
            &defs,
            scope,
            |store, object, region, effect| {
                perform(&defs, store, sinks, &me, now, object, region, effect)
            },
            |store, fired, tail| {
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
                run.push(
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
                    .reading(evidence),
                );
                let _ = tail;
            },
        )?;
        self.run.extend(run);
        self.take_over_released()?;
        self.report_takeovers()?;
        self.report_sessions()?;
        self.rewrite_status()?;
        let entries = std::mem::take(&mut self.run);
        self.store.git.append_run(&entries)?;
        Ok(ticked.transitions)
    }

    /// The sweep: every scope this host has a lease or a candidate on, which is
    /// what makes `older:` guards fire and a never-notified host converge
    /// (130, D7).
    pub fn sweep(&mut self) -> Result<usize> {
        let fired = self.tick(&Scope::All)?;
        self.last_sweep = Some(self.now());
        Ok(fired)
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
        for object in self.notified()? {
            for scope in self.chain(&object)? {
                fired += self.tick(&scope)?;
            }
        }
        let due = self
            .last_sweep
            .map(|last| self.now() - last >= Duration::seconds(SWEEP))
            .unwrap_or(true);
        if due {
            fired += self.sweep()?;
        }
        Ok(fired)
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
                if self.run.iter().any(|r| r.object == session && r.fields.iter().any(|(n, v)| n == "entry" && *v == reported)) {
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
        let held = self
            .store
            .leases(flywheel_domain::RAIL)?
            .map(|l| l.holder == self.name)
            .unwrap_or(true);
        if !held {
            return Ok(());
        }
        let as_of = self.store.git.as_of();
        let now = self.now();
        let status = flywheel_domain::status::read(
            &self.store.git,
            &self.defs,
            &as_of,
            now,
            Duration::minutes(5),
            Duration::minutes(30),
        )?;
        let view = flywheel_domain::status::render(&status);
        let held = self.store.git.committed_status()?;
        if let Some(held) = &held {
            if held != &view.body {
                // Drift is rewritten from the source and reported with both
                // values; nothing about the projection is ever the truth (77).
                self.run.push(
                    RunEntry::new(now, &self.name, "drift", flywheel_domain::RAIL, "the status projection differed from its source and was rewritten")
                        .with("was", &digest(held))
                        .with("now", &digest(&view.body)),
                );
            }
        }
        self.store.git.commit_status(&view.body)?;
        Ok(())
    }

    /// The status view as this host serves it, from the same read (132, 141).
    pub fn status(&self) -> Result<flywheel_domain::status::Status> {
        flywheel_domain::status::read(
            &self.store.git,
            &self.defs,
            &self.store.git.as_of(),
            self.now(),
            Duration::minutes(5),
            Duration::minutes(30),
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
        for entry in self.store.git.run_record()? {
            if entry.kind == "refusal" {
                out.push(format!("refusal: {}", entry.object));
            }
        }
        Ok(out)
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

/// Perform one effect through the bindings this release carries. An effect no
/// binding covers is recorded and counts as done: the machines tick over the
/// facts either way, and nothing is silently skipped (D8).
pub fn perform(
    defs: &Definitions,
    store: &mut HostStore,
    sinks: &mut Sinks,
    host: &str,
    now: DateTime<Utc>,
    object: &str,
    region: &str,
    effect: &PlannedEffect,
) -> bool {
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
            let _ = RecordedWorkspace::new(&mut store.git).create_line(object, &parent);
        }
        "take_parent" => {
            let _ = RecordedWorkspace::new(&mut store.git).take_parent(object);
        }
        "remove_line" => {
            let _ = RecordedWorkspace::new(&mut store.git).remove_line(object);
        }
        "land_line" => {
            let _ = RecordedWorkspace::new(&mut store.git).land_line(object, LandingPolicy::Direct);
        }
        "write_acceptance" => {
            let _ = RecordedWorkspace::new(&mut store.git).write_acceptance(object, "");
        }
        "prepare_place" => {
            let order = work_order(defs, store, &session, &place, object);
            let _ = RecordedWorkspace::new(&mut store.git).prepare_place(&place, object, &order.body);
        }
        "rebase_place" => {
            let _ = RecordedWorkspace::new(&mut store.git).rebase_place(&place);
        }
        "merge_place" => {
            let _ = RecordedWorkspace::new(&mut store.git).merge_place(&place);
        }
        "remove_place" => {
            let _ = RecordedWorkspace::new(&mut store.git).remove_place(&place);
        }
        // ---- the sessions, with the operator as the session (93b)
        "start_session" => {
            let fresh = flywheel_sessions_operator::next_attempt(&store.git, &stem);
            let order = work_order(defs, store, &fresh, &place, object);
            let _ = flywheel_sessions_operator::start(&mut store.git, host, now, &order);
        }
        "end_session" => {
            let _ = flywheel_sessions_operator::end(&mut store.git, &session, now);
        }
        "deliver_answer" => {
            let text = effect
                .args
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let _ = flywheel_sessions_operator::answer(&mut store.git, &session, host, now, &text);
        }
        "tell_moved" => {
            let _ = flywheel_sessions_operator::moved(&mut store.git, &session, host, now);
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
                let _ = store.git.lease(&LeaseOp::Release {
                    object: held.id.clone(),
                    holder: gone.clone(),
                });
                let _ = store.git.lease(&LeaseOp::Mark {
                    object: held.id.clone(),
                    state: "expired".into(),
                });
            }
            for session in flywheel_sessions_operator::of_host(&store.git, &gone) {
                let _ = flywheel_sessions_operator::take_over(&mut store.git, &session, host, now);
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
            let _ = flywheel_domain::signals::ensure_signal(
                &mut store.git,
                &mut *store.world,
                defs,
                object,
                &by,
                now,
            );
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
        "deliver_rail" => return deliver(defs, store, sinks, host, object),
        // The status projection is the rail's own effect (D12); the tick writes
        // it after every pass, so nothing to do here.
        "render_status" => {}
        _ => {}
    }
    true
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
) -> bool {
    let Some(channel) = sinks.channels.remove(sink) else {
        // No channel bound: this host presents nothing here, so nothing was
        // delivered and the proof stays absent (148, 127).
        return false;
    };
    let mut chat = Chat::new(sink, host, &sinks.address, channel);
    let delivered = matches!(chat.deliver(store, defs), Ok(Some(_)));
    sinks.channels.insert(sink.to_string(), chat.channel);
    delivered
}

/// The work order a session is given: the closed set of inputs, rendered, and
/// the name it reports under (88, 89).
fn work_order(
    _defs: &Definitions,
    store: &HostStore,
    session: &str,
    place: &str,
    object: &str,
) -> WorkOrder {
    let held = store.get(object).ok().flatten();
    let kind = held
        .as_ref()
        .map(|o| o.machine.clone())
        .unwrap_or_else(|| "work".into());
    let mut body = String::new();
    body.push_str(&format!("# {object}\n\n"));
    body.push_str(&format!("place: {place}\nsession: {session}\n"));
    body.push_str(&format!(
        "report with: flywheel exit done --deliverables <list>  (FLYWHEEL_SESSION={session})\n"
    ));
    if let Some(object) = &held {
        for (name, value) in &object.record {
            body.push_str(&format!("{name}: {value}\n"));
        }
    }
    WorkOrder {
        session: session.to_string(),
        kind,
        place: place.to_string(),
        body,
    }
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
