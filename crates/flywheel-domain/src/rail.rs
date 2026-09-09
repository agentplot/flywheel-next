//! The rail as an object (148, `engine/rail.yaml`).
//!
//! Its record is the register and the status projection's as-of point; it is
//! not a second store. The object itself is made from that record at the top of
//! every tick, the way a lease object is, so the machine ticks over it while
//! the record stays the one place the register lives.

use chrono::{DateTime, Utc};
use flywheel_atoms::{Object, Records};
use flywheel_engine::Definitions;
use std::collections::BTreeMap;

pub use crate::RAIL;

/// The rail object, initialised where its record carries no state yet.
pub fn attach<S: Records>(
    store: &S,
    defs: &Definitions,
    objects: &mut BTreeMap<String, Object>,
    now: DateTime<Utc>,
) -> anyhow::Result<()> {
    let Some(mut rail) = store.get(RAIL)? else {
        return Ok(());
    };
    if rail.config.is_empty() {
        flywheel_engine::initialise(defs, &mut rail, now);
    }
    objects.insert(RAIL.to_string(), rail);
    Ok(())
}
