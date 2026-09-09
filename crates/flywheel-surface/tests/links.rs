//! A link names the host's private-network address, never a localhost port
//! (205a, 308, D10a).

use flywheel_surface::links;

#[test]
fn link_never_names_localhost() {
    // The address the manifest's router gives the host, with the instance in
    // the path: this is what a link opens.
    let address = "http://mac-mini.tailnet.ts.net/willdan";
    let link = links::to_object(address, "unit/atlas/u").expect("a link");
    assert_eq!(link, "http://mac-mini.tailnet.ts.net/willdan/unit/atlas/u");
    assert!(link.contains("/willdan/"), "the instance is in the path");
    assert!(!links::is_localhost(&link));

    // The port the operator at the machine uses is served, and is never what a
    // link names: it opens nothing on a phone (245, 306).
    for machine_only in [
        "http://localhost:4242/willdan",
        "http://127.0.0.1:4242/willdan",
        "http://[::1]:4242/willdan",
    ] {
        assert!(links::is_localhost(machine_only));
        let refused = links::to_object(machine_only, "unit/atlas/u")
            .expect_err("a link naming the machine's own address is refused");
        assert!(
            format!("{refused}").contains("opens nothing on a phone"),
            "{refused}"
        );
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
