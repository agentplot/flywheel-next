//! The acts of 42 over real repositories: a temp git host, a host's bare
//! clone and checkout as a join makes them, and a bolt's line and places
//! under a temp root. The repositories are the subject, so this is the unit
//! tier (D17).

use super::*;
use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::Object;
use flywheel_world_host::git;
use serde_json::json;
use std::path::PathBuf;

/// A scratch directory of this test's own, emptied first.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("flywheel-workspace-host-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

/// A git host with one repository on it that has a first commit, and a host
/// that joined it: the bare clone and the checkout, as `join` leaves them.
fn a_host_with(repository: &str, name: &str) -> (PathBuf, PathBuf) {
    let dir = scratch(name);
    let origin = dir.join("git-host").join(format!("{repository}.git"));
    git::init_bare(&origin).expect("the git host's repository");
    // The first commit, from a scratch clone.
    let seed = dir.join("seed");
    git::checkout_line(&origin, &seed, "main").expect("a clone to seed from");
    git::commit_file(&Repo::at(&seed), "main", "README.md", "# the shop\n", "the first commit")
        .expect("the first commit lands");
    // What a join makes (205).
    let root = dir.join("root").join("storefront");
    git::clone_bare(&origin, &root.join(format!("{repository}.git"))).expect("the bare clone");
    git::checkout_line(&root.join(format!("{repository}.git")), &root.join(repository), "main")
        .expect("the checkout");
    (dir, root)
}

fn object(id: &str, machine: &str, parent: Option<&str>, record: &[(&str, serde_json::Value)]) -> Object {
    Object {
        id: id.into(),
        machine: machine.into(),
        parent: parent.map(String::from),
        config: Default::default(),
        entered_at: Default::default(),
        record: record.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    }
}

/// A bolt with one unit and one work item under it, as `propose-unit` and
/// `create_items` leave them.
fn a_bolt(store: &mut FakeStore) {
    store.seed(object("bolt/storefront/plan-rows", "bolt", None, &[("repository", json!("storefront")), ("name", json!("plan-rows"))]));
    store.seed(object("unit/storefront/plan-rows", "unit", Some("bolt/storefront/plan-rows"), &[("repository", json!("storefront")), ("type", json!("chore"))]));
    store.seed(object("work-item/storefront/plan-rows/wi-1", "work-item", Some("unit/storefront/plan-rows"), &[("ordinal", json!(1))]));
}

fn commit_in(dir: &std::path::Path, path: &str, body: &str, message: &str) {
    let full = dir.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(&full, body).unwrap();
    let tree = Repo::at(dir);
    tree.git(&["add", "--", path]).unwrap();
    tree.git(&["commit", "--quiet", "-m", message]).unwrap();
}

fn log(dir: &std::path::Path, what: &str) -> String {
    Repo::at(dir).git(&["log", "--format=%s", what]).unwrap_or_default()
}

#[test]
fn a_line_is_a_branch_with_a_worktree_and_a_place_is_a_worktree_off_it() {
    let (_dir, root) = a_host_with("storefront", "line-and-place");
    let mut store = FakeStore::default();
    a_bolt(&mut store);
    let mut ws = HostWorkspace::new(&mut store, &root);

    ws.create_line("bolt/storefront/plan-rows", "").expect("the line");
    let line = line_dir(&root, "storefront", "bolt/storefront/plan-rows");
    assert!(line.join("README.md").is_file(), "the line's worktree is at the shared line's head");
    assert!(ws.branch_exists("storefront", "bolt/storefront/plan-rows").unwrap());

    ws.prepare_place("work-item/storefront/plan-rows/wi-1#own", "work-item/storefront/plan-rows/wi-1", "# the job\n")
        .expect("the place");
    let place = place_dir(&root, "storefront", "work-item/storefront/plan-rows/wi-1#own", "bolt/storefront/plan-rows");
    assert!(place.join("README.md").is_file(), "the place is a worktree off the line");
    assert_eq!(std::fs::read_to_string(place.join(".flywheel/work-order.md")).unwrap(), "# the job\n");
    let status = Repo::at(&place).git(&["status", "--porcelain"]).unwrap();
    assert!(status.trim().is_empty(), "the work order is untracked and excluded (203): {status}");

    // The facts the machines read are the recorded binding's, written alike.
    let evidence = |name: &str| flywheel_workspace_recorded::evidence(ws.store, "work-item/storefront/plan-rows/wi-1#own", name);
    assert_eq!(evidence("place.exists"), Some(json!(true)));
    assert_eq!(evidence("place.contains_line"), Some(json!(true)));
    assert_eq!(
        flywheel_workspace_recorded::evidence(ws.store, "bolt/storefront/plan-rows", "line.exists"),
        Some(json!(true))
    );

    // Twice is once (73).
    ws.create_line("bolt/storefront/plan-rows", "").expect("a repeat changes nothing");
    ws.prepare_place("work-item/storefront/plan-rows/wi-1#own", "", "# the job\n").expect("a repeat changes nothing");
}

