//! The links every chat rendering, notification and rail line carries (308).
//!
//! One address per host, with the instance in the path, and every link is
//! written at it: a name on the operator's private network, or a localhost
//! port when the host serves this computer alone (205a, 308, D10a, 191). A
//! link at localhost opens nothing on a phone, which is why the operator gives
//! a host its name when the phone should answer; it is never refused here.

use anyhow::Result;

/// Whether an address is the machine's own rather than a name on the operator's
/// private network.
pub fn is_localhost(address: &str) -> bool {
    let address = address.to_ascii_lowercase();
    address.contains("localhost") || address.contains("127.0.0.1") || address.contains("[::1]")
}

/// A link to an object, at the host's address with the instance already in it.
///
/// The address is the host's, resolved from the manifest's router by
/// `flywheel-world-host`; this only writes the link (205a, 308, D10a).
pub fn to_object(address: &str, object: &str) -> Result<String> {
    Ok(format!(
        "{}/{}",
        address.trim_end_matches('/'),
        object.trim_start_matches('/')
    ))
}

/// A link to the page itself, which every chat rendering carries (18, 308).
pub fn to_page(address: &str) -> Result<String> {
    Ok(address.trim_end_matches('/').to_string())
}

/// What a link to an away host says instead of failing silently (308, 150a).
pub fn away(address: &str, object: &str, host: &str, since: &str) -> Result<String> {
    let link = to_object(address, object)?;
    Ok(format!("{link} — {host} is away since {since}"))
}
