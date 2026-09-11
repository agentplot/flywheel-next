//! flywheel-atoms: the names the domain is written in, and the seams the phases
//! attach to.
//!
//! Two things live here and nothing else: the evidence and effect registries
//! generated from `definitions/atoms.yaml` (87, 138), and the four traits a
//! host binds an implementation to — `StateStore`, `World`, `Workspace` and
//! `Sessions` (D8). No implementation of any of them is in this crate.

pub mod atoms;
pub mod conformance;
pub mod scenario;
pub mod traits;

pub use atoms::{AtomType, Effect, EffectAtom, Evidence, EvidenceAtom};
pub use traits::{
    Cost, EffectWrite, Endpoint, EvidenceRead, HostRecord, LandingPolicy, LeaseOp, LeaseOutcome,
    LeaseRecord, Listing, Notice, Object, Presentation, PutOutcome, ReadPoint, Received, Records,
    RepositoryRef, Scope, SessionPresence, Sessions, StateStore, StatusView, TakeOutcome,
    ThreadEntry, WorkOrder, Workspace, World, WriteOutcome,
};
