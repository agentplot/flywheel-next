//! The four seams a host binds an implementation to, and nothing that binds one.
//!
//! `StateStore` is the one door between the data plane and durable, shared
//! storage (125); `World`, `Workspace` and `Sessions` are the three doors on to
//! the world the effects act on (D8). Each has one implementation in phase 1
//! and is replaced, not branched, in phase 2.

use anyhow::Result;
use chrono::{DateTime, Utc};
pub use flywheel_engine::Object;
use flywheel_engine::{DecisionInstance, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

// ---------------------------------------------------------------- shared shapes

/// The point a read is as of: what the store names when it answers (126).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadPoint {
    /// The profile's own name for the point — a commit, a revision, a cursor.
    pub mark: String,
    /// The newest write sequence the read covers.
    pub seq: u64,
    pub at: DateTime<Utc>,
}

/// The objects a call concerns (131).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scope {
    /// Every object the store holds.
    All,
    /// Every object of one machine.
    Machine(String),
    /// One object and everything it owns.
    Under(String),
}

/// An entry on an object's thread.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreadEntry {
    pub at: DateTime<Utc>,
    pub kind: String,
    #[serde(default)]
    pub by: Option<String>,
    #[serde(default)]
    pub fields: BTreeMap<String, Value>,
}

/// What `put` did. A rejection carries the sequence the store holds, so the
/// loser of a race knows it lost and reads again before deciding (134).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PutOutcome {
    Written { seq: u64 },
    Rejected { held_seq: u64 },
}

/// A lease record: who holds an object, and since when (128).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseRecord {
    pub object: String,
    pub holder: String,
    pub taken_at: DateTime<Utc>,
    pub renewed_at: DateTime<Utc>,
}

/// A host's heartbeat record (147, 163).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostRecord {
    pub host: String,
    pub last_seen: DateTime<Utc>,
    #[serde(default)]
    pub bound: u32,
    #[serde(default)]
    pub intermittent: bool,
}

// --------------------------------------------------------------- record layer

/// The six record operations every profile binds, over which every evidence
/// and effect that is a function of an object's record and thread is stated
/// once (`profiles/record-derived.yaml`). `leases` and `hosts` are the sixth.
pub trait Records {
    fn get(&self, id: &str) -> Result<Option<Object>>;

    fn put(&mut self, id: &str, record: &Object, base_seq: u64) -> Result<PutOutcome>;

    fn append(&mut self, id: &str, entry: &ThreadEntry) -> Result<()>;

    /// `record-derived.yaml`'s `list(scope)`; the contract's `list` is served
    /// over it.
    fn list_records(&self, scope: &Scope) -> Result<Vec<Object>>;

    /// The thread on an object, in order.
    fn thread(&self, id: &str) -> Result<Vec<ThreadEntry>>;

    /// Op-response records naming this object, or a number in its register
    /// entries.
    fn responses(&self, id: &str) -> Result<Vec<Response>>;

    fn leases(&self, id: &str) -> Result<Option<LeaseRecord>>;

    fn hosts(&self) -> Result<Vec<HostRecord>>;
}

// ------------------------------------------------------------- the state store

/// One object's evidence, as of a point the store names (126).
#[derive(Debug, Clone)]
pub struct EvidenceRead {
    pub object: Option<Object>,
    pub evidence: BTreeMap<String, Value>,
    pub as_of: ReadPoint,
}

/// The objects in a scope, as of a point the store names (131).
#[derive(Debug, Clone)]
pub struct Listing {
    pub objects: Vec<Object>,
    pub as_of: ReadPoint,
}

/// An effect written with an identity of its own (127).
#[derive(Debug, Clone)]
pub struct EffectWrite {
    /// `<object>/<transition>/<proof evidence>/<evidence hash>` (79, 127, 167).
    pub effect_id: String,
    pub object: String,
    pub effect: String,
    pub reason: String,
    /// The evidence values the guard read.
    pub evidence: BTreeMap<String, Value>,
}

/// What a write reported. A repeat is neither an error nor a second write
/// (127); a write made while disconnected is an intention until its push lands
/// (161, 133, D4a).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteOutcome {
    Written { effect_id: String },
    AlreadyWritten { effect_id: String },
    Pending { effect_id: String },
}

/// Take, renew or release (128).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeaseOp {
    Take { object: String, holder: String },
    Renew { object: String, holder: String },
    Release { object: String, holder: String },
}

#[derive(Debug, Clone)]
pub enum LeaseOutcome {
    Held(LeaseRecord),
    /// Someone else holds it; the reader moves on (128).
    HeldByAnother(LeaseRecord),
    Released,
}

/// What moved since a point, so a host re-reads only what a notice names (130).
#[derive(Debug, Clone)]
pub struct Notice {
    pub as_of: ReadPoint,
    pub objects: Vec<String>,
}

/// The rail's decisions on their way to a sink (129).
#[derive(Debug, Clone)]
pub struct Presentation {
    pub sink: String,
    pub decisions: Vec<DecisionInstance>,
}

/// The status view, served from the same state the engine reads and readable
/// with no machinery running (132, 145).
#[derive(Debug, Clone)]
pub struct StatusView {
    pub as_of: ReadPoint,
    pub body: String,
}

/// The operations of 125, and the engine needs no others. Present and receive
/// are two, so the trait is eight methods over the six record operations (D3).
pub trait StateStore: Records {
    /// Read an object's evidence as of a point the store names, writing
    /// nothing (126).
    fn read(&self, id: &str) -> Result<EvidenceRead>;

    /// Enumerate the objects in a scope, so a forgetful engine is complete
    /// (131, 136).
    fn list(&self, scope: &Scope) -> Result<Listing>;

