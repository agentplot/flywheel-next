//! The address `flywheel init` registers a host at (205a, 308, D10a).

use crate::init::address_of;

#[test]
fn no_name_given_is_localhost_at_the_pages_port() {
    assert_eq!(address_of(None, 4242), "http://localhost:4242");
    assert_eq!(address_of(Some("  "), 5150), "http://localhost:5150");
}

#[test]
fn a_bare_name_is_taken_as_http_at_the_pages_port() {
    assert_eq!(address_of(Some("mac-studio"), 4242), "http://mac-studio:4242");
    assert_eq!(
        address_of(Some("mac-studio.tailnet.example"), 4242),
        "http://mac-studio.tailnet.example:4242"
    );
}

#[test]
fn a_given_address_gains_the_port_it_does_not_name_and_keeps_one_it_does() {
    assert_eq!(address_of(Some("http://mac-studio.local/"), 4242), "http://mac-studio.local:4242");
    assert_eq!(address_of(Some("http://mac-studio.local:8080"), 4242), "http://mac-studio.local:8080");
    assert_eq!(address_of(Some("http://[::1]"), 4242), "http://[::1]:4242");
}
