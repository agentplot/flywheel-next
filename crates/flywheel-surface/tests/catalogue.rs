//! The catalogue is one object, and the transport is a transport (193, D9).

use serde_json::Value;

mod caller;

/// The in-process caller and the HTTP caller each enumerate the catalogue, and
/// they list the same tools with the same schemas — so a further transport adds
/// a client and not an operation (193).
#[test]
fn catalogue_is_identical_across_callers() {
    let in_process = flywheel_surface::enumerate();
    let over_http: Value = caller::get("/api/tools");
    assert_eq!(
        in_process, over_http,
        "the HTTP caller enumerated a different catalogue from the in-process caller"
    );

    // And it is a catalogue, not an empty list that would make the equality
    // above vacuous.
    let tools = over_http["tools"].as_array().expect("tools is a list");
    assert!(!tools.is_empty(), "the catalogue served no tools");
    for tool in tools {
        assert!(tool["name"].as_str().is_some_and(|n| !n.is_empty()));
        assert!(
            tool["args"].is_array(),
            "{} carries no argument schema",
            tool["name"]
        );
    }
}

/// Every argument is named, and no two tools share a name (193).
#[test]
fn every_tool_names_its_arguments() {
    let mut seen: Vec<&str> = vec![];
    for tool in flywheel_surface::catalogue() {
        assert!(
            !seen.contains(&tool.name),
            "`{}` is in the catalogue twice",
            tool.name
        );
        seen.push(tool.name);
        for arg in tool.args {
            assert!(!arg.is_empty(), "`{}` has an unnamed argument", tool.name);
        }
        assert!(!tool.doc.is_empty(), "`{}` says nothing it does", tool.name);
    }
}
