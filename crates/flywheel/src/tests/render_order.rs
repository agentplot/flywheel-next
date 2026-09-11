//! `flywheel render-order`: the exact prompt a session would be handed, with
//! no session started (90, 124), and two instruction versions told apart (123).

use crate::render_order::{render, render_order};
use flywheel_domain::order::SECTIONS;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn scenario() -> PathBuf {
    root().join("conformance/scenarios/S01.yaml")
}

/// The inputs are closed: the schema instruction, the type skill, the work
/// order and the change's artifacts, and nothing else reaches the session (89).
#[test]
fn render_order_inputs_are_closed() {
    let order = render("self-closing", 1, &scenario(), None).expect("the order renders");

    // The four sections of 89, and no fifth.
    let text = render_order("self-closing", 1, &scenario(), None).expect("the order renders");
    let sections: Vec<&str> = text
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .collect();
    assert_eq!(sections, SECTIONS, "the order carries the four inputs of 89 and no fifth");

    // Each one is there and says something.
    assert!(
        !order.schema_instruction.is_empty(),
        "the schema of each deliverable the type names, and the defaults it carries (120, 190)"
    );
    assert!(
        order
            .schema_instruction
            .iter()
            .any(|i| i.path == "flywheel/schemas/claim.md"),
        "self-closing names claim among its deliverables"
    );
    assert!(
        order
            .schema_instruction
            .iter()
            .any(|i| i.path == "flywheel/instructions/design-conclusion.md"),
        "a design type carries the default instructions of 120"
    );
    assert!(
        order
            .type_skill
            .iter()
            .any(|i| i.path == "flywheel/skills/self-closing/SKILL.md"),
        "the type skill, keyed by the agent name (ruling 3)"
    );
    assert!(
        order.deliverables.iter().any(|d| d.name == "book-chapter"),
        "the deliverables table names each with its producer, schema and surface (190)"
    );
    assert!(!order.change_artifacts.is_empty(), "the artifacts of the change it works");

    // The header names the versions in force, so two sessions either side of a
    // chore are told apart by it (123, 226).
    assert!(text.contains("instruction set: 1"), "{text}");
    assert!(text.contains("deliverables binding: 3"), "{text}");
    for input in order.inputs() {
        assert!(text.contains(&input), "the header names `{input}`");
        assert!(input.contains('@'), "an input is named `name@version`: {input}");
    }

    // And nothing else: what the row says must never reach the session is a
    // rule about the place and the tools and is no part of the prompt (89).
    let row = flywheel_domain::context::Binding::load(&flywheel_domain::profile::Embedded)
        .expect("the context binding loads")
        .row("self-closing")
        .expect("self-closing has a row");
    let never = row.never.expect("the row says what never reaches it");
    assert!(!text.contains(&never), "the `never` column is not written into the order");
    assert!(
        !text.contains(&row.charged_by),
        "what charges the session is not one of its inputs"
    );
    assert!(
        !text.contains("## identity"),
        "the identity column is not a fifth input"
    );

    // No session was started: the render reads the seeded stores and writes
    // nothing (90).
    assert!(text.contains("job: elaboration/"), "the job is the scenario's object: {text}");
}

/// An instruction changes, and the prompt rendered before and after names
/// different versions, so a session started before the chore and one after can
/// be told apart (123, 91).
#[test]
fn render_order_two_versions_differ() {
    let before = render_order("self-closing", 1, &scenario(), None).expect("version 1 renders");

    let dir = std::env::temp_dir().join(format!("flywheel-instructions-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    copy_tree(&root().join("instructions"), &dir);

    // The chore: the skill's text moves and its version moves with it, and the
    // release's set version moves too.
    let skill = dir.join("skills/self-closing/SKILL.md");
    let text = std::fs::read_to_string(&skill).expect("the skill is readable");
    std::fs::write(
        &skill,
        text.replace("version: 1", "version: 2")
            + "\nOne more thing this type is told, added by a chore on the blueprints.\n",
    )
    .expect("the copy is writable");
    let set = dir.join("set.yaml");
    let text = std::fs::read_to_string(&set).expect("set.yaml is readable");
    std::fs::write(&set, text.replace("\nversion: 1", "\nversion: 2")).expect("writable");

    let after = render_order("self-closing", 2, &scenario(), Some(&dir)).expect("version 2 renders");

    assert_ne!(before, after, "the two renderings differ");
    assert!(before.contains("instruction set: 1"));
    assert!(after.contains("instruction set: 2"));
    assert!(
        after.contains("self-closing@2"),
        "the changed skill is named at its new version"
    );
    assert!(
        !before.contains("self-closing@2"),
        "the rendering before the chore names the older version"
    );
    assert!(after.contains("added by a chore on the blueprints"));

    // A version the set does not carry is refused rather than rendered against
    // whatever is at hand.
    let refused = render_order("self-closing", 3, &scenario(), Some(&dir));
    assert!(refused.is_err(), "a version the set does not carry is refused");

    let _ = std::fs::remove_dir_all(&dir);
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("the copy is made");
    for entry in std::fs::read_dir(from).expect("the directory is readable").flatten() {
        let path = entry.path();
        let target = to.join(path.file_name().expect("a name"));
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            std::fs::copy(&path, &target).expect("the file copies");
        }
    }
}
