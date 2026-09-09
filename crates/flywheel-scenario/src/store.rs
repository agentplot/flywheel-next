//! The stand-in state store: every object, response and fact in one JSON
//! file. Sessions are played from a script; everything the machinery owns
//! (the engine, the register, the tail, the effects) runs for real.

use chrono::{DateTime, Duration, Utc};
pub use flywheel_atoms::scenario::{ScriptEntry, ServiceDecl, SessionFact};
use flywheel_engine::runtime::{EvidenceSource, Object, Register, Response, TailEntry};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PlaceFact {
    pub exists: bool,
    pub merged: bool,
    pub absent: bool,
    pub conflicted: bool,
    pub endpoints_recorded: bool,
    pub endpoints: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct LineFact {
    pub exists: bool,
    pub landed: bool,
    pub absent: bool,
    pub landing: String,
}

/// The tethered process behind one service object: what `wt tether` and portless would report.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ServiceFact {
    /// absent | present | exited
    pub process: String,
    pub serving: bool,
    pub endpoint: Option<String>,
    pub failure: Option<String>,
    pub started_tick: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct World {
    pub places: BTreeMap<String, PlaceFact>,
    pub lines: BTreeMap<String, LineFact>,
    pub sessions: BTreeMap<String, SessionFact>,
    pub script: BTreeMap<String, Vec<ScriptEntry>>,
    pub archived: BTreeMap<String, bool>,
    /// Service declarations per repository.
    pub declarations: BTreeMap<String, Vec<ServiceDecl>>,
    /// Service facts per service object id.
    pub services: BTreeMap<String, ServiceFact>,
    /// What the world reports on disk: path -> content, materialized from
    /// `conformance/fixtures/` or given inline. These are repository files.
    pub files: BTreeMap<String, String>,
    /// The raw store: transcripts, logs and the like, which stay outside every
    /// repository and are cited by the captures that point at them (111).
    #[serde(default)]
    pub raw: BTreeMap<String, String>,
}

/// One host's checkout of the state repository, behind the record operations.
/// Each host has its own, so two hosts racing on one object race the way they
/// do on a real git host: on the expected-old push (134, 162, I15). A restart
/// drops what the host remembers and not what the store holds (75, I14).
pub type Durable = std::sync::Arc<std::sync::Mutex<flywheel_store_git::GitStore>>;

/// A response handed back to the engine, and the tick that reported it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Unapplicable {
    pub response: String,
    pub object: String,
    pub reason: String,
    /// The tick this stood under attention; after it, it is reported and gone.
    pub reported_after: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub at: DateTime<Utc>,
    pub tick: u64,
    pub kind: String,
    pub object: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Store {
    pub objects: BTreeMap<String, Object>,
    pub responses: Vec<Response>,
    pub register: Register,
    pub tail: Vec<TailEntry>,
    /// Scenario-given evidence: object id (or `*`) → name → value.
    pub given: BTreeMap<String, BTreeMap<String, Value>>,
    pub world: World,
    pub log: Vec<LogEntry>,
    pub now: DateTime<Utc>,
    pub tick: u64,
    pub tick_seconds: i64,
    pub next_created: u64,
    pub next_response: u64,
    pub host_bound: usize,
    pub marks: BTreeMap<String, DateTime<Utc>>,
    /// Decision ids standing after the last derive; used for response.decision_present.
    pub standing: Vec<String>,
    pub scenario: Option<String>,
    // ---- the state store's own durable shapes (125)
    /// Object id -> its thread, in order.
    #[serde(default)]
    pub threads: BTreeMap<String, Vec<flywheel_atoms::ThreadEntry>>,
    /// Every effect written, by its identity; a repeat finds itself here (127).
    #[serde(default)]
    pub effects_written: Vec<EffectRecord>,
    /// Object id -> the lease on it (128).
    #[serde(default)]
    pub leases: BTreeMap<String, flywheel_atoms::LeaseRecord>,
    /// Host id -> its heartbeat (147, 163).
    #[serde(default)]
    pub heartbeats: BTreeMap<String, flywheel_atoms::HostRecord>,
    /// The write sequence: the point a read is as of (126).
    #[serde(default)]
    pub writes: u64,
    /// (write sequence, object) for every write, so a notice names what moved (130).
    #[serde(default)]
    pub moved: Vec<(u64, String)>,
    /// The durable state binding `--profile git-only` puts behind the record
    /// operations: a real state repository, whose objects, threads, effect
    /// commits and leases are files and branches of it. The world, the clock
    /// and the log stay the runner's either way, which is the only thing the
    /// two profiles differ in (125, D15).
    #[serde(skip)]
    pub durable: BTreeMap<String, Durable>,
    /// A response whose decision was gone when it arrived. It is never
    /// dropped: it stands under attention until a tick has reported it once
    /// (6, 129).
    #[serde(default)]
    pub unapplicable: Vec<Unapplicable>,
    /// What each sink has been presented with (129).
    #[serde(default)]
    pub presented: Vec<PresentedRecord>,
    /// The acting host, which a `host` step sets; `local` with no host step.
    #[serde(default)]
    pub acting_host: Option<String>,
    /// Hosts that cannot reach the store (151, D4a).
    #[serde(default)]
    pub disconnected: Vec<String>,
    /// A start of a session name whose pane was already present: the
    /// multiplexer refuses one, so this counts what would have been a second
    /// session under one name (72, 111).
    #[serde(default)]
    pub duplicate_starts: usize,
    /// The rail object's own state and the point it entered it: the register
    /// lives in the record, the machine's states here (148).
    #[serde(default)]
    pub rail_config: BTreeMap<String, String>,
    #[serde(default)]
    pub rail_entered: BTreeMap<String, DateTime<Utc>>,
    /// The status projection `render_status` wrote, and the write sequence it
    /// is as of (132, 145).
    #[serde(default)]
    pub status_body: String,
    #[serde(default)]
    pub status_as_of: u64,
    /// Whether the projection was committed where a reader with no host can
    /// find it (`git-only.yaml` status, S20).
    #[serde(default)]
    pub status_committed: bool,
    /// The most sessions this host had running at once; never above its bound
    /// (31, 32, 149). Kept with the store, so a restart does not forget it.
    #[serde(default)]
    pub sessions_running_max: usize,
    /// The objects that reached `merged`, in the order they did (57).
    #[serde(default)]
    pub merge_order: Vec<String>,
    /// What each host declared it takes, by host name (149, 217).
    #[serde(default)]
    pub declarations: BTreeMap<String, flywheel_domain::derived::Declaration>,
    /// Numbers a scenario gave a decision by its readable name,
    /// `<object id>/<decision kind>`. The register itself is keyed by the
    /// engine's decision id, which also carries the point the decision was
    /// raised; this is how `given.register` reaches it.
    #[serde(default)]
    pub register_aliases: BTreeMap<String, u32>,
    /// Every decision seen, by its readable name, with the number the register
    /// gave it. A number is never reused, so this only grows; it is what lets a
    /// repeat delivery name a decision that has since been retracted (15, 137).
    #[serde(default)]
    pub decision_numbers: BTreeMap<String, u32>,
}

/// One effect written, with its identity, its reason and the evidence the
/// guard read (79, 127, 167).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectRecord {
    pub effect_id: String,
    pub object: String,
    pub effect: String,
    pub reason: String,
    pub evidence: BTreeMap<String, Value>,
    pub at: DateTime<Utc>,
    /// The host that wrote it, or `local`.
    pub by: String,
    /// A write made while disconnected is an intention until its push lands
    /// (161, D4a).
    pub pending: bool,
}

