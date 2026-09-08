//! flywheel-store-git: `StateStore` over the state repository.
//!
//! `profiles/git-only.yaml` is this crate's specification. Every durable fact
//! is a commit on `<instance>/flywheel-state`'s shared line, and the record is
//! readable as files with no host running (160, 161, 132, 145).
//!
//! The layout, from the profile:
//!
//! | what | where |
//! |---|---|
//! | an object's record | `objects/<id>/object.rec` |
//! | its thread | `objects/<id>/thread.rec` |
//! | a response | `responses/<delivery id>.rec` |
//! | an ask | `asks/<id>.rec` |
//! | a run record | `runs/<host>/<date>.rec` |
//! | the status view | `status.html` |
//! | a lease | branch `lease/<object id>`, one orphan commit |
//! | a heartbeat | branch `host/<host id>`, the same way |
//!
//! Leases and heartbeats are never files on `main`, so `main`'s history is
//! state changes and nothing else and stays a readable audit record after
//! months of minute-by-minute renewals (167, D5).

pub mod git;
pub mod layout;
pub mod store;

pub use store::GitStore;
