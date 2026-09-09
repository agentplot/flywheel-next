//! The acceptance table of proposal.md, as this repository's tests read it.
//!
//! What a run *skips* is decided by the scenario file and by no list held
//! outside it (93a, D15). What the phase *lists* is this: the scenarios it
//! accepts, the ones a recorded workspace cannot serve, and the ones it defers
//! for reasons of its own. It is here because a scenario the table lists that
//! every configuration skips is a failure and never a silent pass, and because
//! the 390px pass runs over the accepted ones (314).

#![allow(dead_code)]

/// The twenty-one scenarios phase 1 accepts (proposal.md — The acceptance).
pub const ACCEPTED: &[&str] = &[
    "S01", "S02", "S04", "S05", "S06", "S07", "S08", "S13", "S16", "S17", "S18", "S19", "S20",
    "S21", "S22", "S23", "S24", "S29", "X01", "X05", "X08",
];

/// The nine that assert a real take, merge, rebase, conflict or landing, which
/// a recorded workspace cannot serve (93a).
pub const REAL_WORKSPACE: &[&str] = &[
    "S14", "S15", "S26", "S32", "S33", "S34", "X03", "X06", "X09",
];

/// The fifteen the phase defers for reasons of its own — a pane runner, the
/// tracker, the ledger, the dispatcher, construction itself — rather than for
/// anything a binding provides (proposal.md — Deferred, with the reason).
pub const DEFERRED: &[&str] = &[
    "S03", "S09", "S10", "S11", "S12", "S25", "S27", "S28", "S30", "S31", "T01", "X02", "X04",
    "X07", "X10",
];
