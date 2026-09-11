//! What a seed refuses to put in place (149, 205, 206).

use crate::seed::covered;
use flywheel_domain::derived::Declaration;
use flywheel_engine::Object;
use serde_json::json;

fn a_unit(id: &str, repository: &str) -> Object {
    Object {
        id: id.into(),
        machine: "unit".into(),
        parent: None,
        config: [("life".to_string(), "proposed".to_string())].into_iter().collect(),
        entered_at: Default::default(),
        record: [("repository".to_string(), json!(repository))].into_iter().collect(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    }
}

fn declaring(repositories: &[&str]) -> Declaration {
    Declaration {
        repositories: repositories.iter().map(|r| r.to_string()).collect(),
        types: vec![],
        kinds: vec!["all".into()],
    }
}

/// A seed that would put an object no declaration covers in place is refused,
/// and the refusal names the repository and the object that named it.
///
/// 149 makes such an object a decision under attention, which is right — the
/// machinery will act on none of it. But when the seed put it there, the
/// operator can answer that decision with nothing but "seen": the described
/// instance arrives with one such card per object instead of the decisions it
/// was written to show. The seed is the thing to fix, so it says so.
#[test]
fn a_seed_no_declaration_covers_is_refused_with_what_is_missing() {
    let refusal = covered(
        &[
            a_unit("unit/atlas/status-writer", "atlas"),
            a_unit("unit/storefront/checkout", "storefront"),
        ],
        &declaring(&["atlas"]),
    )
    .expect_err("a seed the declaration does not cover is refused");
    let said = format!("{refusal:#}");
    assert!(said.contains("`storefront`"), "{said}");
    assert!(said.contains("unit/storefront/checkout"), "{said}");
    assert!(
        !said.contains("atlas"),
        "the refusal named a repository the declaration covers: {said}"
    );
}

/// What the declaration covers goes in, and so does the machinery's own: the
/// rail, the hosts, the sinks and the bindings' facts belong to whichever host
/// runs and are covered by every declaration (D5, 149).
#[test]
fn a_seed_the_declaration_covers_goes_in() {
    let mut rail = a_unit("rail", "");
    rail.machine = "rail".into();
    rail.record.clear();
    covered(
        &[a_unit("unit/atlas/status-writer", "atlas"), rail],
        &declaring(&["atlas"]),
    )
    .expect("the declaration covers it");
}
