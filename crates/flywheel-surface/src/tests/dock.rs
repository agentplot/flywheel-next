//! The dock's page per kind (S28): what a developer wants to know about the
//! object opened — its words, its branch, its sessions, its commits — and no
//! key-value dump of the record (S214, S222).

use crate::testing as world;
use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::{CommitRef, Records};
use flywheel_domain::commands;
use flywheel_engine::{Definitions, Object};
use serde_json::{json, Value};
use std::collections::BTreeMap;

const ADDRESS: &str = "http://studio.tailnet.ts.net/willdan";
const BOLT: &str = "bolt/atlas/rows-lose-numbers";
const UNIT: &str = "unit/atlas/rows-lose-numbers";
const ITEM: &str = "work-item/atlas/rows-lose-numbers/wi-1";
const SESSION: &str = "work-item/atlas/rows-lose-numbers/wi-1/fix/1";
const JOB: &str = "the rows lose their numbers on the second page";

fn record(fields: &[(&str, Value)]) -> BTreeMap<String, Value> {
    fields.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

/// A fact a binding records: a place, a session (42, 72).
fn fact(store: &mut FakeStore, id: &str, fields: &[(&str, Value)]) {
    let object = Object {
        id: id.into(),
        machine: "fact".into(),
        parent: None,
        config: Default::default(),
        entered_at: Default::default(),
        record: record(fields),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    };
    store.put(id, &object, 0).expect("the fact");
}

/// A bolt with one chore unit, its work item in its fix stage, the places of
/// the bolt and the item, and the item's session done with its commits.
fn a_bolt_worked() -> (FakeStore, world::Files, Definitions) {
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let mut store = FakeStore::default();
    let at = commands::now(&store).expect("a point");
    commands::put_new(&mut store, &defs, BOLT, "bolt", None, record(&[("repository", json!("atlas"))]), at).expect("the bolt");
    let unit = record(&[("repository", json!("atlas")), ("type", json!("chore")), ("subject", json!(JOB))]);
    commands::put_new(&mut store, &defs, UNIT, "unit", Some(BOLT), unit, at).expect("the unit");
    commands::put_new(&mut store, &defs, ITEM, "work-item", Some(UNIT), record(&[("type", json!("chore"))]), at).expect("the item");
    let mut item = Records::get(&store, ITEM).expect("a read").expect("the item");
    item.config.insert("life".into(), "in-type".into());
    item.config.insert("life.in-type.stages".into(), "fix".into());
    let base = item.seq;
    Records::put(&mut store, ITEM, &item, base).expect("the item in its stage");

    fact(
        &mut store,
        &format!("fact/place/{BOLT}#own"),
        &[("dir", json!("/hosts/laptop/willdan/places/atlas/rows-lose-numbers")), ("branch", json!(BOLT)), ("exists", json!(true))],
    );
    fact(
        &mut store,
        &format!("fact/place/{ITEM}#own"),
        &[
            ("dir", json!("/hosts/laptop/willdan/places/atlas/rows-lose-numbers-wi-1")),
            ("branch", json!("bolt/atlas/rows-lose-numbers/wi-1")),
            ("exists", json!(false)),
        ],
    );
    fact(
        &mut store,
        &format!("fact/session/{SESSION}"),
        &[
            ("host", json!("laptop")),
            ("herdr_agent", json!("rows-wi-1-fix-1")),
            ("herdr_pane", json!("w1-p2")),
            ("started_at", json!("2026-09-14T12:00:00Z")),
            ("ended_at", Value::Null),
        ],
    );
    flywheel_domain::report::write_report(
        &mut store,
        SESSION,
        "rows-wi-1-fix-1",
        at,
        &flywheel_domain::report::Report::Exit {
            kind: "done".into(),
            deliverables: vec!["commits".into()],
            question: None,
            text: None,
        },
    )
    .expect("the session reports");
    commands::rail(&mut store, &defs).expect("the rail derives");
    (store, world::Files::new(), defs)
}

fn commit() -> CommitRef {
    CommitRef {
        hash: "d63b6f7".into(),
        subject: "fix: the rows keep their numbers".into(),
        author: "session".into(),
        at: "2026-09-14T12:30:00Z".into(),
    }
}

/// The dock's page for one object, as the page renders it.
fn page_of(html: &str, id: &str) -> String {
    let at = html
        .find(&format!("id=\"dock-{id}\""))
        .unwrap_or_else(|| panic!("no dock page for {id}"));
    let rest = &html[at..];
    rest[..rest.find("</article>").expect("the page is closed")].to_string()
}

fn rendered(store: &mut FakeStore, world: &world::Files, defs: &Definitions, landed: bool) -> String {
    let mut read = crate::page::read(store, world, defs, ADDRESS, "chuck").expect("the page reads");
    read.commits.insert(BOLT.into(), vec![commit()]);
    if landed {
        read.commits_are_mains.insert(BOLT.into());
    }
    crate::page::render(&read)
}

/// The page's body names no field of the record and no word of the model's
/// where the page says branch (S214, S222).
fn assert_no_dump(page: &str) {
    let body = page.split("<div class=\"dk-b\">").nth(1).expect("the page has a body");
    for said in ["held by", "config", "entered_at", "applied_responses", " line "] {
        assert!(!body.contains(said), "the page says `{said}`: {body}");
    }
}

/// A bolt's page is its repository, branch and place, its units, every session
/// with its agent, pane, host, start, exit and delivery, and the commits on the
/// branch — titled as main's latest once it has landed (S28, S222).
#[test]
fn a_bolts_page_is_its_branch_units_sessions_and_commits() {
    let (mut store, world, defs) = a_bolt_worked();
    let html = rendered(&mut store, &world, &defs, false);
    let page = page_of(&html, BOLT);
    for shown in [
        "<h3>the branch</h3>",
        "<span class=\"mono\">atlas</span>",
        &format!("<span class=\"mono\">{BOLT}</span>"),
        "<span class=\"mono\">/hosts/laptop/willdan/places/atlas/rows-lose-numbers</span>",
        "<h3>units</h3>",
        &format!("href=\"#dock-{UNIT}\""),
        "<h3>sessions</h3>",
        "agent <span class=\"mono\">rows-wi-1-fix-1</span>",
        "pane <span class=\"mono\">w1-p2</span>",
        "host <span class=\"mono\">laptop</span>",
        "started 2026-09-14 12:00",
        "<b>done</b>",
        "delivered commits",
        "<h3>commits on the branch</h3>",
        "d63b6f7",
        "fix: the rows keep their numbers",
    ] {
        assert!(page.contains(shown), "the bolt's page lacks `{shown}`: {page}");
    }
    assert_no_dump(&page);

    let landed = page_of(&rendered(&mut store, &world, &defs, true), BOLT);
    assert!(landed.contains("<h3>landed · main"), "a landed bolt's commits are main's latest: {landed}");
    assert!(!landed.contains("commits on the branch"), "{landed}");
}

/// A unit's page is its job as a quote, its type, bolt and repository, its work
/// items with their stage, its sessions and the commits on its bolt (S28).
#[test]
fn a_units_page_is_its_job_items_sessions_and_commits() {
    let (mut store, world, defs) = a_bolt_worked();
    let page = page_of(&rendered(&mut store, &world, &defs, false), UNIT);
    for shown in [
        &format!("<blockquote class=\"dtext said\">{JOB}</blockquote>"),
        "<span class=\"st\">type</span><span class=\"grow\">chore</span>",
        &format!("href=\"#dock-{BOLT}\""),
        "<h3>work items</h3>",
        &format!("href=\"#dock-{ITEM}\""),
        "stage fix",
        "<b>done</b>",
        "fix: the rows keep their numbers",
    ] {
        assert!(page.contains(shown), "the unit's page lacks `{shown}`: {page}");
    }
    assert_no_dump(&page);
}

/// A work item's page is its unit's job, its stage, its place and branch —
/// said to be removed once it is — and its session (S28, S222).
#[test]
fn a_work_items_page_is_its_job_stage_place_and_session() {
    let (mut store, world, defs) = a_bolt_worked();
    let page = page_of(&rendered(&mut store, &world, &defs, false), ITEM);
    for shown in [
        &format!("<blockquote class=\"dtext said\">{JOB}</blockquote>"),
        "<span class=\"st\">stage</span><span class=\"grow\">fix</span>",
        "rows-lose-numbers-wi-1</span> · removed",
        "<span class=\"mono\">bolt/atlas/rows-lose-numbers/wi-1</span>",
        "agent <span class=\"mono\">rows-wi-1-fix-1</span>",
    ] {
        assert!(page.contains(shown), "the item's page lacks `{shown}`: {page}");
    }
    assert_no_dump(&page);
}

/// A capture's page is its words as a quote from their beginning, where it came
/// from and who typed it, and what became of it once its signal has moved
/// (S215, S223).
#[test]
fn a_captures_page_is_its_words_its_source_and_what_became_of_it() {
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let (mut store, mut world) = (FakeStore::default(), world::Files::new());
    let call = crate::catalogue::Call::new("capture", "chuck", "page")
        .arg("text", json!(JOB))
        .arg("source", json!("console"));
    crate::catalogue::call(&mut store, &mut world, &defs, &call).expect("the capture is taken");
    let read = crate::page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("the page reads");
    let id_of = |machine: &str| read.objects.iter().find(|o| o.machine == machine).expect("taken").id.clone();
    let (capture, signal) = (id_of("capture"), id_of("signal"));

    let page = page_of(&crate::page::render(&read), &capture);
    assert!(page.contains(&format!("<blockquote class=\"dtext said\">{JOB}</blockquote>")), "{page}");
    assert!(page.contains("from the console · by chuck"), "{page}");
    assert!(!page.contains("what became of it"), "nothing has become of it yet: {page}");

    let mut moved = Records::get(&store, &signal).expect("a read").expect("the signal");
    moved.config.insert("move".into(), "dropped".into());
    let base = moved.seq;
    Records::put(&mut store, &signal, &moved, base).expect("the signal dropped");
    let read = crate::page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("the page reads");
    let page = page_of(&crate::page::render(&read), &capture);
    assert!(page.contains("<h3>what became of it</h3>"), "{page}");
    assert!(page.contains("<div class=\"tail\">dropped</div>"), "{page}");
}

/// Any other object's page is what it holds, in the order it was made, and
/// what it is part of (210, S28).
#[test]
fn any_other_page_is_what_it_holds_and_what_it_is_part_of() {
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let (mut store, world) = (FakeStore::default(), world::Files::new());
    let at = commands::now(&store).expect("a point");
    commands::put_new(&mut store, &defs, "instance/willdan", "instance", None, Default::default(), at).expect("the instance");
    commands::put_new(&mut store, &defs, "intent/atlas-rows", "intent", Some("instance/willdan"), Default::default(), at)
        .expect("the intent");
    let research = record(&[("type", json!("research"))]);
    commands::put_new(&mut store, &defs, "elaboration/atlas-rows/research", "elaboration", Some("intent/atlas-rows"), research, at)
        .expect("the elaboration");
    commands::rail(&mut store, &defs).expect("the rail derives");
    let read = crate::page::read(&mut store, &world, &defs, ADDRESS, "chuck").expect("the page reads");

    let page = page_of(&crate::page::render(&read), "intent/atlas-rows");
    assert!(page.contains("<h3>elaborations</h3><ol class=\"elaborations\">"), "{page}");
    assert!(page.contains("href=\"#dock-elaboration/atlas-rows/research\""), "{page}");
    assert!(page.contains("research · "), "each elaboration says its type: {page}");
    assert!(page.contains("<h3>part of</h3>"), "{page}");
    assert!(page.contains("href=\"#dock-instance/willdan\""), "{page}");
    assert_no_dump(&page);
}
