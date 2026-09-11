//! flywheel-scenario: the conformance runner and the stand-ins clause 93 and
//! 93a admit — the scripted sessions, the recorded world and workspace. The
//! state store is the git-only profile's, against a bare repository on the same
//! computer (92); the engine, the register, the tail and the effects run for
//! real.

pub mod bindings;
pub mod delivery;
pub mod console;
pub mod conformance;
pub mod runner;
pub mod scenario;
pub mod sessions;
pub mod statestore;
pub mod store;
pub mod world;

pub use runner::Runtime;
pub use store::Store;

#[cfg(test)]
mod tests;
