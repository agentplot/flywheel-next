//! A link is written at the host's one address with the instance in the path,
//! whether that is a name on the private network or a localhost port (205a,
//! 308, D10a).

use crate::links;

#[test]
fn a_link_is_written_at_the_hosts_address() {
    // The address the manifest's router gives the host, with the instance in
    // the path: this is what a link opens.
    let address = "http://mac-mini.tailnet.ts.net/willdan";
    let link = links::to_object(address, "unit/atlas/u").expect("a link");
    assert_eq!(link, "http://mac-mini.tailnet.ts.net/willdan/unit/atlas/u");
    assert!(link.contains("/willdan/"), "the instance is in the path");
    assert!(!links::is_localhost(&link));

    // A host that serves this computer alone writes its links at its localhost
    // port, where they open (191, 245, D10a).
    for machine_only in [
        "http://localhost:4242/willdan",
        "http://127.0.0.1:4242/willdan",
        "http://[::1]:4242/willdan",
    ] {
        assert!(links::is_localhost(machine_only));
        let link = links::to_object(machine_only, "unit/atlas/u").expect("a link at the machine");
        assert_eq!(link, format!("{machine_only}/unit/atlas/u"));
        assert_eq!(links::to_page(&format!("{machine_only}/")).unwrap(), machine_only);
    }
}

/// A link to a host that is away says so rather than failing silently
/// (308, 150a).
#[test]
fn away_link_says_so() {
    let said = links::away(
        "http://mac-mini.tailnet.ts.net/willdan",
        "unit/atlas/u",
        "mac-mini",
        "2026-09-08T09:00:00Z",
    )
    .expect("a link");
    assert!(said.starts_with("http://mac-mini.tailnet.ts.net/willdan/unit/atlas/u"));
    assert!(said.contains("mac-mini is away since"), "{said}");
}