/// One delivery of the rail's decisions to a sink (129).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresentedRecord {
    pub sink: String,
    pub at: DateTime<Utc>,
    pub numbers: Vec<u32>,
    pub decisions: Vec<String>,
}

impl Default for Store {
    fn default() -> Self {
        Store {
            objects: BTreeMap::new(),
            responses: vec![],
            register: Register::default(),
            tail: vec![],
            given: BTreeMap::new(),
            world: World::default(),
            log: vec![],
            now: Utc::now(),
            tick: 0,
            tick_seconds: 60,
            next_created: 1,
            next_response: 1,
            host_bound: 4,
            marks: BTreeMap::new(),
            standing: vec![],
            scenario: None,
            threads: BTreeMap::new(),
            effects_written: vec![],
            leases: BTreeMap::new(),
            heartbeats: BTreeMap::new(),
            writes: 0,
            moved: vec![],
            durable: BTreeMap::new(),
            unapplicable: vec![],
            presented: vec![],
            acting_host: None,
            disconnected: vec![],
            duplicate_starts: 0,
            rail_config: BTreeMap::new(),
            rail_entered: BTreeMap::new(),
            status_body: String::new(),
            status_as_of: 0,
            status_committed: false,
            sessions_running_max: 0,
            merge_order: vec![],
            declarations: BTreeMap::new(),
            register_aliases: BTreeMap::new(),
            decision_numbers: BTreeMap::new(),
        }
    }
}