    /// Serve the status view from the same state the engine reads (132).
    fn status(&self) -> Result<StatusView>;

    /// Write an effect with an identity of its own; a repeat changes nothing
    /// and is not a second write (127).
    fn write_effect(&mut self, write: &EffectWrite) -> Result<WriteOutcome>;

    /// Take, renew, release — and expire by the profile's stated rule (128).
    fn lease(&mut self, op: &LeaseOp) -> Result<LeaseOutcome>;

    /// Tell a host what moved since a point, within a bound the profile states
    /// (130).
    fn notify(&self, since: &ReadPoint) -> Result<Notice>;

    /// Present the rail's decisions to a sink (129).
    fn present(&mut self, presentation: &Presentation) -> Result<()>;

    /// Receive the operator's responses, attributed to the decisions they
    /// answer; one that cannot be applied is handed back and never dropped
    /// (129, 6).
    fn receive(&mut self, response: &Response) -> Result<Received>;
}

/// What became of a received response (129, 137, 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Received {
    Recorded { id: String },
    /// The same delivery again: it takes effect once (137).
    AlreadyApplied { id: String },
    /// Its decision is gone; shown once under attention, never dropped (6).
    Unapplicable { id: String, reason: String },
}

// ---------------------------------------------------------------------- world

/// A repository as the manifest names it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryRef {
    pub name: String,
    pub remote: String,
    pub shared_line: String,
}

/// The endpoint a host's router gives a service in a place (46, 191).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub name: String,
    pub url: String,
}

/// Repositories, the manifest, the router, and the App's tokens (D8).
pub trait World {
    /// The manifest as of the shared line, by key path.
    fn manifest(&self) -> Result<Value>;

    /// The repositories the organization tracks (205).
    fn repositories(&self) -> Result<Vec<RepositoryRef>>;

    /// Clone what the manifest names bare under the root, checking out each
    /// shared line once; a repeat adds only what is missing (205).
    fn clone_repositories(&mut self) -> Result<()>;

    /// The address the host's router gives a name on the operator's private
    /// network (191, 205a, D10a).
    fn route(&self, name: &str) -> Result<Endpoint>;

    /// A token for the App, for the git host calls the machinery makes (207).
    fn app_token(&self) -> Result<String>;

    /// Files the world reports at a path under a repository's shared line —
    /// the blueprints' type files among them (57, 85).
    fn read_file(&self, repository: &str, path: &str) -> Result<Option<Vec<u8>>>;
}

// ------------------------------------------------------------------ workspace

/// How a line lands (53, 175, 179).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LandingPolicy {
    Direct,
    PullRequest,
}

/// Lines, places, merges and landings: the effects of 42. Recorded in phase 1
/// (93a, D8), over `wt` and git in phase 2.
pub trait Workspace {
    fn create_line(&mut self, line: &str, parent: &str) -> Result<()>;

    /// Merge the parent line into this line, never a rebase; aborted whole on
    /// conflict (42, 50, 179).
    fn take_parent(&mut self, line: &str) -> Result<TakeOutcome>;

    fn remove_line(&mut self, line: &str) -> Result<()>;

    fn land_line(&mut self, line: &str, policy: LandingPolicy) -> Result<()>;

    /// Write the acceptance file beside the as-built, before the landing (172,
    /// 181, 192, 203).
    fn write_acceptance(&mut self, line: &str, body: &str) -> Result<()>;

    /// Worktree at the line's head with the closed set of inputs written in
    /// (42, 43, 45, 88, 89).
    fn prepare_place(&mut self, place: &str, line: &str, work_order: &str) -> Result<()>;

    /// Rebase the place onto its line while no session works in it; aborted
    /// whole on conflict (42, 51, 179).
    fn rebase_place(&mut self, place: &str) -> Result<TakeOutcome>;

    /// Merge the place into its line, one place at a time in the fixed order
    /// (38, 42, 54, 179, 227).
    fn merge_place(&mut self, place: &str) -> Result<TakeOutcome>;

    /// Remove the worktree; never while the operator holds it (42, 45, 55).
    fn remove_place(&mut self, place: &str) -> Result<()>;

    /// The endpoints the place's processes serve (46).
    fn endpoints(&self, place: &str) -> Result<Vec<Endpoint>>;
}

/// A take, rebase or merge either completed or was aborted whole (50, 51, 52).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TakeOutcome {
    Done,
    Conflicted { detail: String },
}

// ------------------------------------------------------------------- sessions

/// What the machinery asks a session to do (88, 89).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkOrder {
    pub session: String,
    pub kind: String,
    pub place: String,
    /// The closed set of inputs, rendered (89).
    pub body: String,
}

/// A session as the machinery observes it (72, 196).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionPresence {
    /// The pane is present and working.
    Alive,
    /// The pane is present and idle.
    Idle,
    Absent,
}

/// Starting, observing and ending a session. The operator is the session in
/// phase 1 (93b, D8); a runner and a multiplexer in phase 2.
pub trait Sessions {
    /// Start the named agent in the place, through the one command, in the
    /// multiplexer session the manifest gives it; a second start of the same
    /// name is refused (65, 72, 89, 196).
    fn start_session(&self, order: &WorkOrder) -> Result<()>;

    fn presence(&self, session: &str) -> Result<SessionPresence>;

    /// End the pane; only ever from a state the operator's response or the
    /// type reached (26, 69, 74, 196).
    fn end_session(&self, session: &str) -> Result<()>;

    /// Send an answer to a living session, or start a fresh one whose work
    /// order opens with it (68, 70, 197).
    fn deliver_answer(&self, session: &str, text: &str) -> Result<()>;

    /// Tell an idle session what moved under it (51, 71, 197).
    fn tell_moved(&self, session: &str) -> Result<()>;
}
