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
            address: Some("http://laptop.example".into()),
            manifest: self.dir.join("flywheel.yaml"),
            repositories: vec![],
            curation: None,
            at: chrono::Utc::now(),
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

/// The router base the manifest holds for the sandbox's host.
fn base_of(sandbox: &Sandbox) -> Option<String> {
    let manifest = flywheel_world_host::Manifest::read(&sandbox.dir.join("flywheel.yaml")).unwrap();
    manifest.hosts.get("mac-mini")?.router.as_ref().map(|r| r.base.clone())
}

/// Given no hostname, the first host is registered at localhost at its page's
/// port and init says it serves this computer alone; no name is derived from
/// the computer or its network. The same init given the name later moves the
/// host there (204, 205a, 306, D10a).
#[test]
fn init_with_no_host_registers_localhost_and_says_so() {
    let mut sandbox = Sandbox::new("localhost");
    sandbox.place_the_key();
    let mut ask = sandbox.ask();
    ask.address = None;
    let report = init::run(ask).expect("init runs");
    assert_eq!(report.state, "hosted", "{report:?}");
    assert_eq!(base_of(&sandbox).as_deref(), Some("http://localhost:4242"));
    assert!(
        report
            .lines
            .iter()
            .any(|l| l.contains("http://localhost:4242") && l.contains("serves this computer alone")),
        "init says the host serves this computer alone: {report:?}"
    );

    let mut named = sandbox.ask();
    named.address = Some("mac-mini.tailnet.example".into());
    let report = init::run(named).expect("init runs again with the name");
    assert_eq!(base_of(&sandbox).as_deref(), Some("http://mac-mini.tailnet.example:4242"));
    assert!(
        !report.lines.iter().any(|l| l.contains("serves this computer alone")),
        "a host given its name is not said to serve this computer alone: {report:?}"
    );
}

