//! flywheel-domain: the object envelope and the domain's record schemas,
//! written over the engine's generic rec reader and writer. The names of the
//! seven objects live here and not in the engine (86, I13).

pub mod envelope;
pub mod records;

pub use envelope::{from_record, read_all, to_record, write_all, ENVELOPE_FIELDS};
