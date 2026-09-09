//! `--hosts real`: every host a scenario names is a process of its own (232,
//! D15).
//!
//! Under this flag the runner holds no engine. Each host of `given.hosts` is a
//! child of `std::env::current_exe()` with a root of its own and a port range
//! from its router, all of them pushing at one shared state repository, and the
//! runner asserts only through the store. That is what lets two writers race on
//! one object, which is the thing the whole profile rests on (134, 162, I15).
//!
//! The `start`, `lose`, `disconnect` and `return` steps drive those processes:
//! a start is a spawn, a loss is the process stopped without a farewell — so
//! its heartbeat simply stops, which is what makes it stale and then gone — and
//! a disconnect and a return are commands the running process answers.
//!
//! The children take the runner's virtual clock through an injected clock
//! source and their sweep is triggered by the runner rather than by a timer, so
//! a two-host run is deterministic (D15, D7).

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use flywheel_world_host::manifest::{App, Host as HostEntry, Manifest, Repository, Router};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// The binary a host is started as. Unset, the runner starts the process it is
/// — which under `flywheel scenario run` is the `flywheel` binary itself (D15).
/// A test that is not the binary points at it with this.
pub const BINARY_ENV: &str = "FLYWHEEL_BIN";

/// How many ports one host's router reserves, so two hosts on one computer
/// never collide (232).
pub const PORTS_PER_HOST: u16 = 10;

/// The first port a run's routers hand out.
pub const FIRST_PORT: u16 = 45000;

/// What a scenario said one of its hosts is. A host is what the manifest says
/// it is — how many sessions it runs at once, whether it is a laptop, what it
/// takes leases within — so a scenario's `given.hosts` becomes manifest entries
/// and nothing about a host is decided here (149, 150a, 183).
#[derive(Debug, Clone)]
pub struct HostSpec {
    pub name: String,
    pub bound: u32,
    pub intermittent: bool,
    pub covers: Vec<String>,
}

impl HostSpec {
    pub fn named(name: &str) -> HostSpec {
        HostSpec {
            name: name.to_string(),
            bound: 4,
            intermittent: false,
            covers: vec![],
        }
    }
}

/// One host, running.
struct Process {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Process {
    /// Say one thing and read the one line it answers. A host that says
    /// nothing is a host that died, and the caller is told which.
    fn say(&mut self, name: &str, command: &str) -> Result<String> {
        writeln!(self.input, "{command}").with_context(|| format!("telling {name} `{command}`"))?;
        self.input.flush()?;
        let mut line = String::new();
        let read = self
            .output
            .read_line(&mut line)
            .with_context(|| format!("reading {name}'s answer to `{command}`"))?;
        if read == 0 {
            bail!("host `{name}` stopped without answering `{command}`");
        }
        let line = line.trim().to_string();
        match line.strip_prefix("err ") {
            Some(why) => bail!("host `{name}` refused `{command}`: {why}"),
            None => Ok(line.trim_start_matches("ok ").to_string()),
        }
    }
}

/// The hosts of one run, and the shared state repository they push at.
pub struct RealHosts {
    /// Where everything this run makes lives.
    pub base: PathBuf,
    pub instance: String,
    /// The manifest every host reads itself from (183).
    pub manifest: PathBuf,
    /// Every host the scenario named, in the order it named them.
    pub names: Vec<String>,
    /// The first port of this run's range.
    pub first_port: u16,
    running: BTreeMap<String, Process>,
    /// What each host was last told the time is, so one started later starts at
    /// the run's clock and not at the wall's.
    now: DateTime<Utc>,
}

impl RealHosts {
    /// Lay out the roots, the shared state repository and the manifest. Nothing
    /// is started here.
    pub fn open(
        base: &Path,
        instance: &str,
        hosts: &[HostSpec],
        repositories: &[String],
        now: DateTime<Utc>,
    ) -> Result<RealHosts> {
        std::fs::create_dir_all(base)?;
        let names: Vec<String> = hosts.iter().map(|h| h.name.clone()).collect();
        let state = base.join("state").join("flywheel-state.git");
        let blueprints = base.join("state").join("flywheel-blueprints.git");
        let mut tracked: BTreeMap<String, Repository> = BTreeMap::new();
        for name in repositories {
            let bare = base.join("state").join(format!("{name}.git"));
            tracked.insert(
                name.clone(),
                Repository {
                    remote: bare.to_string_lossy().to_string(),
                    shared_line: "main".into(),
                    template_version: None,
                },
            );
        }
        let mut bares: Vec<PathBuf> = vec![state.clone(), blueprints.clone()];
        bares.extend(tracked.values().map(|r| PathBuf::from(&r.remote)));
        for bare in &bares {
            if !bare.exists() {
                flywheel_world_host::git::init_bare(bare)
                    .with_context(|| format!("making {}", bare.display()))?;
            }
        }
        // A run's ports are its own: several runs share one computer, and a
        // port a neighbour holds is not this run's to bind (232).
        let first_port = FIRST_PORT
            + ((std::process::id() as u16) % 300).saturating_mul(names.len().max(1) as u16 * PORTS_PER_HOST);
        let mut manifest = Manifest {
            instance: instance.to_string(),
            app: App::default(),
            router: Router {
                base: format!("http://{instance}.internal"),
            },
            blueprints: Repository {
                remote: blueprints.to_string_lossy().to_string(),
                shared_line: "main".into(),
                template_version: None,
            },
            state: Repository {
                remote: state.to_string_lossy().to_string(),
                shared_line: "main".into(),
                template_version: None,
            },
            repositories: tracked,
            hosts: Default::default(),
            template_version: None,
        };
        for (index, host) in hosts.iter().enumerate() {
            manifest.hosts.insert(
                host.name.clone(),
                HostEntry {
                    root: base.join("hosts").join(&host.name),
                    // The two bindings this release carries (93a, 93b, D8).
                    workspace: "recorded".into(),
                    sessions: "operator".into(),
                    covers: host.covers.clone(),
                    bound: host.bound,
                    intermittent: host.intermittent,
                    // Each host's own name on the operator's private network,
                    // and the port range that name routes to (191, 205a, D10a).
                    router: Some(Router {
                        base: format!("http://{}.{instance}.internal", host.name),
                    }),
                    localhost_port: first_port + index as u16 * PORTS_PER_HOST,
                },
            );
        }
        let path = base.join(flywheel_world_host::manifest::FILE);
        manifest.write(&path)?;
        Ok(RealHosts {
            base: base.to_path_buf(),
            instance: instance.to_string(),
            manifest: path,
            names,
            first_port,
            running: BTreeMap::new(),
            now,
        })
    }