pub use flywheel_domain::regions::{place_key, session_key, session_key_of};

impl Store {
    pub fn log(&mut self, kind: &str, object: &str, text: impl Into<String>) {
        self.log.push(LogEntry { at: self.now, tick: self.tick, kind: kind.to_string(), object: object.to_string(), text: text.into() });
    }

    pub fn given_value(&self, object: &str, name: &str) -> Option<Value> {
        self.given.get(object).and_then(|m| m.get(name)).cloned().or_else(|| self.given.get("*").and_then(|m| m.get(name)).cloned())
    }

    pub fn set_given(&mut self, object: &str, name: &str, v: Value) {
        self.given.entry(object.to_string()).or_default().insert(name.to_string(), v);
    }

    fn state_of(&self, id: &str) -> Option<&str> {
        self.objects.get(id).and_then(|o| o.top_state())
    }

    fn running_sessions(&self) -> usize {
        self.world.sessions.values().filter(|s| s.pane).count()
    }

    /// How many sessions this host is running, against which its bound is read
    /// (31, 32, `record-derived.yaml` host.running). A scenario that states it
    /// is stating what the host reports; otherwise it is the panes the world
    /// holds.
    pub fn host_running(&self) -> usize {
        let me = self.me();
        let stated: Vec<u64> = self
            .given
            .iter()
            .filter(|(key, _)| {
                key.as_str() == me || key.trim_start_matches("host/") == me || self.given.len() == 1
            })
            .filter_map(|(_, per)| per.get("host.running").and_then(|v| v.as_u64()))
            .collect();
        match stated.len() {
            1 => stated[0] as usize,
            _ => {
                let any: Vec<u64> = self
                    .given
                    .values()
                    .filter_map(|per| per.get("host.running").and_then(|v| v.as_u64()))
                    .collect();
                match any.len() {
                    1 => any[0] as usize,
                    _ => self.running_sessions(),
                }
            }
        }
    }

    fn deps_merged(&self, obj: &Object) -> bool {
        let Some(Value::Array(deps)) = obj.record.get("depends_on") else { return true };
        deps.iter().all(|d| d.as_str().map(|id| self.state_of(id) == Some("merged") || self.state_of(id) == Some("landed")).unwrap_or(true))
    }

    /// The bolt a service belongs to, and its declaration if the repository still names it.
    fn service_context(&self, object: &str) -> (Option<&Object>, Option<&ServiceDecl>) {
        let obj = self.objects.get(object);
        let bolt = obj.and_then(|o| o.parent.as_deref()).and_then(|b| self.objects.get(b));
        let name = obj.and_then(|o| o.record.get("name")).and_then(|v| v.as_str()).unwrap_or("");
        let decl = bolt.and_then(|b| self.declarations_of(b)).and_then(|d| d.iter().find(|d| d.name == name));
        (bolt, decl)
    }

    fn declarations_of(&self, bolt: &Object) -> Option<&Vec<ServiceDecl>> {
        bolt.record.get("repository").and_then(|v| v.as_str()).and_then(|r| self.world.declarations.get(r))
    }

