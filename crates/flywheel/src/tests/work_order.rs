//! What a work order gives a session to report with: the exact command for
//! every report it may make, from its place (65, 67, 89).

use crate::host::{how_to_report, work_order, HostStore, Reporting};
use flywheel_atoms::Records;
use flywheel_world_host::git::{self, Repo};
use flywheel_world_host::manifest::{Host, Repository};
use flywheel_world_host::{HostWorld, Manifest};
use std::path::Path;

/// A chore a curation session offered from its place on the blueprints, taken
/// on flywheel-next's shared line by another host, is handed the document as
/// its job, as the document stood at the offer's revision and not as a later
/// commit in that place left it. The taking host's clone never held the
/// place's commits, so the document is read at the pin record_offers pushed
/// (62, 89, 232; chore@2 params.job, `host.yaml` prepare_place).
#[test]
fn a_chores_order_carries_its_offered_document() {
    let dir = std::env::temp_dir().join(format!(
        "flywheel-chore-order-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let git_host = dir.join("git-host");
    let names = ["flywheel-blueprints", "flywheel-next"];
    for name in names {
        let origin = git_host.join(format!("{name}.git"));
        git::init_bare(&origin).expect("the git host's repository");
        let seed = dir.join("seed").join(name);
        git::checkout_line(&origin, &seed, "main").expect("a clone to seed from");
        git::commit_file(&Repo::at(&seed), "main", "README.md", "# first\n", "the first commit").expect("the first commit");
    }
    let repository = |name: &str| Repository {
        remote: git_host.join(format!("{name}.git")).display().to_string(),
        shared_line: "main".into(),
        template_version: None,
    };
    let manifest = Manifest {
        instance: "willdan".into(),
        blueprints: repository("flywheel-blueprints"),
        repositories: [("flywheel-next".to_string(), repository("flywheel-next"))].into(),
        hosts: ["studio", "mac-mini"]
            .into_iter()
            .map(|host| (host.to_string(), Host { root: dir.join(host), ..Default::default() }))
            .collect(),
        ..Default::default()
    };
    let joined = |host: &str| {
        let world = HostWorld::open(manifest.clone(), host).expect("the world opens");
        for name in names {
            git::clone_bare(&git_host.join(format!("{name}.git")), &world.bare(name)).expect("the bare clone");
        }
        world
    };

    // Two hosts joined, and the curation session's place on the blueprints, on
    // the host it ran on.
    let studio = joined("studio");
    let taking = joined("mac-mini");
    let place = dir.join("curation-place");
    Repo::at(studio.bare("flywheel-blueprints"))
        .git(&["worktree", "add", "--quiet", "-b", "place/curation-main", &place.to_string_lossy(), "main"])
        .expect("the curation session's place");
    let document = "flywheel/curation/chores/readme-row.md";
    let commit = |body: &str| {
        let file = place.join(document);
        std::fs::create_dir_all(file.parent().expect("a directory")).unwrap();
        std::fs::write(&file, body).unwrap();
        Repo::at(&place).git(&["add", "--", document]).expect("git add");
        Repo::at(&place).git(&["commit", "--quiet", "-m", "a chore for flywheel-next"]).expect("git commit");
    };
    commit("Add flywheel-workspace-host to the crates table in README.md.\n");

    let now = chrono::Utc::now();
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let state = flywheel_store_git::store::sandbox(&dir.join("state"), "mac-mini", now).expect("the state repository");
    let mut store = HostStore::new(state, "mac-mini", now);
    store.world = Box::new(studio);
    flywheel_domain::commands::put_new(&mut store, &defs, "curation/main", "curation", None, Default::default(), now)
        .expect("the curation");
    let session = "curation/main/1";
    let tracked = || Ok(vec!["flywheel-next".to_string()]);
    let offered = crate::report::offer(&mut store, tracked, &place, session, "curator", now, "chore", document, Some("flywheel-next"), None)
        .expect("the offer is written");
    assert_eq!(crate::report::exit_code(&offered), 0, "{offered:?}");
    let made = store
        .with_world(|store, world| flywheel_domain::offers::record(store, world, &defs, session, "curation/main", now))
        .expect("the offer is recorded and pinned");
    assert_eq!(made, vec!["unit/flywheel-next/chore-1".to_string()]);
    // The session goes on working in its place after the offer.
    commit("Something the session wrote over it later.\n");

    // The chore is taken on the other host, whose clone never held the place's
    // commits.
    let unit = store.get(&made[0]).expect("a read").expect("the chore unit");
    let revision = unit.record.get("revision").and_then(|v| v.as_str()).expect("the unit keeps the offer's revision");
    assert!(!git::holds(&Repo::at(taking.bare("flywheel-blueprints")), revision), "the taking host's clone lacks the revision");
    store.world = Box::new(taking);
    let item = "work-item/flywheel-next/chore-1/wi-1";
    flywheel_domain::commands::put_new(&mut store, &defs, item, "work-item", Some(&made[0]), Default::default(), now)
        .expect("the chore's work item");
    let order = work_order(&defs, &store, &format!("{item}/fix/1"), &format!("{item}#own"), item).expect("the order renders");
    let job = order.body.split("## the job").nth(1).and_then(|rest| rest.split("\nobject:").next()).unwrap_or_default();
    assert!(
        job.contains("Add flywheel-workspace-host to the crates table in README.md."),
        "the order's job is the offered document: {}",
        order.body
    );
    assert!(!order.body.contains("wrote over it later"), "the order reads the document at the offer's revision: {}", order.body);
}

/// A finding a build session offered on its own bolt is a proposed fast unit,
/// and that unit's session is handed the document as its job as a chore's is:
/// as the document stood at the offer's revision and not as a later commit in
/// the offering place left it, read on a host whose clone never held the
/// place's commits (62, 232; `host.yaml` prepare_place, `unit.yaml` revision).
#[test]
fn a_findings_fast_unit_order_carries_its_offered_document() {
    let dir = std::env::temp_dir().join(format!(
        "flywheel-finding-order-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let git_host = dir.join("git-host");
    let names = ["flywheel-blueprints", "flywheel-next"];
    for name in names {
        let origin = git_host.join(format!("{name}.git"));
        git::init_bare(&origin).expect("the git host's repository");
        let seed = dir.join("seed").join(name);
        git::checkout_line(&origin, &seed, "main").expect("a clone to seed from");
        git::commit_file(&Repo::at(&seed), "main", "README.md", "# first\n", "the first commit").expect("the first commit");
    }
    let repository = |name: &str| Repository {
        remote: git_host.join(format!("{name}.git")).display().to_string(),
        shared_line: "main".into(),
        template_version: None,
    };
    let manifest = Manifest {
        instance: "willdan".into(),
        blueprints: repository("flywheel-blueprints"),
        repositories: [("flywheel-next".to_string(), repository("flywheel-next"))].into(),
        hosts: ["studio", "mac-mini"]
            .into_iter()
            .map(|host| (host.to_string(), Host { root: dir.join(host), ..Default::default() }))
            .collect(),
        ..Default::default()
    };
    let joined = |host: &str| {
        let world = HostWorld::open(manifest.clone(), host).expect("the world opens");
        for name in names {
            git::clone_bare(&git_host.join(format!("{name}.git")), &world.bare(name)).expect("the bare clone");
        }
        world
    };

    // Two hosts joined, and the build session's place on flywheel-next, on the
    // host it ran on.
    let studio = joined("studio");
    let taking = joined("mac-mini");
    let place = dir.join("build-place");
    Repo::at(studio.bare("flywheel-next"))
        .git(&["worktree", "add", "--quiet", "-b", "place/rows-writer", &place.to_string_lossy(), "main"])
        .expect("the build session's place");
    let document = "docs/findings/row-order.md";
    let commit = |body: &str| {
        let file = place.join(document);
        std::fs::create_dir_all(file.parent().expect("a directory")).unwrap();
        std::fs::write(&file, body).unwrap();
        Repo::at(&place).git(&["add", "--", document]).expect("git add");
        Repo::at(&place).git(&["commit", "--quiet", "-m", "a finding on the bolt"]).expect("git commit");
    };
    commit("Rows sort by their ids as they count, not as strings.\n");

    let now = chrono::Utc::now();
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let state = flywheel_store_git::store::sandbox(&dir.join("state"), "mac-mini", now).expect("the state repository");
    let mut store = HostStore::new(state, "mac-mini", now);
    store.world = Box::new(studio);
    let on_flywheel_next = || [("repository".to_string(), serde_json::json!("flywheel-next"))].into_iter().collect();
    let bolt = "bolt/flywheel-next/rows";
    let writer = "unit/flywheel-next/rows/writer";
    let building = "work-item/flywheel-next/rows/writer/wi-1";
    flywheel_domain::commands::put_new(&mut store, &defs, bolt, "bolt", None, on_flywheel_next(), now).expect("the bolt");
    flywheel_domain::commands::put_new(&mut store, &defs, writer, "unit", Some(bolt), on_flywheel_next(), now).expect("the unit");
    flywheel_domain::commands::put_new(&mut store, &defs, building, "work-item", Some(writer), Default::default(), now)
        .expect("the work item");
    let session = format!("{building}/build/1");
    let tracked = || Ok(vec!["flywheel-next".to_string()]);
    let offered = crate::report::offer(&mut store, tracked, &place, &session, "builder", now, "finding", document, None, None)
        .expect("the offer is written");
    assert_eq!(crate::report::exit_code(&offered), 0, "{offered:?}");
    let made = store
        .with_world(|store, world| flywheel_domain::offers::record(store, world, &defs, &session, building, now))
        .expect("the offer is recorded and pinned");
    assert_eq!(made, vec!["unit/flywheel-next/rows/finding-1".to_string()]);
    let unit = store.get(&made[0]).expect("a read").expect("the fast unit");
    assert_eq!(unit.record.get("type").and_then(|v| v.as_str()), Some("fast"), "a finding on the bolt is a fast unit");
    // The session goes on working in its place after the offer.
    commit("Something the session wrote over it later.\n");

    // The fast unit is taken on the other host, whose clone never held the
    // place's commits.
    let revision = unit.record.get("revision").and_then(|v| v.as_str()).expect("the unit keeps the offer's revision");
    assert!(!git::holds(&Repo::at(taking.bare("flywheel-next")), revision), "the taking host's clone lacks the revision");
    store.world = Box::new(taking);
    let item = "work-item/flywheel-next/rows/finding-1/wi-1";
    flywheel_domain::commands::put_new(&mut store, &defs, item, "work-item", Some(&made[0]), Default::default(), now)
        .expect("the fast unit's work item");
    let order = work_order(&defs, &store, &format!("{item}/build/1"), &format!("{item}#own"), item).expect("the order renders");
    let job = order.body.split("## the job").nth(1).and_then(|rest| rest.split("\nobject:").next()).unwrap_or_default();
    assert!(
        job.contains("Rows sort by their ids as they count, not as strings."),
        "the order's job is the offered document: {}",
        order.body
    );
    assert!(!order.body.contains("wrote over it later"), "the order reads the document at the offer's revision: {}", order.body);
}

fn reporting<'a>(session: &'a str, deliverables: &'a [String]) -> Reporting<'a> {
    Reporting {
        session,
        state: "/hosts/laptop/scratch/flywheel-state/main",
        manifest: Some(Path::new("/flywheel/flywheel.yaml")),
        flywheel: "/bin/flywheel",
        host: "laptop",
        deliverables,
    }
}

/// Every session may offer a finding, a chore or a signal outside its job, so
/// every order gives the offer's command naming the three and when each is
/// used, with the manifest a chore's scope is checked against; the ask's
/// command is a curation session's and not a chore's (58,
/// 60, 62, 116, `sessions.yaml` commands.offer, commands.ask).
#[test]
fn every_order_gives_the_offer_command_and_curations_the_ask() {
    let named = vec!["commits".to_string(), "verdict".to_string()];
    let chore = "work-item/flywheel-next/chore-1/wi-1/fix/1";
    let curation = "curation/scratch/main/1";
    for session in [chore, curation] {
        let said = how_to_report(&reporting(session, &named));
        // The session, the state repository and the manifest are the command's
        // own arguments: a line the program's shell parser can read whole is
        // one it runs without asking a person (67, 72).
        assert!(
            said.contains(&format!(
                "/bin/flywheel exit done --deliverable commits --deliverable verdict \
                 --session {session} --state /hosts/laptop/scratch/flywheel-state/main --host laptop"
            )),
            "{said}"
        );
        assert!(!said.contains("FLYWHEEL_SESSION="), "the order still writes an environment prefix: {said}");
        let offer = said
            .lines()
            .find(|line| line.contains("/bin/flywheel offer "))
            .unwrap_or_else(|| panic!("the order gives no offer command: {said}"));
        assert_eq!(
            offer.trim(),
            format!(
                "/bin/flywheel offer finding|chore|signal --document <path> --about <object> \
                 [--scope bolt-line|<repository>] --session {session} \
                 --state /hosts/laptop/scratch/flywheel-state/main \
                 --manifest /flywheel/flywheel.yaml --host laptop"
            ),
        );
        assert!(
            said.contains("a signal when what you saw is about neither your intent nor your bolt"),
            "the order says when each offer is used: {said}"
        );
    }
    assert!(!how_to_report(&reporting(chore, &named)).contains(" ask "), "a chore's order gives the ask");
    assert!(
        how_to_report(&reporting(curation, &named))
            .contains("/bin/flywheel ask <repository> \"<the words>\" --session curation/scratch/main/1"),
        "curation's order gives no ask"
    );
}
