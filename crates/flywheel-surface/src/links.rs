//! The links every chat rendering, notification and rail line carries (308).
//!
//! One address per host — the private-network name its router gives it, with
//! the instance in the path — and never a localhost port, which opens nothing
//! on a phone (205a, 308, D10a). The port the operator at the machine uses is
//! served all the same; it is simply never what a link names (245).

use anyhow::{bail, Result};

/// Whether an address is the machine's own rather than a name on the operator's
/// private network.
pub fn is_localhost(address: &str) -> bool {
    let address = address.to_ascii_lowercase();
    address.contains("localhost") || address.contains("127.0.0.1") || address.contains("[::1]")
}

/// A link to an object, at the host's address with the instance already in it.
///
/// The address is the host's, resolved from the manifest's router by
/// `flywheel-world-host`; this only writes the link and refuses one that would
/// name a localhost port (205a, 308, D10a).
pub fn to_object(address: &str, object: &str) -> Result<String> {
    if is_localhost(address) {
        bail!(
            "a link would name `{address}`, which opens nothing on a phone; a link names the \
             host's private-network address with the instance in the path (205a, 308, D10a)"
        );
    }
    Ok(format!(
        "{}/{}",
        address.trim_end_matches('/'),
        object.trim_start_matches('/')
    ))
}

/// What a link to an away host says instead of failing silently (308, 150a).
pub fn away(address: &str, object: &str, host: &str, since: &str) -> Result<String> {
    let link = to_object(address, object)?;
    Ok(format!("{link} — {host} is away since {since}"))
}
