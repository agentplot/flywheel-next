//! The store's own tests. A temp bare repository and a checkout of it are this
//! crate's subject rather than a dependency of something else, and a tick
//! against them spends at most one process, so they are unit tests and hold to
//! the first tier's bar (D17, 169).

mod disconnected;
mod records;