    /// The session a region path refers to, at the attempt the object is on: a
    /// session lost sends its stage round again with the attempt one higher,
    /// and the fresh session is a session of its own (150, `stage.yaml`).
    pub fn session_of(&self, object: &str, region: &str) -> String {
        match self.objects.get(object) {
            Some(held) => session_key_of(held, region),
            None => session_key(object, region),
        }
    }

    /// The bolt's own place exists: its place region is anywhere but absent or removed.
    fn bolt_place_present(bolt: Option<&Object>) -> bool {
        bolt.and_then(|b| b.config.get("place.place.life")).map(|s| s != "removed" && s != "absent").unwrap_or(false)
    }

    /// The service object a bolt would hold for a declaration name.
    pub fn service_id(bolt: &str, name: &str) -> String {
        format!("service/{}/{name}", bolt.strip_prefix("bolt/").unwrap_or(bolt))
    }

    /// Every declaration of the bolt's repository has a service object in a state other than gone.
    fn services_declared(&self, bolt: &Object) -> bool {
        self.declarations_of(bolt).map(|decls| decls.iter().all(|d| {
            self.objects.get(&Self::service_id(&bolt.id, &d.name)).and_then(|o| o.top_state()).map(|s| s != "gone").unwrap_or(false)
        })).unwrap_or(true)
    }

