//! The fakes the first tier binds (D17).
//!
//! A fake, not a mock: each behaves the way the real implementation must — a
//! `put` against a stale sequence is rejected, an effect written twice is
//! written once, a write outside the machinery's prefix is refused. Nothing
//! here is told what to expect and nothing records calls.
//!
//! None of it is a profile. It is never named by `--profile`, never bound in a
//! `profiles/` file and never present in the acceptance set: what 92 retired
//! was a second store *profile* claiming conformance, and this claims nothing.
//!
//! Behind the `testing` feature, and a dev-dependency of the crates that use it.

mod store;
mod world;

pub use store::{start_of_time, FakeStore};
pub use world::FakeWorld;
