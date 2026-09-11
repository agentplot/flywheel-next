//! What a session reports, and the one path it reports through.
//!
//! The report itself is `flywheel-domain`'s, because it is a record of the
//! domain and because the page's curator surface writes the same one the
//! command does (67, 93b, D16). `flywheel exit | offer | note | refuse` is this
//! module's caller and nothing here is a second path.

pub use flywheel_domain::report::{write_report, Report, Reported, EXITS, OFFERS, SESSION_ENV};
