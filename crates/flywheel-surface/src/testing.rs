//! The fakes this crate's tests bind. The `World` of files in memory is
//! `flywheel-atoms`', so the page's tests and the domain's read the same fake
//! and a prefix rule cannot be looser in one than in the other (D17, 203).

pub use flywheel_atoms::testing::FakeWorld as Files;