    /// The shared state repository every host pushes at, and the runner reads.
    pub fn remote(&self) -> PathBuf {
        self.base.join("state").join("flywheel-state.git")
    }

    /// Where one host keeps its clones (205).
    pub fn root(&self, name: &str) -> PathBuf {
        self.base.join("hosts").join(name)
    }

    /// The ports one host's router reserves.
    pub fn port_range(&self, name: &str) -> (u16, u16) {
        let index = self.names.iter().position(|n| n == name).unwrap_or(0) as u16;
        let low = self.first_port + index * PORTS_PER_HOST;
        (low, low + PORTS_PER_HOST - 1)
    }

    /// The binary a host is started as.
    pub fn binary() -> Result<PathBuf> {
        if let Ok(p) = std::env::var(BINARY_ENV) {
            return Ok(PathBuf::from(p));
        }
        std::env::current_exe().context("the running binary has no path")
    }

    pub fn is_running(&self, name: &str) -> bool {
        self.running.contains_key(name)
    }

    pub fn started(&self) -> Vec<String> {
        self.running.keys().cloned().collect()
    }

    /// Start one host as a process of its own: its own root, its own port range,
    /// the one shared state repository, and the runner's clock (232, D15).
    pub fn start(&mut self, name: &str) -> Result<()> {
        if self.running.contains_key(name) {
            return Ok(());
        }
        if !self.names.iter().any(|n| n == name) {
            bail!("`{name}` is no host of this scenario; a host is declared before it is started");
        }
        let binary = Self::binary()?;
        let root = self.root(name);
        std::fs::create_dir_all(&root)?;
        let mut child = Command::new(&binary)
            .arg("host")
            .arg("--manifest")
            .arg(&self.manifest)
            .arg("--name")
            .arg(name)
            .arg("--root")
            .arg(&root)
            .arg("run")
            .arg("--driven")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("starting host `{name}` as {}", binary.display()))?;
        let input = child.stdin.take().ok_or_else(|| anyhow!("no input to host `{name}`"))?;
        let output = BufReader::new(
            child.stdout.take().ok_or_else(|| anyhow!("no output from host `{name}`"))?,
        );
        let mut process = Process { child, input, output };
        // It says it is ready before it has read anything.
        let mut hello = String::new();
        if process.output.read_line(&mut hello)? == 0 || !hello.starts_with(flywheel_ready()) {
            let _ = process.child.kill();
            let mut said = String::new();
            if let Some(mut err) = process.child.stderr.take() {
                use std::io::Read;
                let _ = err.read_to_string(&mut said);
            }
            bail!("host `{name}` did not open: {}", said.trim());
        }
        self.running.insert(name.to_string(), process);
        let now = self.now;
        self.tell(name, &format!("clock {}", now.to_rfc3339()))?;
        // A host that has started is alive: it writes its declaration and its
        // heartbeat before it ticks anything (147, 149, 163).
        self.tell(name, "declare")?;
        Ok(())
    }