    fn derived(&self, object: &str, region: &str, name: &str) -> Option<Value> {
        // The signal material, read from the files the blueprints hold, exactly
        // as a host reads it from the checkout (111, 107,
        // `blueprints.yaml` evidence).
        if let Some(v) = flywheel_domain::signals::evidence(&self.world.files, object, name) {
            return Some(v);
        }
        let obj = self.objects.get(object);
        let skey = self.session_of(object, region);
        let pkey = place_key(object, region);
        let sess = self.world.sessions.get(&skey);
        let place = self.world.places.get(&pkey);
        let line = self.world.lines.get(object);
        let svc = self.world.services.get(object);
        let v = match name {
            // ---- service
            "service.process" => json!(svc.map(|s| if s.process.is_empty() { "absent".to_string() } else { s.process.clone() }).unwrap_or_else(|| "absent".into())),
            "service.process_absent" => json!(svc.map(|s| s.process.is_empty() || s.process == "absent").unwrap_or(true)),
            "service.serving" => json!(svc.map(|s| s.serving).unwrap_or(false)),
            "service.endpoint" => svc.and_then(|s| s.endpoint.clone()).map(Value::String)?,
            "service.failure" => svc.and_then(|s| s.failure.clone()).map(Value::String)?,
            "service.endpoint_recorded" => json!(match (svc.and_then(|s| s.endpoint.as_deref()), obj.and_then(|o| o.record.get("endpoint")).and_then(|v| v.as_str())) {
                (Some(served), Some(recorded)) => served == recorded,
                (None, None) => true,
                (None, Some(recorded)) => recorded.is_empty(),
                _ => false,
            }),
            "service.place_present" => { let (bolt, _) = self.service_context(object); json!(Self::bolt_place_present(bolt)) }
            "service.declared" => { let (_, decl) = self.service_context(object); json!(decl.is_some()) }
            "bolt.services_declared" => json!(obj.map(|b| self.services_declared(b)).unwrap_or(true)),
            // ---- capture and signal
            "capture.signals_present" => json!(self.objects.values().any(|o| o.parent.as_deref() == Some(object) && o.machine == "signal")),
            "capture.source" => obj.and_then(|o| o.record.get("source").cloned())?,
            "signal.move" => json!(obj.and_then(|o| o.record.get("move")).and_then(|m| m.get("target")).and_then(|t| t.as_str()).filter(|t| !t.is_empty()).unwrap_or("none")),
            // ---- place
            "place.exists" => json!(place.map(|p| p.exists && !p.absent).unwrap_or(false)),
            "place.absent" => json!(place.map(|p| p.absent || !p.exists).unwrap_or(true)),
            "place.merged" => json!(place.map(|p| p.merged).unwrap_or(false)),
            "place.conflicted" => json!(place.map(|p| p.conflicted).unwrap_or(false)),
            "place.contains_line" => json!(true),
            "place.ahead_of_line" => json!(false),
            "place.merge_slot" => json!(true),
            "place.endpoints_recorded" => json!(place.map(|p| p.endpoints_recorded).unwrap_or(true)),
            "place.endpoints_served" => json!(place.map(|p| p.endpoints.clone()).unwrap_or_default()),
            "place.held" => json!(obj.map(|o| o.record.get("held_at").map(|v| !v.is_null()).unwrap_or(false)).unwrap_or(false)),
            "place.conflict_retries" => json!(obj.and_then(|o| o.counters.get("conflict_retries").copied()).unwrap_or(0)),
            "place.ready" => json!(place.map(|p| p.exists && !p.absent).unwrap_or(false)),
            "place.job_seeded" | "place.line_moved_told" => json!(true),
            "session.pane_absent" => json!(!sess.map(|s| s.pane).unwrap_or(false)),
            "unit.bolt_exists" => json!(obj.and_then(|o| o.record.get("target")).map(|t| t.get("bolt").and_then(|b| b.as_str()).map(|b| !b.is_empty()).unwrap_or(false) || t.get("new_name").is_none()).unwrap_or(true)),
            // ---- line
            "line.exists" => json!(line.map(|l| l.exists && !l.absent).unwrap_or(false)),
            "line.absent" => json!(line.map(|l| l.absent || !l.exists).unwrap_or(true)),
            "line.contains_parent" => json!(true),
            "line.take_due" | "line.take_conflicts" | "line.conflict_job_done" => json!(false),
            "line.landed" => json!(line.map(|l| l.landed).unwrap_or(false)),
            "line.landing" => json!(line.map(|l| if l.landing.is_empty() { "none".to_string() } else { l.landing.clone() }).unwrap_or_else(|| "none".into())),
            "line.retries" => json!(obj.and_then(|o| o.counters.get("retries").copied()).unwrap_or(0)),
            // the pull-request landing (175–177): the stand-in's lines land direct unless a scenario says otherwise
            "line.policy" => json!("direct"),
            "line.request" => json!("none"),
            "line.request_opened" | "line.request_review_pending" => json!(false),
            "line.request_review_recorded" | "line.acceptance_written" => json!(true),
            "line.request_links" => json!([]),
            // ---- session
            "session.pane" => json!(sess.map(|s| if s.pane { "present" } else { "absent" }).unwrap_or("absent")),
            "session.activity" => json!(sess.map(|s| if s.pane { s.activity.clone() } else { "none".into() }).unwrap_or_else(|| "none".into())),
            "session.exit" => json!(sess.and_then(|s| s.exit.clone()).unwrap_or_else(|| "none".into())),
            "session.idle_since" => sess.and_then(|s| s.idle_since).map(|t| json!(t.to_rfc3339()))?,
            "session.operator_present" | "session.offers_pending" | "session.refusals_pending" => json!(false),
            "session.exit_recorded" | "session.host_alive" | "session.answer_delivered" | "session.message_delivered" => json!(true),
            "session.question" => json!(sess.and_then(|s| s.question.clone())),
            "session.expected" | "session.delivered" => json!(sess.map(|s| s.deliverables.clone()).unwrap_or_default()),
            // ---- stage
            "stage.agents" => json!(["agent"]),
            "stage.join_met" => json!(sess.map(|s| matches!(s.exit.as_deref(), Some("done") | Some("stalled") | Some("invalid"))).unwrap_or(false)),
            "stage.verdict" => json!(sess.and_then(|s| s.verdict.clone().or_else(|| match s.exit.as_deref() {
                Some("done") => Some("pass".into()),
                Some("blocked") => Some("blocked".into()),
                Some("stalled") | Some("invalid") => Some("stalled".into()),
                _ => None,
            })).unwrap_or_else(|| "none".into())),
            "item.send_backs" => json!(obj.and_then(|o| o.counters.get("send_backs").copied()).unwrap_or(0)),
            "item.retry_max" => json!(3),
            "item.deps_merged" | "unit.deps_merged" => json!(obj.map(|o| self.deps_merged(o)).unwrap_or(true)),
            "item.slot_free" => json!(self.host_running() < self.host_bound),
            "unit.claim_moved" | "bolt.citations_moved" | "bolt.chores_outstanding" | "bolt.hold_since_last_merge" => json!(false),
            "unit.items_exist" => json!(self.objects.values().any(|o| o.parent.as_deref() == Some(object) && o.machine == "work-item")),
            // ---- design side
            "intent.material_pending" => json!(false),
            "intent.covered_by" => json!("none"),
            "elaboration.covers_set" | "elaboration.records_fanned_out" | "curation.gatherings_proposed" => json!(true),
            "intent.close_declined_since_last_final" => json!(obj.map(|o| o.record.get("close_declined_at").map(|v| !v.is_null()).unwrap_or(false)).unwrap_or(false)),
            "intent.archived" | "item.change_archived" => json!(self.world.archived.get(object).copied().unwrap_or(false)),
            "elaboration.shown_with_parent" => json!(obj.and_then(|o| o.parent.as_deref()).and_then(|p| self.state_of(p)) == Some("proposed")),
            "elaboration.kept_since" => obj.and_then(|o| o.record.get("kept_at").cloned()).filter(|v| !v.is_null())?,
            "elaboration.type" => obj.and_then(|o| o.record.get("type").cloned())?,
            // ---- engine machines
            "rail.unnumbered" | "sink.due" | "host.stray_places" => json!(false),
            // The projection's as-of equals the newest seq across `list(all)`,
            // or it is stale and is rewritten from its source on this tick
            // (77, 142, `record-derived.yaml` rail.status_current).
            "rail.status_current" => json!(!self.status_body.is_empty() && self.status_as_of >= self.newest_seq()),
            "host.last_seen" => obj.and_then(|o| o.record.get("last_seen").cloned())?,
            "lease.holder" => return None,
            "response.applied" => {
                let rid = object.strip_prefix("response/").unwrap_or(object);
                json!(self.objects.values().any(|o| o.applied_responses.iter().any(|a| a == rid)))
            }
            "response.decision_present" => {
                let rid = object.strip_prefix("response/").unwrap_or(object);
                let Some(r) = self.responses.iter().find(|r| r.id == rid) else { return Some(json!(false)) };
                match r.decision {
                    // A dictation names an operation of the catalogue. One
                    // that asserts work was done names none — no such tool
                    // exists — so it is unapplicable and comes back once under
                    // attention (4, 6, 193).
                    None => json!(self
                        .objects
                        .get(object)
                        .and_then(|o| o.record.get("tool").and_then(|v| v.as_str()))
                        .map(flywheel_domain::commands::is_operation)
                        .unwrap_or(true)),
                    Some(n) => json!(self.register.decision_of(n).map(|d| self.standing.iter().any(|s| s == d)).unwrap_or(false)),
                }
            }
            "response.reported" => json!(false),
            "response.answer" => {
                let rid = object.strip_prefix("response/").unwrap_or(object);
                json!(self.responses.iter().find(|r| r.id == rid).map(|r| r.answer.clone()))
            }
            // Most of what a profile reads is a field of the object's own
            // record (`record-derived.yaml`): `curation.threshold` is the
            // threshold the record carries, and a scenario that seeds one has
            // said what it is. Last, so nothing above it is shadowed.
            _ => match name.rsplit_once('.').and_then(|(_, field)| {
                obj.and_then(|o| o.record.get(field)).cloned()
            }) {
                Some(held) => held,
                None => return None,
            },
        };
        Some(v)
    }

