//! `flywheel init`: the instance machine, driven repeatably (204, 207, 82).

use flywheel::init::{self, Init};
use std::path::PathBuf;

/// The operator's key, as this sandbox hands it over. Nothing is written into
/// the process's own environment: the manifest names where the operator put it
/// and the machinery only asks whether it is there, so a test says so directly
/// (207, 207a). The environment is one table for the whole process, and two
/// threads writing and reading it at once is a race whatever the names are.
struct Sandbox {
    dir: PathBuf,
    key_from: String,
    placed: Option<String>,
}

impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let dir = std::env::temp_dir().join(format!(
            "flywheel-init-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // A name of its own, so one test's placed key is not another's.
        Sandbox {
            dir,
            key_from: format!("FLYWHEEL_TEST_KEY_{}", name.to_uppercase()),
            placed: None,
        }
    }

    fn ask(&self) -> Init {
        Init {
            instance: "willdan".into(),
            host: "mac-mini".into(),
            root: self.dir.join("root"),
            git_host: self.dir.join("git-host"),
            app: "12345".into(),
            app_key_from: self.key_from.clone(),
            app_key: self.placed.clone(),
            address: "http://laptop.example".into(),
            manifest: self.dir.join("flywheel.yaml"),
            curation: None,
        }
    }

    fn place_the_key(&mut self) {
        self.placed = Some("the operator placed this".into());
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// The App's installation is a decision under attention until it is seen, and
/// no agent makes it true (204, 207, 207a, 82).
#[test]
fn init_awaits_app_install() {
    let sandbox = Sandbox::new("await");
    let report = init::run(sandbox.ask()).expect("init runs");

    assert_eq!(report.state, "awaiting-app");
    assert_eq!(
        report.attention.as_deref(),
        Some("app-install"),
        "the decision stands: {report:?}"
    );
    assert_eq!(
        report.performed,
        vec!["create_blueprints", "create_state", "register_app"],
        "everything an agent may do is done, and the installation is not one of them"
    );
    assert!(
        !report.performed.iter().any(|e| e == "register_host"),
        "nothing past the App's installation is taken"
    );

    // The manifest names where the key is, and never the key.
    let manifest = std::fs::read_to_string(sandbox.dir.join("flywheel.yaml")).unwrap();
    assert!(manifest.contains(&sandbox.key_from));
    assert!(!manifest.contains("the operator placed this"));
}

/// Every step is an effect with a proof, so running it again changes nothing
/// (204).
#[test]
fn init_twice_writes_nothing() {
    let mut sandbox = Sandbox::new("twice");
    sandbox.place_the_key();
    let first = init::run(sandbox.ask()).expect("init runs");
    assert_eq!(first.state, "hosted", "{first:?}");
    let manifest = std::fs::read_to_string(sandbox.dir.join("flywheel.yaml")).unwrap();

    let again = init::run(sandbox.ask()).expect("init runs again");
    assert!(
        again.performed.is_empty(),
        "a second run performs nothing: {again:?}"
    );
    assert_eq!(again.state, "hosted");
    assert_eq!(
        manifest,
        std::fs::read_to_string(sandbox.dir.join("flywheel.yaml")).unwrap(),
        "and writes nothing"
    );
}

/// A half-finished bootstrap is finished by the next run: the guard reads the
/// world, not a flag the machinery set (204).
#[test]
fn init_resumes_half_finished() {
    let mut sandbox = Sandbox::new("resume");

    // Stopped where 207 stops it: the key is not placed yet.
    let stopped = init::run(sandbox.ask()).expect("init runs");
    assert_eq!(stopped.state, "awaiting-app");

    // The operator places it, and the next run carries on from there rather
    // than starting again.
    sandbox.place_the_key();
    let resumed = init::run(sandbox.ask()).expect("init resumes");
    assert_eq!(resumed.state, "hosted");
    assert_eq!(
        resumed.performed,
        vec!["register_host"],
        "only what was left: {resumed:?}"
    );
    assert!(resumed.attention.is_none());
}

/// The machinery writes under its own prefix in a tracked repository. A write
/// outside it that no response asked for is refused and reported, never made
/// (203).
#[test]
fn prefix_write_refused() {
    use flywheel_world_host::prefix;

    // Its own prefix, in any tracked repository.
    prefix::check("atlas", "flywheel/declarations.yaml", None).expect("under the prefix");
    prefix::check("flywheel-blueprints", "flywheel/unit-types/spike@1.yaml", None)
        .expect("under the prefix");
    // The state repository is the machinery's alone and has no prefix rule.
    prefix::check("flywheel-state", "objects/lamp/1/object.rec", None).expect("its own");

    let refused = prefix::check("atlas", "README.md", None).expect_err("outside the prefix");
    let said = format!("{refused}");
    assert!(said.contains("README.md"), "the refusal names the path: {said}");
    assert!(said.contains("flywheel/"), "and the prefix: {said}");
    assert!(said.contains("203"), "and the rule: {said}");

    // The one thing it writes outside its prefix is what a response asked for:
    // the change directory an intent opens (23, 49, 203).
    prefix::check(
        "flywheel-blueprints",
        "openspec/changes/loop-granularity/proposal.md",
        Some("chat/1001"),
    )
    .expect("the effect of a response");
    assert!(prefix::check("atlas", "README.md", Some("")).is_err());
}

/// A second flywheel runs beside the first against the same instance and shares
/// nothing with it: its own state repository, its own root, its own prefix
/// (96, D14).
#[test]
fn coexistence_scope_is_disjoint() {
    let mut first = Sandbox::new("coexist-a");
    let mut second = Sandbox::new("coexist-b");
    first.place_the_key();
    second.place_the_key();

    let mut a = first.ask();
    a.instance = "willdan".into();
    init::run(a).expect("the first flywheel");
    let mut b = second.ask();
    b.instance = "willdan-next".into();
    init::run(b).expect("the second, beside it");

    let manifest_a =
        flywheel_world_host::Manifest::read(&first.dir.join("flywheel.yaml")).unwrap();
    let manifest_b =
        flywheel_world_host::Manifest::read(&second.dir.join("flywheel.yaml")).unwrap();
    let scope_a = init::scope_of(&manifest_a, "mac-mini").unwrap();
    let scope_b = init::scope_of(&manifest_b, "mac-mini").unwrap();

    assert!(scope_a.disjoint_from(&scope_b), "{scope_a:?} {scope_b:?}");
    assert!(!scope_a.covers(&scope_b.root), "neither reads into the other");
    assert!(!scope_b.covers(&scope_a.root));
    assert_ne!(scope_a.state, scope_b.state, "its own state repository");
    // Everything either wrote is inside its own scope and nowhere else.
    for path in walk(&first.dir.join("root")) {
        assert!(scope_a.covers(&path), "{} is outside the first's scope", path.display());
        assert!(!scope_b.covers(&path));
    }
}

/// An instance is removed by a response, never by deleting files: the state is
/// archived with its counter, the git repositories stay on disk, and no number
/// is ever reused (221, 15, 4).
#[test]
fn remove_instance_keeps_counter() {
    let mut sandbox = Sandbox::new("remove");
    sandbox.place_the_key();
    let ask = sandbox.ask();
    init::run(sandbox.ask()).expect("init");

    let removed = init::remove(&ask).expect("removed by the operator's response");
    // The state is the state repository, which stays on disk with every other
    // repository; the archive says where it is, so nothing of it is thrown away
    // (221).
    let archived = std::fs::read_to_string(removed.archive.join("state"))
        .expect("the state is archived");
    assert!(
        archived.contains("willdan-state.git"),
        "the archive names the state repository: {archived}"
    );
    assert!(
        std::fs::read_to_string(removed.archive.join("counter"))
            .unwrap()
            .contains(&removed.counter.to_string()),
        "with its decision counter, so no number is reused (15)"
    );
    assert_eq!(
        removed.repositories_left,
        vec!["flywheel-state", "flywheel-blueprints"],
        "every git repository is left where it was (221)"
    );
    for name in &removed.repositories_left {
        let remote = sandbox.dir.join("git-host").join(format!("willdan-{}.git",
            name.trim_start_matches("flywheel-")));
        assert!(remote.exists(), "{} is still on disk", remote.display());
    }
}

fn walk(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else { continue };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out
}

/// A repository record in the manifest holds git details alone. A kind, a
/// capability or a scope in one is refused before anything is read from it:
/// what a repository holds is the map's and the repository's own declarations'
/// to say (199).
#[test]
fn repository_record_has_git_details_only() {
    let mut sandbox = Sandbox::new("records");
    sandbox.place_the_key();
    init::run(sandbox.ask()).expect("init");

    let path = sandbox.dir.join("flywheel.yaml");
    let good = std::fs::read_to_string(&path).unwrap();
    flywheel_world_host::Manifest::check_repository_records(&good).expect("git details alone");

    std::fs::write(
        &path,
        good.replace("repositories: {}", "repositories:\n  atlas:\n    remote: /a.git\n    kind: book\n"),
    )
    .unwrap();
    let refused = init::run(sandbox.ask()).expect_err("a kind in a repository record is refused");
    let said = format!("{refused}");
    assert!(said.contains("git details alone"), "{said}");
    assert!(said.contains("199"), "the refusal names the rule: {said}");
}

/// The set version is stamped at initialization and at repository creation, and
/// a newer set upgrades nothing on its own (208).
#[test]
fn set_version_stamped() {
    let mut sandbox = Sandbox::new("stamped");
    sandbox.place_the_key();
    let report = init::run(sandbox.ask()).expect("init");
    assert!(report.lines.iter().any(|l| l.contains("set ")), "{report:?}");

    let manifest = flywheel_world_host::Manifest::read(&sandbox.dir.join("flywheel.yaml")).unwrap();
    let set = flywheel_domain::set::SET_VERSION;
    assert_eq!(manifest.template_version.as_deref(), Some(set));
    assert_eq!(manifest.blueprints.template_version.as_deref(), Some(set));
    assert_eq!(manifest.state.template_version.as_deref(), Some(set));

    // Running again against the same set changes no stamp, and a newer set
    // would upgrade nothing without a chore the operator accepts (208).
    init::run(sandbox.ask()).expect("again");
    let again = flywheel_world_host::Manifest::read(&sandbox.dir.join("flywheel.yaml")).unwrap();
    assert_eq!(again.template_version, manifest.template_version);
    assert_eq!(again.blueprints.template_version, manifest.blueprints.template_version);
}

/// What `flywheel.yaml` says curation is charged on reaches the curation
/// record, where the evidence reads it — at `init`, and again when a host
/// declares, so changing the manifest takes effect rather than being read once
/// and forgotten (110, 118, `blueprints.yaml` evidence.curation.threshold).
#[test]
fn the_manifests_curation_threshold_reaches_the_record() {
    let mut sandbox = Sandbox::new("threshold");
    sandbox.place_the_key();
    // The manifest the operator writes says four, not the shipped dozen.
    let mut ask = sandbox.ask();
    ask.curation = Some(flywheel_world_host::manifest::Curation {
        threshold: 4,
        cadence: "weekly".into(),
    });
    let report = init::run(ask).expect("init runs");
    assert_eq!(report.state, "hosted", "{report:?}");

    let threshold_of = |sandbox: &Sandbox| -> (u64, String) {
        let mut host = flywheel::host::Host::open(
            &sandbox.dir.join("flywheel.yaml"),
            "mac-mini",
            None,
            chrono::Utc::now(),
        )
        .expect("the host opens");
        host.store.git.fetch().expect("the shared line");
        let held = flywheel_atoms::Records::get(&host.store.git, "curation/willdan")
            .expect("a read")
            .expect("the instance's curation record");
        (
            held.record.get("threshold").and_then(|v| v.as_u64()).expect("a threshold"),
            held.record
                .get("cadence")
                .and_then(|v| v.as_str())
                .expect("a cadence")
                .to_string(),
        )
    };
    assert_eq!(threshold_of(&sandbox), (4, "weekly".to_string()));

    // And the manifest edited afterwards is read: a setting nothing reads
    // after the first run is not a setting.
    let path = sandbox.dir.join("flywheel.yaml");
    let mut manifest = flywheel_world_host::Manifest::read(&path).expect("the manifest");
    manifest.curation.threshold = 9;
    manifest.write(&path).expect("the manifest is written");
    let mut host = flywheel::host::Host::open(&path, "mac-mini", None, chrono::Utc::now())
        .expect("the host opens");
    host.declare().expect("the host declares");
    assert_eq!(threshold_of(&sandbox).0, 9, "the edited setting was not read");
}
