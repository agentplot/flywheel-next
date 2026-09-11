//! The binary's own tests that need no repository: the crate boundaries this
//! workspace keeps, the work order a session is handed, the console's reading
//! and the report a session writes. The first tier (D17).

mod boundaries;
mod console;
mod render_order;
mod report;
