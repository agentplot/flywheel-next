//! The commands, written against the trait surface.
//!
//! They live in `flywheel-domain`, over `StateStore` and the six record
//! operations under it, so every caller writes the same way: the command line,
//! the conformance runner, the tool catalogue and the page. This module is the
//! name the runner and the binary have always reached them by (125, 136, 139).

pub use flywheel_domain::commands::*;
