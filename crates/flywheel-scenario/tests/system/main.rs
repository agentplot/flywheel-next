//! The system tier: the real `flywheel` binary, hosts that are processes of
//! their own, and a headless browser (D17).
//!
//! These cost what they cost and they run at merge, never on every change.
//! They are behind `required-features = ["system-tests"]` rather than
//! `#[ignore]`, so the everyday run saves their compile time as well as their
//! wall time and nothing is marked ignored to hide a cost.

#[path = "../driver/mod.rs"]
mod driver;
#[path = "../phase/mod.rs"]
mod phase;

mod acceptance_sets;
mod phone;
mod real_hosts;
mod scripted;
