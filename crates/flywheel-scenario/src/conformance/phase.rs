//! The acceptance table of proposal.md, as the runner reads it.
//!
//! What a run *skips* is decided by the scenario file and by no list held
//! outside it (93a, D15). What the phase *lists* is this: the scenarios it
//! accepts, the ones a recorded workspace cannot serve, and the ones it defers
//! for reasons of its own. A scenario the table lists that a run skips is a
//! failure of that run and never a silent pass (tasks 2.15, 11.4), and the
//! 390px pass runs over the accepted ones (314).

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

/// The three accepted rows that declare `requires: [real-hosts]`: lease
/// contention between hosts, a second host reading the holder, a reconnect —
/// what one in-process host cannot produce. The runner plays them under
/// `--hosts real` rather than skipping them (D15, 93a).
pub const REAL_HOSTS: &[&str] = &["S13", "S17", "S18"];

/// The fifteen the phase defers for reasons of its own — a pane runner, the
/// tracker, the ledger, the dispatcher, construction itself — rather than for
/// anything a binding provides (proposal.md — Deferred, with the reason).
pub const DEFERRED: &[&str] = &[
    "S03", "S09", "S10", "S11", "S12", "S25", "S27", "S28", "S30", "S31", "T01", "X02", "X04",
    "X07", "X10",
];

/// Whether the acceptance table lists a scenario, by the name its file has.
pub fn accepted(name: &str) -> bool {
    ACCEPTED.contains(&name)
}

/// Whether the phase defers a scenario to a later one, by the name its file
/// has: the rows a recorded workspace cannot serve and the rows it defers for
/// reasons of its own (proposal.md — Deferred, with the reason). Such a row is
/// played and reported, and gates nothing until its phase opens (roadmap, phase
/// gates).
pub fn deferred(name: &str) -> bool {
    REAL_WORKSPACE.contains(&name) || DEFERRED.contains(&name)
}
