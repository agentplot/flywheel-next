//! Unit tests for the answer matcher and the duration parser in `eval`.

use chrono::Duration;
use flywheel_engine::eval::{match_answer, parse_duration};

#[test]
fn bare_word_matches_exactly() {
    assert_eq!(match_answer("yes", "yes"), Some(String::new()));
    assert_eq!(match_answer("yes", "  yes "), Some(String::new()), "surrounding whitespace is ignored");
    assert_eq!(match_answer("drop", "drop"), Some(String::new()));
    assert_eq!(match_answer("yes", "no"), None);
    assert_eq!(match_answer("yes", "yes please"), None, "a bare word takes no argument");
    assert_eq!(match_answer("yes", "Yes"), None, "case matters");
    assert_eq!(match_answer("yes", ""), None);
}

#[test]
fn word_with_argument_binds_the_rest() {
    assert_eq!(match_answer("bolt <name>", "bolt plan-rows"), Some("plan-rows".into()));
    assert_eq!(match_answer("bolt <name>", "bolt   bolt/atlas/plan-rows  "), Some("bolt/atlas/plan-rows".into()));
    assert_eq!(match_answer("bolt <name>", "bolt"), Some(String::new()), "the argument may be empty");
    assert_eq!(match_answer("bolt <name>", "bolts x"), None);
    assert_eq!(match_answer("bolt <name>", "rename x"), None);
    // A multi-word head matches whole, never by one of its words.
    assert_eq!(match_answer("new bolt <name>", "new bolt thing"), Some("thing".into()));
    assert_eq!(match_answer("new bolt <name>", "bolt thing"), None);
    assert_eq!(match_answer("new bolt <name>", "new thing"), None);
    assert_eq!(match_answer("bolt <name>", "new bolt thing"), None, "`bolt` is not the first word");
    assert_eq!(match_answer("pick <letters>", "pick a b c"), Some("a b c".into()));
    // `<text>` alone has an empty head: the whole answer is the argument.
    assert_eq!(match_answer("<text>", "anything at all"), Some("anything at all".into()));
    assert_eq!(match_answer("<text>", ""), Some(String::new()));
}

#[test]
fn word_with_colon_binds_the_text_after_it() {
    assert_eq!(match_answer("redo: <notes>", "redo: split the item"), Some("split the item".into()));
    assert_eq!(match_answer("redo: <notes>", "redo:split"), Some("split".into()));
    assert_eq!(match_answer("redo: <notes>", "redo: a: b"), Some("a: b".into()), "only the first colon splits");
    assert_eq!(match_answer("redo: <notes>", "redo:"), Some(String::new()));
    assert_eq!(match_answer("redo: <notes>", "redo split"), None, "the colon form needs its colon");
    assert_eq!(match_answer("redo: <notes>", "undo: x"), None);
}

#[test]
fn durations_parse_by_unit() {
    assert_eq!(parse_duration("30s"), Some(Duration::seconds(30)));
    assert_eq!(parse_duration("5m"), Some(Duration::minutes(5)));
    assert_eq!(parse_duration("14h"), Some(Duration::hours(14)));
    assert_eq!(parse_duration("7d"), Some(Duration::days(7)));
    assert_eq!(parse_duration("2w"), Some(Duration::weeks(2)));
    assert_eq!(parse_duration("0m"), Some(Duration::zero()));
    assert_eq!(parse_duration(" 10m "), Some(Duration::minutes(10)), "surrounding whitespace is ignored");
}

#[test]
fn malformed_durations_are_none() {
    assert_eq!(parse_duration(""), None);
    assert_eq!(parse_duration("5"), None, "a unit is required");
    assert_eq!(parse_duration("m"), None, "a number is required");
    assert_eq!(parse_duration("5x"), None, "unknown unit");
    assert_eq!(parse_duration("5ms"), None, "milliseconds are not a unit");
    assert_eq!(parse_duration("1.5h"), None, "whole numbers only");
    assert_eq!(parse_duration("five m"), None);
}
