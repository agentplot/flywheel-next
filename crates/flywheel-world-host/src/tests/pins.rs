//! An offer's pin on the git host, as a host binds it: pushed from the clone a
//! place shares, read on a host whose clone never held the revision, and
//! removed from the git host and every clone that fetched it (55, 62, 232).
//! The repositories are the subject, so this is the unit tier (D17).

use crate::git::{self, Repo};
use crate::manifest::{Host, Repository};
use crate::{HostWorld, Manifest};
use flywheel_atoms::World;
use std::path::{Path, PathBuf};

const PINS: &str = "refs/flywheel/offers/";

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "flywheel-pins-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn commit_in(place: &Path, path: &str, body: &str) -> String {
    let file = place.join(path);
    std::fs::create_dir_all(file.parent().expect("a directory")).unwrap();
    std::fs::write(&file, body).unwrap();
    let tree = Repo::at(place);
    tree.git(&["add", "--", path]).expect("git add");
    tree.git(&["commit", "--quiet", "-m", "the session's document"]).expect("git commit");
    tree.git(&["rev-parse", "HEAD"]).expect("the head").trim().to_string()
}

#[test]
fn a_pin_is_read_on_a_host_whose_clone_lacks_it_and_removed_from_both() {
    let dir = scratch("read-and-remove");
    let origin = dir.join("git-host").join("atlas.git");
    git::init_bare(&origin).expect("the git host's repository");
    let seed = dir.join("seed");
    git::checkout_line(&origin, &seed, "main").expect("a clone to seed from");
    git::commit_file(&Repo::at(&seed), "main", "README.md", "# atlas\n", "the first commit").expect("the first commit");
    let manifest = Manifest {
        instance: "willdan".into(),
        repositories: [(
            "atlas".to_string(),
            Repository { remote: origin.display().to_string(), shared_line: "main".into(), template_version: None },
        )]
        .into(),
        hosts: ["studio", "mac-mini"]
            .into_iter()
            .map(|host| (host.to_string(), Host { root: dir.join(host), ..Default::default() }))
            .collect(),
        ..Default::default()
    };
    let joined = |host: &str| {
        let world = HostWorld::open(manifest.clone(), host).expect("the world opens");
        git::clone_bare(&origin, &world.bare("atlas")).expect("the bare clone a join makes");
        world
    };

    // Two hosts joined the fleet, and a session's place on the one it ran on, a
    // worktree of that host's clone.
    let mut studio = joined("studio");
    let mut mac_mini = joined("mac-mini");
    let place = dir.join("place");
    Repo::at(studio.bare("atlas"))
        .git(&["worktree", "add", "--quiet", "-b", "place/curation-main", &place.to_string_lossy(), "main"])
        .expect("the place");
    let path = "notes/limits.md";
    let revision = commit_in(&place, path, "as offered\n");
    let reference = format!("{PINS}curation/main/1/0");
    assert!(studio.pin("atlas", &reference, &revision).unwrap(), "the first pin pushes");
    assert!(!studio.pin("atlas", &reference, &revision).unwrap(), "a pin already held pushes nothing");
    commit_in(&place, path, "written over later\n");

    // The other host, whose clone never held the place's commits.
    assert!(!git::holds(&Repo::at(mac_mini.bare("atlas")), &revision), "the place's commits reached no other clone");
    let read = mac_mini.read_pinned("atlas", &reference, &revision, path).expect("the pin is fetched and read");
    assert_eq!(read.as_deref(), Some("as offered\n".as_bytes()), "the document as it stood at the pinned revision");
    assert_eq!(mac_mini.pins("atlas", PINS).unwrap(), vec![(reference.clone(), revision.clone())]);

    studio.unpin("atlas", &reference, &revision).expect("the pin is removed");
    assert!(
        Repo::at(&origin).git(&["for-each-ref", &reference]).unwrap().trim().is_empty(),
        "the git host no longer holds the pin"
    );
    assert!(studio.pins("atlas", PINS).unwrap().is_empty());
    mac_mini.unpin("atlas", &reference, &revision).expect("a pin another host removed leaves this clone too");
    assert!(mac_mini.pins("atlas", PINS).unwrap().is_empty());
}