    /// A lease's own evidence, read from the lease record and the declarations
    /// the hosts made (128, 149).
    fn lease_evidence(&self, object: &str, name: &str) -> Option<Value> {
        let held = self.leases.get(object);
        match name {
            // A holder that is nobody is no holder: `exists:` reads it as absent.
            "lease.holder" => held
                .map(|l| l.holder.clone())
                .filter(|h| !h.is_empty())
                .map(Value::String),
            "lease.renewed_at" => held.map(|l| json!(l.renewed_at.to_rfc3339())),
            "lease.taken_at" => held.map(|l| json!(l.taken_at.to_rfc3339())),
            // No host's declaration covering it is what makes it uncovered
            // (149). With no declaration given, every host covers everything.
            "lease.coverable" => Some(json!(self.covered(object))),
            _ => None,
        }
    }

    /// Whether one host's declaration covers this object (149).
    pub fn covers(&self, host: &str, object: &str) -> bool {
        let Some(declaration) = self.declarations.get(host) else {
            return true;
        };
        let Some(held) = self.objects.get(object) else {
            return true;
        };
        declaration.covers(held)
    }

    /// Whether any host's declaration covers this object (149).
    pub fn covered(&self, object: &str) -> bool {
        if self.declarations.is_empty() {
            return true;
        }
        let Some(held) = self.objects.get(object) else {
            return true;
        };
        self.declarations.values().any(|d| d.covers(held))
    }