    /// Stop a host without a farewell: no shutdown, no release. Its heartbeat
    /// simply stops, which is what makes it stale and then gone (S13, 150).
    pub fn lose(&mut self, name: &str) -> Result<()> {
        let Some(mut process) = self.running.remove(name) else {
            return Ok(());
        };
        let _ = process.child.kill();
        let _ = process.child.wait();
        Ok(())
    }

    /// The route is cut, or comes back (151, 165, D4a).
    pub fn disconnect(&mut self, name: &str) -> Result<()> {
        self.tell(name, "disconnect").map(|_| ())
    }

    pub fn reconnect(&mut self, name: &str) -> Result<()> {
        self.tell(name, "reconnect").map(|_| ())
    }

    /// A host returns: it starts again where it was lost, and reconnects where
    /// its route was cut (S13, S18).
    pub fn returned(&mut self, name: &str) -> Result<()> {
        match self.running.contains_key(name) {
            true => self.reconnect(name),
            false => self.start(name),
        }
    }

    /// Move every running host's clock. This is the only clock they have: no
    /// wall-clock time reaches a guard (D7, D15).
    pub fn set_clock(&mut self, now: DateTime<Utc>) -> Result<()> {
        self.now = now;
        for name in self.started() {
            self.tell(&name, &format!("clock {}", now.to_rfc3339()))?;
        }
        Ok(())
    }

    /// Tell every running host what the world reports. A host learns a local
    /// fact directly — a session on its own machine finishing is not something
    /// it fetches (B.3, 151).
    pub fn tell_given(&mut self, object: &str, name: &str, value: &serde_json::Value) -> Result<()> {
        for host in self.started() {
            self.tell(&host, &format!("given {object}\t{name}\t{value}"))?;
        }
        Ok(())
    }

    /// Sweep the named hosts, in the order named. The runner triggers the
    /// sweep; nothing in the child keeps time (D7, D15).
    pub fn sweep(&mut self, names: &[String]) -> Result<usize> {
        let mut fired = 0;
        for name in names {
            if !self.running.contains_key(name) {
                continue;
            }
            let said = self.tell(name, "sweep")?;
            fired += said
                .split_whitespace()
                .nth(1)
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(0);
        }
        Ok(fired)
    }

    /// Sweep several hosts at once: each is told to sweep before any of them is
    /// waited on, so they plan from the one read they share and their writes
    /// race. That is what puts two writers on one object (134, 162, I15).
    pub fn sweep_concurrently(&mut self, names: &[String]) -> Result<usize> {
        let running: Vec<String> = names
            .iter()
            .filter(|n| self.running.contains_key(*n))
            .cloned()
            .collect();
        for name in &running {
            let process = self
                .running
                .get_mut(name)
                .ok_or_else(|| anyhow!("host `{name}` is not running"))?;
            writeln!(process.input, "sweep")?;
            process.input.flush()?;
        }
        let mut fired = 0;
        for name in &running {
            let process = self
                .running
                .get_mut(name)
                .ok_or_else(|| anyhow!("host `{name}` is not running"))?;
            let mut line = String::new();
            if process.output.read_line(&mut line)? == 0 {
                bail!("host `{name}` stopped without answering `sweep`");
            }
            let line = line.trim();
            if let Some(why) = line.strip_prefix("err ") {
                bail!("host `{name}` refused `sweep`: {why}");
            }
            fired += line
                .split_whitespace()
                .nth(2)
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or(0);
        }
        Ok(fired)
    }

    /// Sweep every running host, in name order, so a run is the same run twice.
    pub fn sweep_all(&mut self) -> Result<usize> {
        let names = self.started();
        self.sweep(&names)
    }

    /// Say one thing to one host and read its answer.
    pub fn tell(&mut self, name: &str, command: &str) -> Result<String> {
        let process = self
            .running
            .get_mut(name)
            .ok_or_else(|| anyhow!("host `{name}` is not running"))?;
        process.say(name, command)
    }

    /// Stop every host, with a farewell where one is still listening.
    pub fn shutdown(&mut self) {
        for name in self.started() {
            let _ = self.tell(&name, "quit");
            if let Some(mut process) = self.running.remove(&name) {
                let _ = process.child.wait();
            }
        }
    }
}

impl Drop for RealHosts {
    fn drop(&mut self) {
        for (_, mut process) in std::mem::take(&mut self.running) {
            let _ = process.child.kill();
            let _ = process.child.wait();
        }
    }
}

/// What a host says when it is open. Held in one place so the two sides cannot
/// drift.
fn flywheel_ready() -> &'static str {
    "ready "
}
