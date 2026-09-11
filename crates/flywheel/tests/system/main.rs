//! The system tier: the real `flywheel` binary (D17).
//!
//! These cost what they cost and they run at merge, never on every change.
//! They are behind `required-features = ["system-tests"]` rather than
//! `#[ignore]`, so the everyday run saves their compile time as well as their
//! wall time and nothing is marked ignored to hide a cost.

mod definitions;
mod walkthrough;