    /// The newest sequence across the objects: what a projection of them is as
    /// of (`record-derived.yaml` rail.status_current).
    pub fn newest_seq(&self) -> u64 {
        self.objects.values().map(|o| o.seq).max().unwrap_or(0)
    }

    /// Write the status projection from `list` and `get` alone, stating the
    /// point it is as of (132, 145). Where the profile keeps files it is
    /// committed on the shared line, so the operator reads it with no host
    /// running (S20, `git-only.yaml` status); on the stand-in the trace holds
    /// it. `render_status` is this, and so is the end of every tick: a
    /// projection is rewritten from its source and is never itself the truth
    /// (77, 142).
    pub fn write_status(&mut self, defs: &flywheel_engine::Definitions) {
        let as_of = flywheel_atoms::ReadPoint {
            mark: self
                .durable()
                .and_then(|d| d.lock().ok().map(|held| held.fetched.clone()))
                .unwrap_or_else(|| format!("write {}", self.writes)),
            seq: self.writes,
            at: self.now,
        };
        let Ok(status) = flywheel_domain::status::read(
            self,
            defs,
            &as_of,
            self.now,
            Duration::minutes(5),
            Duration::minutes(30),
        ) else {
            return;
        };
        let view = flywheel_domain::status::render(&status);
        self.status_body = view.body.clone();
        self.status_as_of = self.newest_seq();
        if let Some(durable) = self.durable() {
            if let Ok(mut held) = durable.lock() {
                self.status_committed = held.commit_status(&view.body).is_ok();
            }
        }
    }

    /// Advance every started session's script by one tick.
    pub fn play_scripts(&mut self) {
        let now = self.now;
        let keys: Vec<String> = self.world.sessions.keys().cloned().collect();
        for k in keys {
            let script = self.world.script.get(&k).cloned().unwrap_or_default();
            let Some(s) = self.world.sessions.get_mut(&k) else { continue };
            if !s.pane { continue; }
            s.ticks_alive += 1;
            for (i, e) in script.iter().enumerate() {
                if s.played.contains(&i) { continue; }
                let due = match e.after.as_deref() {
                    None | Some("") | Some("0") | Some("0t") => true,
                    Some("answer") => false,
                    Some(a) if a.ends_with('t') => a.trim_end_matches('t').parse::<u64>().map(|n| s.ticks_alive >= n).unwrap_or(true),
                    Some(a) => flywheel_engine::eval::parse_duration(a).map(|d| Duration::seconds(s.ticks_alive as i64 * self.tick_seconds) >= d).unwrap_or(true),
                };
                if !due { continue; }
                apply_entry(s, e, now);
                s.played.push(i);
            }
        }
    }

    /// Advance every started service by one tick: a process started last tick now serves, or
    /// exits when its declaration says it fails.
    pub fn play_services(&mut self) {
        let tick = self.tick;
        let ids: Vec<String> = self.world.services.iter().filter(|(_, f)| f.process == "present" && !f.serving && f.started_tick < tick).map(|(k, _)| k.clone()).collect();
        for id in ids {
            let (bolt, decl) = self.service_context(&id);
            let fails = decl.map(|d| d.fails).unwrap_or(false);
            let command = decl.map(|d| d.command.clone()).unwrap_or_default();
            let endpoint = bolt.map(|b| Self::endpoint_for(b, decl, &id));
            let Some(f) = self.world.services.get_mut(&id) else { continue };
            if fails {
                f.process = "exited".into();
                f.failure = Some(format!("exit 1 · {command}\nerror: address already in use"));
            } else {
                f.serving = true;
                f.endpoint = endpoint;
            }
        }
    }

