//! flywheel-domain: the object envelope and the domain's record schemas,
//! written over the engine's generic rec reader and writer. The names of the
//! seven objects live here and not in the engine (86, I13).

pub mod blueprints;
pub mod commands;
pub mod envelope;
pub mod profile;
pub mod set;
pub mod signals;
pub mod sinks;
pub mod status;
pub mod derived;
pub mod leases;
pub mod rail;
pub mod records;
pub mod regions;

/// The rail's own record: the register, and the decisions standing after the
/// last derive (`profiles/record-derived.yaml`).
pub const RAIL: &str = "rail";

pub use envelope::{from_record, read_all, to_record, write_all, ENVELOPE_FIELDS};
