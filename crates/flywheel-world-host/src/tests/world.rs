//! `World` as a host binds it, over a sandbox git host with no network
//! (D1, D8, 204–208).

use flywheel_atoms::World;
use crate::bootstrap::{self, Bootstrap};
use crate::manifest::Manifest;
use crate::{join, HostWorld};
use std::path::PathBuf;

struct Sandbox {
    dir: PathBuf,
}

impl Sandbox {
    fn new(name: &str) -> Sandbox {
        let dir = std::env::temp_dir().join(format!(
            "flywheel-world-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Sandbox { dir }
    }
    fn git_host(&self) -> PathBuf {
        self.dir.join("git-host")
    }
    fn bootstrap(&self) -> Bootstrap {
        Bootstrap::new(self.git_host(), self.dir.join("scratch"))
    }
    /// An initialized instance with one host, at the address the operator gave
    /// for it: a host has one address and it is never a localhost port
    /// (205a, D10a).
    fn initialized(&self, host: &str) -> Manifest {
        self.initialized_at(host, "http://mac-mini.tailnet")
    }

    fn initialized_at(&self, host: &str, address: &str) -> Manifest {
        let root = self.dir.join(host);
        bootstrap::init(
            &self.bootstrap(),
            "willdan",
            host,
            &root,
            "12345",
            "FLYWHEEL_TEST_APP_KEY",
            address,
            None,
        )
        .expect("init")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A repository read, a manifest read and a router lookup: the three things the
/// world answers (D8).
#[test]
fn a_repository_a_manifest_and_a_route() {
    let sandbox = Sandbox::new("three");
    let mut manifest = sandbox.initialized("mac-mini");
    manifest.router.base = "http://mac-mini.tailnet".into();
    let mut world = HostWorld::open(manifest, "mac-mini").unwrap();
    world.clone_repositories().unwrap();

    // The manifest, as the machinery reads it.
    let read = world.manifest().unwrap();
    assert_eq!(read["instance"], "willdan");
    assert_eq!(read["app"]["id"], "12345");

    // A repository read: the blueprints template landed on the shared line and
    // is readable with no checkout of its own.
    let readme = world
        .read_file("flywheel-blueprints", "flywheel/README.md")
        .unwrap()
        .expect("the template's file is on the shared line");
    assert!(String::from_utf8_lossy(&readme).contains("flywheel"));
    assert!(world
        .read_file("flywheel-blueprints", "nothing/here.md")
        .unwrap()
        .is_none());

    // A router lookup: one address, the host's private-network name, with the
    // instance in the path (205a, D10a).
    let endpoint = world.route("rail").unwrap();
    assert_eq!(endpoint.url, "http://mac-mini.tailnet/willdan/rail");
    assert!(!endpoint.url.contains("localhost"));
}

/// A localhost base is refused: a link to one opens nothing on a phone (205a,
/// D10a).
#[test]
fn a_localhost_address_is_refused() {
    let sandbox = Sandbox::new("localhost");
    let manifest = sandbox.initialized_at("mac-mini", "http://localhost");
    let world = HostWorld::open(manifest, "mac-mini").unwrap();
    let refused = world.route("rail").expect_err("localhost is no host address");
    assert!(format!("{refused}").contains("205a"));
}

/// Creating what exists changes nothing: a second init adopts and adds only
/// what the template requires and is missing (204, 220).
#[test]
fn adopt_is_idempotent() {
    let sandbox = Sandbox::new("adopt");
    let bootstrap = sandbox.bootstrap();
    let first = bootstrap.create_blueprints("willdan").unwrap();
    let blueprints = sandbox.git_host().join("willdan-blueprints.git");
    let head = crate::git::Repo::at(&blueprints)
        .git(&["rev-parse", "main"])
        .unwrap();

    let again = bootstrap.create_blueprints("willdan").unwrap();
    assert_eq!(first, again, "the same repository, at the same version");
    assert_eq!(
        head,
        crate::git::Repo::at(&blueprints)
            .git(&["rev-parse", "main"])
            .unwrap(),
        "a second create wrote nothing"
    );

    // A repository an operator already had keeps what it holds, and gains only
    // what the template requires (220).
    let checkout = sandbox.dir.join("by-hand");
    crate::git::checkout_line(&blueprints, &checkout, "main").unwrap();
    let repo = crate::git::Repo::at(&checkout);
    crate::git::commit_file(
        &repo,
        "main",
        "theirs/notes.md",
        "the operator's own\n",
        "the operator's own file",
    )
    .unwrap();
    std::fs::remove_file(checkout.join("context-map/current.yaml")).unwrap();
    repo.git(&["commit", "--quiet", "-am", "and one of the template's removed"])
        .unwrap();
    repo.git(&["push", "--quiet", "origin", "main"]).unwrap();

    bootstrap.create_blueprints("willdan").unwrap();
    let bare = crate::git::Repo::at(&blueprints);
    assert!(
        crate::git::show(&bare, "main", "theirs/notes.md")
            .unwrap()
            .is_some(),
        "adoption rewrites nothing of theirs"
    );
    assert!(
        crate::git::show(&bare, "main", "context-map/current.yaml")
            .unwrap()
            .is_some(),
        "and adds what the template requires and is missing"
    );
}

/// The state repository is created with the profile's layout, empty of objects
/// (204, C.2).
#[test]
fn create_state_repository() {
    let sandbox = Sandbox::new("state");
    let repository = sandbox.bootstrap().create_state("willdan").unwrap();
    assert_eq!(repository.shared_line, "main");
    assert_eq!(
        repository.template_version.as_deref(),
        Some(flywheel_domain::set::SET_VERSION),
        "the set version is stamped at creation (208)"
    );
    let bare = crate::git::Repo::at(sandbox.git_host().join("willdan-state.git"));
    let paths = crate::git::ls_tree(&bare, "main", "").unwrap();
    for wanted in ["objects/.keep", "responses/.keep", "asks/.keep", "runs/.keep"] {
        assert!(paths.iter().any(|p| p == wanted), "{wanted} in {paths:?}");
    }
    assert!(
        !paths.iter().any(|p| p.starts_with("objects/") && !p.ends_with(".keep")),
        "empty of objects: {paths:?}"
    );
}

/// A join clones what is missing and nothing else, and makes no place (205,
/// 93a).
#[test]
fn join_clones_only_what_is_missing() {
    let sandbox = Sandbox::new("join");
    let manifest = sandbox.initialized("mac-mini");
    let mut world = HostWorld::open(manifest, "mac-mini").unwrap();

    let first = join::join(&mut world).unwrap();
    assert_eq!(first.cloned, vec!["flywheel-state", "flywheel-blueprints"]);
    assert_eq!(first.checked_out, first.cloned);

    let again = join::join(&mut world).unwrap();
    assert!(again.cloned.is_empty(), "a repeat clones nothing: {again:?}");
    assert!(again.checked_out.is_empty());

    // Bare clones under one root, one checkout of each shared line, and no
    // worktree: places are made later (52).
    for name in ["flywheel-state", "flywheel-blueprints"] {
        assert!(crate::git::Repo::at(world.bare(name)).is_bare());
        assert!(world.checkout(name).join(".git").is_dir());
    }
    assert!(join::doctor(&world).is_empty(), "{:?}", join::doctor(&world));
}

/// A hand-made layout is refused, and the refusal says what differs (205, 222).
#[test]
fn doctor_names_first_difference() {
    let sandbox = Sandbox::new("doctor");
    let manifest = sandbox.initialized("mac-mini");
    let mut world = HostWorld::open(manifest, "mac-mini").unwrap();

    let before = join::doctor(&world);
    assert!(
        before.iter().any(|d| d.contains("does not exist")),
        "a host with no root is told so: {before:?}"
    );

    join::join(&mut world).unwrap();
    assert!(join::doctor(&world).is_empty());

    // Someone made the checkout by hand and left no bare clone.
    std::fs::remove_dir_all(world.bare("flywheel-blueprints")).unwrap();
    let said = join::doctor(&world);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].contains("flywheel-blueprints"), "{said:?}");
    assert!(said[0].contains("no bare clone"), "{said:?}");
    assert!(said[0].contains("205"), "the refusal names the rule: {said:?}");
}

/// The App's key is read from where the operator placed it, and neither the key
/// nor the token it makes is written into configuration, code or the state
/// repository (207, 207a).
#[test]
fn token_written_nowhere_else() {
    let sandbox = Sandbox::new("token");
    let manifest = sandbox.initialized("mac-mini");
    let rendered = manifest.render().unwrap();
    let world = HostWorld::open(manifest, "mac-mini").unwrap();

    // With nothing placed there is no token, and the refusal says whose job it
    // is to place one.
    std::env::remove_var("FLYWHEEL_TEST_APP_KEY");
    let refused = world.app_token().expect_err("no key, no token");
    assert!(format!("{refused}").contains("207a"), "{refused}");

    std::env::set_var("FLYWHEEL_TEST_APP_KEY", "a-private-key-the-operator-placed");
    let token = world.app_token().unwrap();
    assert!(token.starts_with("installation:12345:"));
    assert!(
        !token.contains("a-private-key"),
        "the token does not carry the key"
    );

    // The manifest names where the key is, never the key.
    assert!(rendered.contains("FLYWHEEL_TEST_APP_KEY"));
    assert!(!rendered.contains("a-private-key"));
    // Nothing of either reached the repositories the host keeps.
    for (name, _) in world.manifest.all_repositories() {
        let bare = crate::git::Repo::at(world.bare(&name));
        if !bare.exists() {
            continue;
        }
        for path in crate::git::ls_tree(&bare, "main", "").unwrap() {
            let text =
                crate::git::show(&bare, "main", &path).unwrap().unwrap_or_default();
            assert!(!text.contains("a-private-key"), "{path} holds the key");
            assert!(!text.contains(&token), "{path} holds the token");
        }
    }
    std::env::remove_var("FLYWHEEL_TEST_APP_KEY");
}

/// A manifest repository the App's installation does not cover is a decision
/// under attention, and nothing is taken on it until the installation is
/// extended (207).
#[test]
fn uncovered_repository_decision() {
    let sandbox = Sandbox::new("covered");
    let mut manifest = sandbox.initialized("mac-mini");
    manifest.repositories.insert(
        "atlas".into(),
        sandbox.bootstrap().create_repository("willdan", "atlas").unwrap(),
    );

    let uncovered = Bootstrap::uncovered_repositories(&manifest);
    assert!(uncovered.contains(&"atlas".to_string()), "{uncovered:?}");
    assert_eq!(uncovered.len(), 3, "nothing is covered until it is: {uncovered:?}");

    manifest.app.installation_covers = vec![
        "flywheel-state".into(),
        "flywheel-blueprints".into(),
        "atlas".into(),
    ];
    assert!(Bootstrap::uncovered_repositories(&manifest).is_empty());
}

/// A repository record holds git details alone: a kind, a capability or a scope
/// in one is refused, because what a repository holds is read from the map and
/// the repository's own declarations (199).
#[test]
fn repository_record_has_git_details_only() {
    let good = "instance: willdan\nblueprints: {remote: /a.git}\nstate: {remote: /b.git}\n\
                repositories:\n  atlas: {remote: /c.git, shared_line: main}\n";
    Manifest::check_repository_records(good).expect("git details alone");
    let manifest = Manifest::parse(good).unwrap();
    assert_eq!(manifest.repositories["atlas"].shared_line, "main");

    for bad in [
        "repositories:\n  atlas: {remote: /c.git, kind: book}\n",
        "repositories:\n  atlas: {remote: /c.git, capability: publishing}\n",
        "repositories:\n  atlas: {remote: /c.git, scope: [docs]}\n",
    ] {
        let refused = Manifest::check_repository_records(bad).expect_err("refused");
        assert!(format!("{refused}").contains("199"), "{refused}");
    }
}

/// Initialization and repository creation stamp the version of the set they
/// used, and a newer set upgrades nothing on its own (208).
#[test]
fn set_version_stamped() {
    let sandbox = Sandbox::new("stamp");
    let manifest = sandbox.initialized("mac-mini");
    let stamped = flywheel_domain::set::SET_VERSION;
    assert_eq!(manifest.template_version.as_deref(), Some(stamped));
    assert_eq!(manifest.blueprints.template_version.as_deref(), Some(stamped));
    assert_eq!(manifest.state.template_version.as_deref(), Some(stamped));

    // A newer set exists; nothing moves until a chore is accepted (208).
    let mut newer = Bootstrap::new(sandbox.git_host(), sandbox.dir.join("scratch"));
    newer.set_version = "9.9.9".into();
    let again = bootstrap::init(
        &newer,
        "willdan",
        "mac-mini",
        &sandbox.dir.join("mac-mini"),
        "12345",
        "FLYWHEEL_TEST_APP_KEY",
        "http://mac-mini.example",
        Some(manifest.clone()),
    )
    .unwrap();
    assert_eq!(
        again.template_version.as_deref(),
        Some(stamped),
        "the stamp is unchanged and nothing is upgraded"
    );
    assert_eq!(again.blueprints.template_version.as_deref(), Some(stamped));
}

/// A host's address is the private-network name its router gives it, with the
/// instance in the path — the manifest names a router per host, and the
/// instance's own stands for a host that names none (191, 205a, 308, D10a).
#[test]
fn address_is_the_routers_name() {
    let text = r#"
instance: willdan
router: {base: "http://studio.tailnet.ts.net"}
blueprints: {remote: "git@example:willdan/blueprints.git"}
state: {remote: "git@example:willdan/state.git"}
hosts:
  studio:
    root: /tmp/flywheel
  mac-mini:
    root: /tmp/flywheel
    router: {base: "http://mac-mini.tailnet.ts.net/"}
    localhost_port: 5150
"#;
    let manifest = Manifest::parse(text).expect("the manifest parses");
    let world = HostWorld::open(manifest, "studio").expect("the world opens");

    // The host that names its own router is reached at that name.
    assert_eq!(
        world.address_of("mac-mini").expect("an address"),
        "http://mac-mini.tailnet.ts.net/willdan"
    );
    // The host that names none is reached at the instance's.
    assert_eq!(
        world.address_of("studio").expect("an address"),
        "http://studio.tailnet.ts.net/willdan"
    );
    // And `route` gives an object's endpoint at the acting host's address.
    assert_eq!(
        world.route("unit/atlas/u").expect("an endpoint").url,
        "http://studio.tailnet.ts.net/willdan/unit/atlas/u"
    );

    // The localhost port is what the operator at the machine uses, kept beside
    // the address and never part of it (245).
    assert_eq!(world.localhost_port_of("mac-mini"), 5150);
    assert_eq!(world.localhost_port_of("studio"), 4242);
    assert!(!world.address_of("studio").unwrap().contains("localhost"));
}

/// A router base that is the machine's own address is refused: a link written
/// at it opens nothing on a phone, which is what 306 asks of every decision
/// (205a, 308, D10a).
#[test]
fn a_localhost_router_is_refused() {
    let text = r#"
instance: willdan
router: {base: "http://localhost:4242"}
blueprints: {remote: "git@example:willdan/blueprints.git"}
state: {remote: "git@example:willdan/state.git"}
hosts:
  studio: {root: /tmp/flywheel}
"#;
    let manifest = Manifest::parse(text).expect("the manifest parses");
    let world = HostWorld::open(manifest, "studio").expect("the world opens");
    let refused = world.address_of("studio").expect_err("refused");
    assert!(
        format!("{refused}").contains("private-network name"),
        "{refused}"
    );
    assert!(world.route("unit/atlas/u").is_err());
}