#[test]
fn a_places_commits_merge_into_the_line_and_the_line_lands_on_the_git_host() {
    let (dir, root) = a_host_with("storefront", "merge-and-land");
    let mut store = FakeStore::default();
    a_bolt(&mut store);
    let mut ws = HostWorkspace::new(&mut store, &root);
    ws.create_line("bolt/storefront/plan-rows", "").unwrap();
    ws.prepare_place("work-item/storefront/plan-rows/wi-1#own", "", "").unwrap();
    let place = place_dir(&root, "storefront", "work-item/storefront/plan-rows/wi-1#own", "bolt/storefront/plan-rows");

    // The session's work: one commit in the place (67).
    commit_in(&place, "src/rows.rs", "fn rows() {}\n", "feat(rows): number the rows");

    assert_eq!(ws.merge_place("work-item/storefront/plan-rows/wi-1#own").unwrap(), TakeOutcome::Done);
    let line = line_dir(&root, "storefront", "bolt/storefront/plan-rows");
    assert!(line.join("src/rows.rs").is_file(), "the line holds the place's work");
    assert_eq!(
        flywheel_workspace_recorded::evidence(ws.store, "work-item/storefront/plan-rows/wi-1#own", "place.merged"),
        Some(json!(true))
    );
    ws.remove_place("work-item/storefront/plan-rows/wi-1#own").unwrap();
    assert!(!place.exists(), "the worktree is gone");
    assert!(!ws.branch_exists("storefront", &place_branch("work-item/storefront/plan-rows/wi-1#own")).unwrap());

    // The landing: take, acceptance, land (50, 192, 175).
    assert_eq!(ws.take_parent("bolt/storefront/plan-rows").unwrap(), TakeOutcome::Done);
    ws.write_acceptance("bolt/storefront/plan-rows", "accepted\n").unwrap();
    ws.land_line("bolt/storefront/plan-rows", LandingPolicy::Direct).unwrap();

    let origin = dir.join("git-host").join("storefront.git");
    let landed = log(&origin, "main");
    assert!(landed.contains("number the rows"), "the git host's main carries the work: {landed}");
    assert!(landed.contains("acceptance"), "and the acceptance commit (192): {landed}");
    assert!(root.join("storefront").join("src/rows.rs").is_file(), "the machinery's checkout followed");
    assert_eq!(
        flywheel_workspace_recorded::evidence(ws.store, "bolt/storefront/plan-rows", "line.landed"),
        Some(json!(true))
    );

    ws.remove_line("bolt/storefront/plan-rows").unwrap();
    assert!(!line.exists());
    assert!(!ws.branch_exists("storefront", "bolt/storefront/plan-rows").unwrap());
}

#[test]
fn a_merge_that_conflicts_is_aborted_whole_and_says_so() {
    let (_dir, root) = a_host_with("storefront", "conflict");
    let mut store = FakeStore::default();
    a_bolt(&mut store);
    let mut ws = HostWorkspace::new(&mut store, &root);
    ws.create_line("bolt/storefront/plan-rows", "").unwrap();
    ws.prepare_place("work-item/storefront/plan-rows/wi-1#own", "", "").unwrap();
    let line = line_dir(&root, "storefront", "bolt/storefront/plan-rows");
    let place = place_dir(&root, "storefront", "work-item/storefront/plan-rows/wi-1#own", "bolt/storefront/plan-rows");

    commit_in(&line, "README.md", "# the shop, by the line\n", "the line moved");
    commit_in(&place, "README.md", "# the shop, by the place\n", "the place moved");

    let outcome = ws.merge_place("work-item/storefront/plan-rows/wi-1#own").unwrap();
    assert!(matches!(outcome, TakeOutcome::Conflicted { .. }), "{outcome:?}");
    let status = Repo::at(&line).git(&["status", "--porcelain"]).unwrap();
    assert!(status.trim().is_empty(), "aborted whole (179): {status}");
    assert_eq!(
        flywheel_workspace_recorded::evidence(ws.store, "work-item/storefront/plan-rows/wi-1#own", "place.conflicted"),
        Some(json!(true))
    );
    assert_eq!(
        flywheel_workspace_recorded::evidence(ws.store, "work-item/storefront/plan-rows/wi-1#own", "place.merged"),
        Some(json!(false))
    );
}

#[test]
fn the_bolts_own_place_is_the_lines_worktree() {
    let (_dir, root) = a_host_with("storefront", "own-place");
    let mut store = FakeStore::default();
    a_bolt(&mut store);
    let mut ws = HostWorkspace::new(&mut store, &root);
    ws.create_line("bolt/storefront/plan-rows", "").unwrap();
    ws.prepare_place("bolt/storefront/plan-rows#own", "", "# the operator's place\n").unwrap();
    let line = line_dir(&root, "storefront", "bolt/storefront/plan-rows");
    assert!(line.join(".flywheel/work-order.md").is_file(), "the own place is the line's worktree (44)");
    assert_eq!(ws.merge_place("bolt/storefront/plan-rows#own").unwrap(), TakeOutcome::Done);
    ws.remove_place("bolt/storefront/plan-rows#own").unwrap();
    assert!(line.join(".git").exists(), "removing the own place leaves the line standing");
}

#[test]
fn the_repository_and_the_line_are_found_from_the_object_and_its_parents() {
    let mut store = FakeStore::default();
    a_bolt(&mut store);
    store.seed(object("intent/declines", "intent", None, &[]));
    store.seed(object("elaboration/declines/1", "elaboration", Some("intent/declines"), &[]));
    let ws = HostWorkspace::new(&mut store, "/nowhere");
    assert_eq!(ws.repository_of("work-item/storefront/plan-rows/wi-1").unwrap(), "storefront");
    assert_eq!(ws.line_of("work-item/storefront/plan-rows/wi-1#own").unwrap(), "bolt/storefront/plan-rows");
    assert!(!is_the_lines_own("work-item/storefront/plan-rows/wi-1#own", "bolt/storefront/plan-rows"));
    assert!(is_the_lines_own("bolt/storefront/plan-rows#own", "bolt/storefront/plan-rows"));
    assert_eq!(ws.line_of("bolt/storefront/plan-rows#own").unwrap(), "bolt/storefront/plan-rows");
    assert_eq!(ws.repository_of("elaboration/declines/1").unwrap(), BLUEPRINTS);
    assert_eq!(ws.line_of("elaboration/declines/1").unwrap(), "intent/declines");
    assert!(ws.repository_of("unit/nowhere/x").is_err());
}
