//! flywheel-surface: the tool catalogue and the page.
//!
//! The catalogue is the one write path into the flywheel (193): every
//! operation the operator may invoke is one tool with a schema naming its
//! arguments by object id, and the page, the chat and the machinery's own
//! commands call the same functions. The transport is a transport — this phase
//! serves the catalogue over HTTP for the page and calls it in-process for the
//! machinery — and never a second write path (193, D9).
//!
//! Everything here is written over `StateStore` and the commands of
//! `flywheel-domain`, so it holds no store of its own and reaches no field of
//! one (125, 136).

pub mod catalogue;
pub mod chat;
pub mod http;
pub mod links;
pub mod page;

pub use catalogue::{catalogue, enumerate, tool, Tool, CATALOGUE};

/// Fakes the tests bind: a `World` held in memory, enforcing the same prefix
/// rule a host does. Behind the `testing` feature, and always present for this
/// crate's own tests (D17).
#[cfg(any(feature = "testing", test))]
pub mod testing;

#[cfg(test)]
mod tests;