/// Given a hostname, the first host is registered at it and the port its page
/// is served on, and a link written at that address names the instance; a run
/// given no name afterwards takes nothing back (204, 205a, 308).
#[test]
fn init_registers_the_hostname_given() {
    let mut sandbox = Sandbox::new("hostname");
    sandbox.place_the_key();
    let mut ask = sandbox.ask();
    ask.address = Some("mac-mini.tailnet.example".into());
    let report = init::run(ask).expect("init runs");
    assert_eq!(report.state, "hosted", "{report:?}");
    assert_eq!(base_of(&sandbox).as_deref(), Some("http://mac-mini.tailnet.example:4242"));
    assert!(
        report.lines.iter().any(|l| l == "host mac-mini at http://mac-mini.tailnet.example:4242"),
        "{report:?}"
    );

    let mut unnamed = sandbox.ask();
    unnamed.address = None;
    init::run(unnamed).expect("init runs again with no name");
    assert_eq!(base_of(&sandbox).as_deref(), Some("http://mac-mini.tailnet.example:4242"));

    let world = flywheel_world_host::HostWorld::open(
        flywheel_world_host::Manifest::read(&sandbox.dir.join("flywheel.yaml")).unwrap(),
        "mac-mini",
    );
    assert_eq!(
        world.map(|w| w.address_of("mac-mini").unwrap()).ok().as_deref(),
        Some("http://mac-mini.tailnet.example:4242/willdan")
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

/// A bolt whose close is offered: one decision, standing, for a sink to carry
/// (22, 39).
fn a_decision(host: &mut flywheel::host::Host, id: &str) {
    let now = host.now();
    let object = flywheel_engine::Object {
        id: id.to_string(),
        machine: "bolt".into(),
        parent: None,
        config: [("life", "open"), ("life.open.close", "offered")]
            .iter()
            .map(|(region, state)| (region.to_string(), state.to_string()))
            .collect(),
        entered_at: [("life".to_string(), now), ("life.open.close".to_string(), now)]
            .into_iter()
            .collect(),
        record: Default::default(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    };
    host.store.git.seed_objects(&[object]).expect("the bolt");
    let defs = host.defs.clone();
    flywheel_domain::commands::rail(&mut host.store, &defs).expect("the rail numbers it");
}

/// The App's key and the token it makes, and the chat's bot token, are written
/// nowhere a host writes: not the manifest, not any tracked line of the state
/// repository — the run record under `runs/**` and the committed `status.html`
/// included — and not the body of the page the host serves, after ticks that
/// heartbeat, deliver to Discord and fail to (204, 207, 207a; audit 16, D9).
///
/// The sweep is over what a real `Host` wrote, so the entries it covers are
/// asserted present first: a sweep over an empty tree proves nothing. On the
/// way, a chat sink whose token is not placed is under attention and the host
/// runs on (217f).
#[test]
fn token_written_nowhere_else() {
    use flywheel_surface::chat::Channel;
    use flywheel_surface::testing::discord::{Double, CHANNEL};
    use flywheel_world_host::git::{ls_tree, show, Repo};

    let mut sandbox = Sandbox::new("nowhere");
    sandbox.place_the_key();
    let report = init::run(sandbox.ask()).expect("init runs");
    assert_eq!(report.state, "hosted", "{report:?}");
    let path = sandbox.dir.join("flywheel.yaml");

    // The chat the operator named: a Discord channel whose bot's token they
    // placed in a variable of their own, presented by this host (D9, 148).
    let chat_from = "FLYWHEEL_TEST_DISCORD_TOKEN_NOWHERE";
    let chat_token = "MTAwMDAwMDAwMDAwMDAwMDAwMA.GhYjKl.a-bot-token-the-operator-placed";
    let mut named = flywheel_world_host::Manifest::read(&path).unwrap();
    named.sinks.insert(
        "chat".into(),
        flywheel_world_host::manifest::Sink {
            kind: "chat".into(),
            channel: "discord".into(),
            surface: CHANNEL.to_string(),
            member: None,
            routes: vec!["all".into()],
            token_from: chat_from.into(),
        },
    );
    named.hosts.get_mut("mac-mini").unwrap().presents = vec!["chat".into()];
    named.write(&path).unwrap();

    // The key is where the manifest says the operator put it, and the token the
    // world mints from it is what the sweep looks for.
    let key = "a-private-key-the-operator-placed";
    std::env::set_var(&sandbox.key_from, key);
    let now = chrono::Utc::now();
    let mut host = flywheel::host::Host::open(&path, "mac-mini", None, now).expect("the host opens");
    let token = host.store.world.app_token().expect("a key placed makes a token");
    assert!(token.starts_with("installation:12345:"), "{token}");
    assert!(!token.contains(key), "the token does not carry the key");

    // The chat's token not placed yet: the sink is under attention with what to
    // do, and the host runs on (217f).
    let double = Double::start();
    let api = double.address.clone();
    let unplaced = host
        .present_sinks(&named, |sink, entry| {
            let discord = flywheel::host::discord_for(sink, entry, &|_| None, Some(&api), None)?;
            Ok(Box::new(discord) as Box<dyn Channel + Send>)
        })
        .expect("the host presents what it can");
    assert_eq!(unplaced.len(), 1, "{unplaced:?}");
    assert!(
        unplaced[0].starts_with("under attention") && unplaced[0].contains(chat_from),
        "{unplaced:?}"
    );

    assert!(double.seen().is_empty(), "a sink with no token spoke to Discord");

    // Placed: a decision is delivered through the channel with the token, by a
    // tick that heartbeats — it declares, decides, rewrites the status view and
    // appends its run record, and pushes the lot at the shared line...
    let placed = host
        .present_sinks(&named, |sink, entry| {
            let discord = flywheel::host::discord_for(
                sink,
                entry,
                &|variable| (variable == chat_from).then(|| chat_token.to_string()),
                Some(&api),
                Some("chuck"),
            )?;
            Ok(Box::new(discord) as Box<dyn Channel + Send>)
        })
        .expect("the host presents the sink");
    assert_eq!(placed, ["presents chat on discord"]);
    a_decision(&mut host, "bolt/atlas/plan-rows");
    host.sweep().expect("a sweep delivers");
    let posted = double.seen();
    assert_eq!(posted.len(), 1, "the decision was not delivered: {posted:#?}");
    assert_eq!(posted[0].authorization.as_deref(), Some(format!("Bot {chat_token}").as_str()));
    assert!(
        host.attention().unwrap().iter().any(|a| a == "refusal: sink/chat"),
        "the token not placed at first was not under attention: {:?}",
        host.attention()
    );

    // ...and one Discord refuses is reported with its reason (81, 127).
    double.refuse();
    host.set_now(host.now() + chrono::Duration::minutes(2));
    a_decision(&mut host, "bolt/atlas/drop-the-tail");
    host.sweep().expect("a failed delivery does not stop the host");
    let failed = host
        .store
        .git
        .run_record()
        .unwrap()
        .into_iter()
        .filter(|e| e.reason == "deliver_rail")
        .flat_map(|e| e.fields)
        .find(|(field, _)| field == "failed")
        .map(|(_, why)| why)
        .expect("the refused delivery is in the run record");
    assert!(failed.contains("401") && failed.contains(chat_from), "{failed}");
    let page = {
        let flywheel::host::HostStore { git, world, .. } = &mut host.store;
        let read = flywheel_surface::page::read(git, &**world, &host.defs, &host.sinks.address, "chuck")
            .expect("the page reads");
        flywheel_surface::page::render(&read)
    };
    // A member's own client, holding a credential of its own, calls the
    // catalogue at the host's address: a view, an answer, and a call the host
    // refuses, which its run record carries. No credential of a client is
    // written anywhere either (320, 321, 79, 204, 207).
    let client_token = "mcp-client-credential-a-member-holds";
    let number = {
        let defs = host.defs.clone();
        flywheel_domain::commands::rail(&mut host.store, &defs)
            .expect("the rail")
            .iter()
            .find_map(|d| d.number)
            .expect("a decision stands")
    };
    let host = std::sync::Arc::new(std::sync::Mutex::new(host));
    let served = flywheel::serve::page_of(&host, 4242, &["chuck".to_string()]);
    let instance = served.instance().to_string();
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let address = runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = flywheel_surface::http::serve_on(served, listener).await;
        });
        address
    });
    for params in [
        serde_json::json!({"name": "rail"}),
        serde_json::json!({"name": "answer", "arguments": {"decision": number, "answer": "yes"}}),
        serde_json::json!({"name": "ask", "arguments": {"repository": "nowhere", "text": "keep the rows"}}),
    ] {
        use std::io::{Read, Write};
        let body = serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": params}).to_string();
        let mut socket = std::net::TcpStream::connect(address).expect("the host answers");
        socket
            .write_all(
                format!(
                    "POST /{instance} HTTP/1.1\r\nHost: 127.0.0.1:4242\r\nAuthorization: Bearer {client_token}\r\n\
                     Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .expect("the call");
        let mut reply = String::new();
        socket.read_to_string(&mut reply).expect("the reply");
        assert!(reply.starts_with("HTTP/1.1 200"), "the client's call was not served: {reply}");
    }
    let page_after = {
        let mut guard = host.lock().expect("the host");
        let held = &mut *guard;
        let later = held.now() + chrono::Duration::minutes(2);
        held.set_now(later);
        held.sweep().expect("a sweep after the client's calls");
        let flywheel::host::HostStore { git, world, .. } = &mut held.store;
        let read = flywheel_surface::page::read(git, &**world, &held.defs, &held.sinks.address, "chuck")
            .expect("the page reads");
        flywheel_surface::page::render(&read)
    };
    std::env::remove_var(&sandbox.key_from);

    // The manifest names where the key and the chat's token are, never either.
    let manifest = std::fs::read_to_string(&path).unwrap();
    assert!(manifest.contains(&sandbox.key_from));
    assert!(!manifest.contains(key), "the manifest holds the key");
    assert!(!manifest.contains(&token), "the manifest holds the token");
    assert!(manifest.contains(chat_from));
    assert!(!manifest.contains(chat_token), "the manifest holds the chat's token");
    assert!(!manifest.contains(client_token), "the manifest holds a client's credential");

    // Every tracked file on the state repository's shared line, the run record
    // and the status view among them.
    let read = flywheel_world_host::Manifest::read(&path).unwrap();
    let state = Repo::at(&read.state.remote);
    let paths = ls_tree(&state, "main", "").expect("the shared line lists");
    assert!(
        paths.iter().any(|p| p.starts_with("runs/") && p.ends_with(".rec")),
        "the tick wrote no run record to sweep: {paths:?}"
    );
    assert!(paths.iter().any(|p| p == "status.html"), "the tick wrote no status view to sweep: {paths:?}");
    let mut delivered_on_record = false;
    let mut client_refused_on_record = false;
    for path in &paths {
        let text = show(&state, "main", path).unwrap().unwrap_or_default();
        assert!(!text.contains(key), "{path} holds the key");
        assert!(!text.contains(&token), "{path} holds the token");
        assert!(!text.contains(chat_token), "{path} holds the chat's token");
        assert!(!text.contains(client_token), "{path} holds a client's credential");
        delivered_on_record |= path.starts_with("runs/") && text.contains("401");
        client_refused_on_record |= path.starts_with("runs/") && text.contains("nowhere");
    }
    assert!(delivered_on_record, "the run record on the shared line does not carry the refusal");
    assert!(client_refused_on_record, "the run record does not carry the client's refused call");

    // And the served page's body, rendered from the same state.
    assert!(page.contains("mac-mini"), "the page is rendered from the host's state");
    assert!(!page.contains(key), "the page holds the key");
    assert!(!page.contains(&token), "the page holds the token");
    assert!(!page.contains(chat_token), "the page holds the chat's token");
    assert!(page_after.contains("mac-mini"), "the page after the client's calls is rendered from the host's state");
    assert!(!page_after.contains(client_token), "the page holds a client's credential");
    drop(runtime);
}

/// A bootstrap stamps the instance at the moment it happens at, and not minutes
/// past it (231, D15).
///
/// The instance machine is settled on the scenario runner's `Runtime`, whose
/// tick advances a minute a pass — the virtual clock that keeps a test from
/// sleeping. A real bootstrap ran on it and stamped the instance up to twelve
/// minutes into the future, and every age a host then judges by — a lease's
/// renewal, a stall window, a cadence's last run, "seen at" — is measured
/// against a clock that has not got there yet.
#[test]
fn a_bootstrap_stamps_no_moment_ahead_of_its_own() {
    let mut sandbox = Sandbox::new("clock");
    sandbox.place_the_key();
    let at = chrono::Utc::now();
    let mut ask = sandbox.ask();
    ask.at = at;
    let manifest = ask.manifest.clone();
    let root = ask.root.clone();
    let report = flywheel::init::run(ask).expect("the instance bootstraps");
    assert_eq!(report.state, "hosted", "{report:?}");

    let host = flywheel::host::Host::open(&manifest, "mac-mini", Some(&root), at)
        .expect("the host opens over what init made");
    use flywheel_atoms::Records;
    let mut ahead: Vec<String> = Vec::new();
    for object in host
        .store
        .list_records(&flywheel_atoms::Scope::All)
        .expect("the state repository reads")
    {
        for (region, entered) in &object.entered_at {
            if *entered > at {
                ahead.push(format!(
                    "{}/{region} entered at {} — {}s ahead",
                    object.id,
                    entered.to_rfc3339(),
                    (*entered - at).num_seconds()
                ));
            }
        }
    }
    assert!(
        ahead.is_empty(),
        "the bootstrap stamped moments the clock has not reached: {ahead:?}"
    );
}

/// The repositories the operator names are tracked, and the first host's
/// declaration covers them (149, 199, 205, 206).
///
/// `covers: []` on a host means "everything the instance tracks", which is
/// right — and vacuous where the instance tracks nothing. `init` created the
/// blueprints and the state and no built repository, and offered no way to
/// name one, so a fresh instance declared coverage of nothing: every unit,
/// bolt and planning object waited under attention instead of being worked,
/// and `flywheel host seed` refused them. `covers: [all]` written in by hand
/// was the only way through, which puts a host's declaration in the operator's
/// text editor rather than in the manifest the machinery writes.
#[test]
fn the_repositories_an_instance_tracks_are_covered() {
    let mut sandbox = Sandbox::new("covers");
    sandbox.place_the_key();

    // With none named, the instance tracks none and the host covers none.
    // That is honest rather than broken: there is nothing to cover.
    let report = init::run(sandbox.ask()).expect("init runs");
    assert_eq!(report.state, "hosted", "{report:?}");
    let bare = flywheel_world_host::Manifest::read(&sandbox.dir.join("flywheel.yaml"))
        .expect("the manifest");
    assert!(bare.repositories.is_empty());
    assert!(
        bare.host("mac-mini").expect("the first host").covers.is_empty(),
        "an empty `covers:` is what the manifest writes; it means what the instance tracks"
    );

    // Named, they are created on the git host, tracked by the instance, and
    // covered by the App's installation — and the host that opens over that
    // manifest declares them without a word of hand-written YAML.
    let mut ask = sandbox.ask();
    ask.repositories = vec!["storefront".into(), "payments".into()];
    let report = init::run(ask).expect("init runs again with the repositories named");
    assert_eq!(report.state, "hosted", "{report:?}");
    let read = flywheel_world_host::Manifest::read(&sandbox.dir.join("flywheel.yaml"))
        .expect("the manifest");
    for named in ["storefront", "payments"] {
        assert!(read.repositories.contains_key(named), "{named} is tracked");
        assert!(
            read.app.installation_covers.iter().any(|r| r == named),
            "{named} is covered by the installation (207)"
        );
    }
    assert!(
        read.host("mac-mini").expect("the first host").covers.is_empty(),
        "still empty: the declaration follows what the instance tracks, and is not a copy of it"
    );

    // What the host declares when it opens: the repositories the instance
    // tracks, so an object naming one is covered and worked (149).
    let mut world = flywheel_world_host::HostWorld::open(read, "mac-mini").expect("the world");
    flywheel_world_host::join::join(&mut world).expect("the host joins");
    let host = flywheel::host::Host::open(
        &sandbox.dir.join("flywheel.yaml"),
        "mac-mini",
        None,
        chrono::Utc::now(),
    )
    .expect("the host opens");
    let mut declared = host.declaration.repositories.clone();
    declared.sort();
    assert_eq!(declared, vec!["payments".to_string(), "storefront".to_string()]);

    // And a unit in one of them is covered, which is the whole point: before
    // this it was a decision under attention that no answer could clear.
    let unit = flywheel_engine::Object {
        id: "unit/storefront/log-decline-code".into(),
        machine: "unit".into(),
        parent: None,
        config: Default::default(),
        entered_at: Default::default(),
        record: [("repository".to_string(), serde_json::json!("storefront"))]
            .into_iter()
            .collect(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    };
    assert!(host.declaration.covers(&unit), "{:?}", host.declaration);

    // Running it a third time with the same names changes nothing (204).
    let mut ask = sandbox.ask();
    ask.repositories = vec!["storefront".into(), "payments".into()];
    let again = init::run(ask).expect("a third run");
    assert!(
        !again.lines.iter().any(|l| l.contains("repository")),
        "a repository already tracked is made again: {:?}",
        again.lines
    );
}
