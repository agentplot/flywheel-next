//! The status view in its forms: every kind of object has one of its own and no
//! two kinds share one, and an object is drawn inside the one it is part of
//! (209, S62).

use crate::status::{self, Row, Status};
use chrono::{TimeZone, Utc};
use flywheel_atoms::ReadPoint;

fn row(object: &str, machine: &str, group: &str, parent: Option<&str>, created: u64) -> Row {
    Row {
        object: object.into(),
        machine: machine.into(),
        group: group.into(),
        states: vec![],
        said: String::new(),
        holder: None,
        runner: None,
        liveness: None,
        lease: None,
        discussion: vec![],
        parent: parent.map(String::from),
        created,
        words: None,
    }
}

fn rendered(rows: Vec<Row>) -> String {
    let at = Utc.with_ymd_and_hms(2026, 9, 15, 9, 0, 0).unwrap();
    let view = status::render(&Status {
        as_of: ReadPoint { mark: "abc123".into(), seq: 1, at },
        rows,
        unmoved: vec![],
        at,
    });
    view.body
}

/// The opening tag of the element drawn for an object.
fn opening<'a>(body: &'a str, object: &str) -> &'a str {
    let at = body
        .find(&format!("id=\"{object}\""))
        .unwrap_or_else(|| panic!("{object} is not drawn:\n{body}"));
    let start = body[..at].rfind('<').expect("an opening tag");
    let end = at + body[at..].find('>').expect("the tag closes");
    &body[start..=end]
}

#[test]
fn every_kind_has_its_own_form_and_parts_hang_inside() {
    let mut capture = row("capture/page/1", "capture", "in progress", None, 1);
    capture.words = Some("the rows lose their numbers".into());
    let mut signal = row("signal/page/1/1", "signal", "in progress", Some("capture/page/1"), 2);
    signal.words = Some("the rows lose their numbers".into());
    let body = rendered(vec![
        // Listed out of order: the view draws parts in the order they were made.
        row("elaboration/rows/second", "elaboration", "in progress", Some("intent/rows"), 6),
        row("intent/rows", "intent", "in progress", None, 3),
        row("elaboration/rows/first", "elaboration", "in progress", Some("intent/rows"), 4),
        row("bolt/atlas/plan-rows", "bolt", "in progress", None, 7),
        row("unit/atlas/plan-rows", "unit", "in progress", Some("bolt/atlas/plan-rows"), 8),
        row("work-item/atlas/plan-rows/wi-1", "work-item", "in progress", Some("unit/atlas/plan-rows"), 9),
        row("bolt/atlas/order-total", "bolt", "done", None, 10),
        row("proposal/atlas/1", "proposal", "waiting on the operator", None, 11),
        capture,
        signal,
        row("session/unit/atlas/plan-rows/fix/1", "session", "in progress", None, 12),
    ]);

    let forms = [
        ("intent/rows", "thread"),
        ("elaboration/rows/first", "bead"),
        ("bolt/atlas/plan-rows", "ledger"),
        ("unit/atlas/plan-rows", "slip"),
        ("work-item/atlas/plan-rows/wi-1", "item"),
        ("bolt/atlas/order-total", "record"),
        ("proposal/atlas/1", "sheet"),
        ("capture/page/1", "note"),
        ("signal/page/1/1", "quote"),
        ("session/unit/atlas/plan-rows/fix/1", "session"),
    ];
    for (object, form) in forms {
        let tag = opening(&body, object);
        assert!(tag.contains(&format!("class=\"{form}\"")), "{object} is not a {form}: {tag}");
        assert_eq!(body.matches(&format!("id=\"{object}\"")).count(), 1, "{object} is drawn once");
    }
    // A kind of the work named by 209 shares its form with no other kind.
    let named: Vec<&str> = forms.iter().map(|(_, form)| *form).collect();
    let mut distinct = named.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(distinct.len(), named.len(), "two kinds share a form: {named:?}");

    // The intent's elaborations are beads on its thread, in the order they
    // were made; the bolt's unit is a slip in its ledger and the work item a
    // line under the unit; the capture's signal is a quote on its note.
    let thread = &body[body.find("id=\"intent/rows\"").unwrap()..];
    let first = thread.find("id=\"elaboration/rows/first\"").expect("the first bead is on the thread");
    let second = thread.find("id=\"elaboration/rows/second\"").expect("the second bead is on the thread");
    assert!(first < second, "the beads are in the order they were made");
    assert!(opening(&body, "elaboration/rows/first").starts_with("<li"));
    assert!(opening(&body, "unit/atlas/plan-rows").starts_with("<li"));
    assert!(opening(&body, "work-item/atlas/plan-rows/wi-1").starts_with("<li"));
    assert!(opening(&body, "signal/page/1/1").starts_with("<li"));
    assert!(body.contains("<p class=\"words\">“the rows lose their numbers”</p>"), "a note says its words");

    // A part whose whole sits in another group is drawn where it sits.
    let body = rendered(vec![
        row("intent/rows", "intent", "done", None, 1),
        row("elaboration/rows/first", "elaboration", "in progress", Some("intent/rows"), 2),
    ]);
    assert!(opening(&body, "elaboration/rows/first").starts_with("<article class=\"bead\""));
}
