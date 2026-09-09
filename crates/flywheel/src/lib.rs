//! flywheel: the binary. What its commands do lives here so a test can call
//! the same functions the command line does.

pub mod host;
pub mod init;
pub mod report;

/// The commands, written against the trait surface. They live in
/// `flywheel-scenario` so the conformance runner ticks the same code the
/// command line does, over whichever store the profile bound (125, D15).
pub use flywheel_scenario::console;
