//! What an elaboration's session is handed: the intent's question, the signals
//! it rests on in their own words, the claims they argue with and where those
//! stand in the book, and what its type delivers (116, 188, 190; context.yaml
//! sessions.self-closing).

use crate::{changes, order, signals};
use flywheel_atoms::testing::FakeWorld;

#[test]
fn an_elaborations_order_carries_its_question_signals_claims_and_deliverables() {
    let defs = crate::set::load().unwrap();
    let mut world = FakeWorld::new();
    world.files.insert(
        "openspec/specs/page/spec.md".into(),
        "## Purpose\n\n### Requirement: The page is a capture surface\n\nThe box SHALL…\n".into(),
    );
    let signal = signals::Signal {
        id: "signal/page-1".into(),
        capture: "capture/page-1".into(),
        kind: "ask".into(),
        asserted_by: "Amy".into(),
        subject_tags: vec!["offering".into()],
        assertion: "Amy asked how the pipeline gets baked in so everyone can use it.".into(),
        excerpt: "\"how does it get baked in\"".into(),
        position: "line 12".into(),
        argues_with: vec!["page/the-page-is-a-capture-surface".into()],
    };
    let inputs = order::Elaborating {
        intent: "intent/flywheel-offering".into(),
        subject: "What Flywheel is offered as, to whom and in what order".into(),
        signals: vec![signal],
        challenges: vec![],
        covers: vec![("intent/work-outside-git".into(), "Whether a flywheel's work may be held anywhere but git".into())],
        claims: changes::standing_claims(&signals::Blueprints(&world)),
        change_directory: "openspec/changes/flywheel-offering/".into(),
        deliverables: order::type_deliverables(&defs, "self-closing"),
    };
    assert_eq!(
        inputs.deliverables,
        vec!["book-chapter", "claim", "context-map", "conceptual-diagram", "logical-diagram"]
    );
    let rendered = order::elaboration(&inputs);
    for shown in [
        "## the question\n\nWhat Flywheel is offered as, to whom and in what order · `intent/flywheel-offering`",
        "- `signal/page-1` · ask · Amy · subjects: offering · argues with: page/the-page-is-a-capture-surface",
        "  Amy asked how the pipeline gets baked in so everyone can use it.",
        "  > \"how does it get baked in\"",
        "- `intent/work-outside-git` · Whether a flywheel's work may be held anywhere but git",
        "- `page/the-page-is-a-capture-surface` · The page is a capture surface · openspec/specs/page/spec.md",
        "`openspec/changes/flywheel-offering/` on the intent's line",
        "book-chapter, claim, context-map, conceptual-diagram, logical-diagram, written in this place",
    ] {
        assert!(rendered.contains(shown), "the order lacks `{shown}`: {rendered}");
    }
    assert!(!rendered.contains("## the claims it challenges"), "a section with nothing in it: {rendered}");
}