    /// The URL the place would serve for a service: the place names the host, the declaration's port
    /// hint (or one derived from the id) the port, so two places on one host never collide.
    fn endpoint_for(bolt: &Object, decl: Option<&ServiceDecl>, id: &str) -> String {
        let repo = bolt.record.get("repository").and_then(|v| v.as_str()).unwrap_or("repo");
        let name = bolt.record.get("name").and_then(|v| v.as_str()).unwrap_or("bolt");
        let port = decl.and_then(|d| d.port).unwrap_or_else(|| 40000 + (id.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) % 10000) as u16);
        format!("http://{repo}.{name}.localhost:{port}")
    }

    pub fn play_after_answer(&mut self, key: &str) {
        let now = self.now;
        let script = self.world.script.get(key).cloned().unwrap_or_default();
        let Some(s) = self.world.sessions.get_mut(key) else { return };
        for (i, e) in script.iter().enumerate() {
            if s.played.contains(&i) || e.after.as_deref() != Some("answer") { continue; }
            apply_entry(s, e, now);
            s.played.push(i);
        }
    }
}

pub fn apply_entry_pub(s: &mut SessionFact, e: &ScriptEntry, now: DateTime<Utc>) { apply_entry(s, e, now) }

fn apply_entry(s: &mut SessionFact, e: &ScriptEntry, now: DateTime<Utc>) {
    // The scenario has spoken for this session by name (see `SessionFact`).
    s.scripted = true;
    if let Some(p) = &e.pane { s.pane = p == "present"; }
    if let Some(a) = &e.activity {
        if a == "idle" && s.activity != "idle" { s.idle_since = Some(now); }
        s.activity = a.clone();
    }
    if let Some(x) = &e.exit { s.exit = Some(x.clone()); }
    if let Some(q) = &e.question { s.question = Some(q.clone()); }
    if let Some(v) = &e.verdict { s.verdict = Some(v.clone()); }
    if !e.deliverables.is_empty() { s.deliverables = e.deliverables.clone(); }
}

impl EvidenceSource for Store {
    fn evidence(&self, object: &str, region: &str, name: &str) -> Option<Value> {
        // What a scenario says about this object outright.
        if let Some(v) = self.given.get(object).and_then(|m| m.get(name)) {
            return Some(v.clone());
        }
        // A wildcard about places and sessions says what the world reports
        // about one that exists — panes come up, places come up ready. It
        // cannot report on one the machinery has not made yet, and so cannot
        // prove `prepare_place` or `start_session` before either has run (127).
        if name.starts_with("place.") || name.starts_with("session.") {
            let made = match name.starts_with("place.") {
                true => self.world.places.contains_key(&place_key(object, region)),
                false => self.world.sessions.contains_key(&self.session_of(object, region)),
            };
            if !made {
                return self.derived(object, region, name);
            }
            // A session the scenario scripted by name has been spoken for, and
            // a wildcard about every session does not overrule it.
            if name.starts_with("session.")
                && self
                    .world
                    .sessions
                    .get(&self.session_of(object, region))
                    .is_some_and(|s| s.scripted)
            {
                return self.derived(object, region, name);
            }
        }
        if let Some(v) = self.given.get("*").and_then(|m| m.get(name)) {
            return Some(v.clone());
        }
        // A lease is named for the object it is on, and a scenario describes it
        // there: `lease.coverable` on `unit/atlas/u` is the lease's own
        // (`engine/lease.yaml`, X05).
        if let Some(leased) = flywheel_domain::leases::object_of(object) {
            if let Some(v) = self.given_value(leased, name) {
                return Some(v);
            }
            return self.lease_evidence(leased, name);
        }
        self.derived(object, region, name)
    }
}
