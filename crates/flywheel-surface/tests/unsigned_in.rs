//! A single-operator host on a private network serves the page unsigned-in, and
//! every response names that operator (253a, 153, 236a).

mod caller;
mod store;

use flywheel_atoms::{Records, StateStore};
use flywheel_surface::http::Served;
use serde_json::json;

const ADDRESS: &str = "http://studio.tailnet.ts.net/willdan";

fn a_page(name: &str, operators: &[&str]) -> (store::Sandbox, caller::Server) {
    let sandbox = store::Sandbox::new(name);
    let listed: Vec<String> = operators.iter().map(|o| o.to_string()).collect();
    let page = caller::Server::for_operators(
        sandbox.store(),
        flywheel_domain::set::load().expect("the embedded definitions"),
        &listed,
        ADDRESS,
    );
    (sandbox, page)
}

/// While the operators list holds one entry the page is served with no sign-in
/// on the operator's private network, and that entry is the identity every
/// response records as given by, with when (253a, 153, 236a).
#[test]
fn unsigned_in_single_operator() {
    let (_sandbox, page) = a_page("unsigned-in", &["chuck"]);

    let html = page.html("/");
    assert!(html.contains("id=\"decisions\""), "served unsigned-in: {html}");
    assert!(
        html.contains("class=\"given-by\">chuck<"),
        "the page names the operator every response will record"
    );

    let answered = page.form("/api/tools/capture", &[("text", "look at the rows")]);
    assert_eq!(answered["recorded"], json!(true), "{answered}");

    page.with_store(|store| {
        let responses = StateStore::list(store, &flywheel_atoms::Scope::Machine("response".into()))
            .expect("a listing")
            .objects;
        assert_eq!(responses.len(), 1);
        assert_eq!(
            responses[0].record.get("given_by").and_then(|v| v.as_str()),
            Some("chuck"),
            "the response names the operators list's single entry"
        );
        assert!(
            responses[0].record.contains_key("given_at"),
            "and when it was given"
        );
        let _ = Records::get(store, &responses[0].id);
    });

    // The operator at the machine reaches the same page on the port 245
    // permits, and nowhere else is served unsigned-in (245, 155).
    let at_the_machine = page.html_at("/", "localhost:4242");
    assert!(at_the_machine.contains("id=\"decisions\""), "{at_the_machine}");
}

/// The exception closes as soon as a second operator is listed, and the host
/// says why (253a).
#[test]
fn refuses_on_second_operator() {
    let (_sandbox, page) = a_page("second-operator", &["chuck", "sam"]);

    let html = page.html("/");
    assert!(
        html.contains("not served unsigned-in"),
        "the host refuses and says why: {html}"
    );
    assert!(html.contains("253a"), "and cites the clause: {html}");
    assert!(!html.contains("id=\"decisions\""), "nothing is served");

    // And no call is taken either: the refusal is the host's, not the page's.
    let refused = page.form("/api/tools/capture", &[("text", "anything")]);
    assert!(
        refused["refused"]
            .as_str()
            .is_some_and(|s| s.contains("unsigned-in")),
        "{refused}"
    );
}

/// The page is not served unsigned-in at any address but the operator's private
/// network — and never at one published beyond it (253a, 155, 46).
#[test]
fn refuses_at_another_address() {
    let (_sandbox, page) = a_page("another-address", &["chuck"]);
    for elsewhere in ["flywheel.example.com", "203.0.113.7", "localhost:9999"] {
        let html = page.html_at("/", elsewhere);
        assert!(
            html.contains("not served unsigned-in") || html.contains("neither the host's"),
            "reached at `{elsewhere}`: {html}"
        );
        assert!(!html.contains("id=\"decisions\""), "at `{elsewhere}`");
    }
}

/// The host binds its private-network address and a localhost port for the
/// operator at the machine, and nothing else; the manifest names no
/// publication (46, 155, 245).
#[test]
fn binds_two_addresses_only() {
    let sandbox = store::Sandbox::new("bindings");
    let served = Served::for_operators(
        sandbox.store(),
        flywheel_domain::set::load().expect("the embedded definitions"),
        &["chuck".to_string()],
        ADDRESS,
    );
    let bound = served.bound();
    assert_eq!(
        bound,
        vec![
            "studio.tailnet.ts.net:80".to_string(),
            "127.0.0.1:4242".to_string()
        ],
        "the host binds its private-network address and the operator's own port"
    );
    assert_eq!(bound.len(), 2, "and no other address: {bound:?}");

    // Neither of them is a wildcard, which would be a publication the operator
    // never asked for (46).
    for address in &bound {
        assert!(!address.starts_with("0.0.0.0"), "{address}");
        assert!(!address.starts_with("[::]"), "{address}");
    }

    // The private-network address is what a link names; the port never is
    // (205a, 308).
    assert!(!flywheel_surface::links::is_localhost(&served.address));
    assert_eq!(served.operator(), "chuck");
}
