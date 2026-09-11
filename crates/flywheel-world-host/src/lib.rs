//! flywheel-world-host: `World` over git, the manifest and the host's router.
//!
//! `profiles/host.yaml` is this crate's specification. It is one of the three
//! doors on to the world the effects act on (D8), and a different door from the
//! one durable state goes through (125): what it reads and writes is the git
//! host, the manifest and the router, never the state store.

pub mod bootstrap;
pub mod effects;
pub mod git;
pub mod join;
pub mod manifest;
pub mod prefix;
pub mod world;

pub use manifest::Manifest;
pub use world::HostWorld;

#[cfg(test)]
mod tests;
