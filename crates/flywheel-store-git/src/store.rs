//! `GitStore`: the eight operations of 125 and the six record operations under
//! them, over the state repository.

use crate::git::{self, Repo};
use crate::layout;
use crate::objects;
use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Duration, Utc};
use flywheel_atoms::{
    EffectWrite, EvidenceRead, HostRecord, LeaseOp, LeaseOutcome, LeaseRecord, Listing, Notice,
    Object, Presentation, PutOutcome, ReadPoint, Received, Records, Scope, StateStore, StatusView,
    ThreadEntry, WriteOutcome,
};
use flywheel_domain::{envelope, records};
use flywheel_engine::rec;
use flywheel_engine::runtime::Response;
use serde_json::Value;
use std::collections::BTreeMap;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// What the operator's own commit says of itself in its subject line, so every
/// host reads the same delivery id out of the same fetched history (164).
pub const OPERATOR: &str = "operator commit";

/// How many times a rejected push is rebased and retried before the host
/// reports and re-reads (`git-only.yaml records.put`).
pub const RETRIES: usize = 3;

/// How often a lease is renewed while its holder works. Once a minute, whatever
/// happens in between: a lease covers an object for the whole time a session
/// runs on it, and a renewal per pass would be a write per pass with nothing to
/// say (128, 150, `git-only.yaml` records.leases).
pub const RENEW_EVERY: Duration = Duration::minutes(1);

/// The state repository as one host sees it.
#[derive(Debug)]
pub struct GitStore {
    /// The host's working checkout of the shared line.
    pub repo: Repo,
    /// Who this host is. Every lease and heartbeat is taken in this name.
    pub host: String,
    /// The point the host has fetched to: what every read is as of (126).
    pub fetched: String,
    /// The virtual clock, injected so no wall-clock time reaches a guard (D15).
    pub now: DateTime<Utc>,
    /// The engine windows the manifest gives; the release's defaults otherwise.
    pub lease_expiry: Duration,
    pub lease_stale: Duration,
    /// A host with no route to the git host keeps ticking what it holds and
    /// commits locally; its writes are intentions until they land (151, 161).
    pub disconnected: bool,
    /// Evidence the world reports, which the git profile inherits from the
    /// host and sessions bindings rather than owning (B.3).
    pub given: BTreeMap<String, BTreeMap<String, Value>>,
    /// The same, as the shared line holds it: read at every fetch, so every
    /// host of one instance answers it alike (B.3, 136).
    pub shared: BTreeMap<String, BTreeMap<String, Value>>,
    /// What this run has done, for the observations a scenario asserts.
    pub writes_attempted: usize,
    pub writes_rejected: usize,
    /// What this tick has cost: the two kinds of process a tick makes, and the
    /// renewals among the pushes (169, `git-only.yaml` cost).
    fetches: u32,
    pushes: u32,
    lease_renewals: u32,
    /// What the repository had spawned when this tick began, so the count is
    /// this tick's and not the run's.
    spawned_at_tick: u32,
    /// The objects this tick's commits were written against, by the sequence
    /// each was read at: what tells a lost race from a stale base when the
    /// tick's one push is refused (134, 162, I15).
    guards: BTreeMap<String, u64>,
    /// Whether a tick is open. Its writes are one push, made when it ends.
    in_tick: bool,
    pub reread_after_rejection: bool,
    pub reads_since_notify: usize,
    /// Effect ids written locally but not yet landed (161, D4a).
    pub pending: Vec<String>,
    /// What has already been read at the fetched commit: the commit it was
    /// read at, the files, and the trees. A commit is immutable, so this is a
    /// memory and never a second source of truth (126, 135).
    read_at: RefCell<(String, BTreeMap<String, Option<String>>, BTreeMap<String, Vec<String>>)>,
    /// The heartbeats as the refs held them when they were last read. Refs move
    /// without the shared line moving, so this is discarded on every fetch and
    /// on every write of a heartbeat or a lease (D5).
    heartbeats: RefCell<Option<Vec<HostRecord>>>,
    /// The repository itself, held open: refs, trees, blobs, the fetch and the
    /// commits this host writes, all in process (`git-only.yaml`, model.md §13).
    odb: gix::Repository,
    /// Where the shared line is fetched from: the remote the manifest names, as
    /// this checkout's `origin` holds it (133, 161).
    remote: String,
    /// That remote, opened at the first fetch and held: a tick's fetch is the
    /// objects that moved and not a repository opened again (169).
    origin: Option<objects::Origin>,
}

impl GitStore {
    /// Open a host's checkout of a state repository.
    pub fn open(remote: &Path, root: &Path, host: &str, now: DateTime<Utc>) -> Result<GitStore> {
        let repo = if root.join(".git").is_dir() {
            Repo::at(root)
        } else {
            git::clone(remote, root)?
        };
        let repo_dir = repo.dir.clone();
        let odb = objects::open(&repo_dir)?;
        // A checkout names its own remote, which is what a host opened at a
        // directory it already has must fetch from; the argument is the remote
        // for the clone that has yet to happen.
        let remote = objects::origin_url(&odb)
            .unwrap_or_else(|| remote.to_string_lossy().to_string());
        let mut store = GitStore {
            repo,
            host: host.to_string(),
            fetched: String::new(),
            now,
            lease_expiry: Duration::hours(24),
            lease_stale: Duration::minutes(5),
            disconnected: false,
            given: BTreeMap::new(),
            shared: BTreeMap::new(),
            writes_attempted: 0,
            writes_rejected: 0,
            fetches: 0,
            pushes: 0,
            lease_renewals: 0,
            spawned_at_tick: 0,
            guards: BTreeMap::new(),
            in_tick: false,
            reread_after_rejection: false,
            reads_since_notify: 0,
            pending: vec![],
            read_at: RefCell::new((String::new(), BTreeMap::new(), BTreeMap::new())),
            heartbeats: RefCell::new(None),
            odb,
            remote,
            origin: None,
        };
        store.fetch()?;
        Ok(store)
    }

    /// Fetch and integrate the shared line. Every tick does this first, so no
    /// host decides on a read older than the bound (165, D7).
    pub fn fetch(&mut self) -> Result<String> {
        if !self.disconnected {
            // A fetch that cannot reach the git host is not an error: the host
            // keeps working what it already holds (151). No process at all: the
            // fetch is `gix`'s, in this host's own process, and the `git` binary
            // is spawned for the guarded push alone (169, `git-only.yaml`
            // Tools, model.md §12.8).
            self.fetches += 1;
            // The refspec is stated rather than left to the clone's own
            // configuration: the remote-tracking refs are what every read's
            // point and every write's expected-old are taken from (165, 134).
            if self.origin.is_none() {
                self.origin = objects::origin(&self.remote).ok();
            }
            if let Some(origin) = &self.origin {
                let _ = objects::fetch(&self.odb, origin, true);
            }
        }
        // A host with no route reads its own line: its local commits are
        // intentions, not facts (161), but they are the state it is working
        // and it must read back what it wrote or it would decide the same
        // thing again on every tick (151, D4a).
        self.fetched = match self.disconnected {
            true => objects::rev(&self.odb, "HEAD")?.or(self.shared_head()?),
            false => self.shared_head()?.or(objects::rev(&self.odb, "HEAD")?),
        }
        .unwrap_or_else(|| git::ZERO.to_string());
        // The checkout is what every read is answered from, so it goes to what
        // came back — unless this host's own commits are ahead of it, which are
        // intentions it keeps (161, D4a, 169).
        self.forget();
        self.on_fetched_head()?;
        // What the world reports comes down the shared line with everything
        // else, so a host that has just fetched answers the same evidence as
        // every other host of this instance (B.3, 136).
        self.shared = self.read_given()?;
        // The refs moved with the fetch, so what was read of them is read again.
        *self.heartbeats.borrow_mut() = None;
        Ok(self.fetched.clone())
    }

    /// Where the shared line stands, as the fetch left it.
    ///
    /// The remote-tracking ref where the ref store holds one, and what the
    /// fetch itself said otherwise: `FETCH_HEAD` names the head of every branch
    /// the fetch brought, which is a file read and not a process (165, 169).
    fn shared_head(&self) -> Result<Option<String>> {
        if let Some(sha) = objects::rev(&self.odb, layout::ORIGIN_MAIN)? {
            return Ok(Some(sha));
        }
        let path = self.repo.dir.join(".git").join("FETCH_HEAD");
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Ok(None);
        };
        for line in text.lines() {
            let mut parts = line.split('\t');
            let Some(sha) = parts.next() else { continue };
            let _merge = parts.next();
            let what = parts.next().unwrap_or_default();
            if what.contains(&format!("branch '{}'", layout::SHARED_LINE)) {
                return Ok(Some(sha.trim().to_string()));
            }
        }
        Ok(None)
    }

    /// The world's answers as the shared line holds them.
    fn read_given(&self) -> Result<BTreeMap<String, BTreeMap<String, Value>>> {
        let Some(text) = self.read_file(layout::GIVEN)? else {
            return Ok(BTreeMap::new());
        };
        let mut out: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
        for record in rec::parse(&text) {
            let (Some(object), Some(name), Some(value)) =
                (record.get("object"), record.get("name"), record.get("value"))
            else {
                continue;
            };
            out.entry(object.to_string()).or_default().insert(
                name.to_string(),
                serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.to_string())),
            );
        }
        Ok(out)
    }

    /// Write one of the world's answers on to the shared line, so every host
    /// reads it (B.3). What the recorded workspace and the operator's sessions
    /// answer is written as facts; this is the rest, which phase 1's bindings
    /// inherit rather than own.
    pub fn commit_given(&mut self, object: &str, name: &str, value: &Value) -> Result<()> {
        let mut held = self.read_given()?;
        held.entry(object.to_string())
            .or_default()
            .insert(name.to_string(), value.clone());
        let mut text = String::from("%rec: given\n\n");
        for (object, per) in &held {
            for (name, value) in per {
                text.push_str(&format!("object: {object}\nname: {name}\nvalue: {value}\n\n"));
            }
        }
        self.on_fetched_head()?;
        self.write_file(layout::GIVEN, &text)?;
        self.commit_and_push(&format!(
            "{object} {name}\n\nreason: what the world reports, which this profile inherits (B.3)"
        ))?;
        self.shared = held;
        Ok(())
    }

    /// The point a read is as of: the commit, and the time the store says it is.
    pub fn as_of(&self) -> ReadPoint {
        ReadPoint {
            mark: self.fetched.clone(),
            seq: 0,
            at: self.now,
        }
    }

    fn at(&self) -> &str {
        if self.fetched.is_empty() {
            git::ZERO
        } else {
            &self.fetched
        }
    }

    /// One file as of the fetched commit.
    ///
    /// The checkout is at that commit — a fetch puts it there and every write
    /// this host makes goes through the tree — so a read is a file read and no
    /// process is spawned on the read path (126, 135, 169, `git-only.yaml`
    /// cost). A commit is immutable, so what a path holds at one is read once
    /// and remembered: a tick reads the same object under a dozen guards. The
    /// memory is discarded whenever the fetched point moves.
    fn read_file(&self, path: &str) -> Result<Option<String>> {
        if self.fetched.is_empty() || self.fetched == git::ZERO {
            return Ok(None);
        }
        {
            let read = self.read_at.borrow();
            if read.0 == self.fetched {
                if let Some(held) = read.1.get(path) {
                    return Ok(held.clone());
                }
            }
        }
        let text = match std::fs::read(self.repo.dir.join(path)) {
            Ok(body) => Some(String::from_utf8_lossy(&body).to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e).with_context(|| format!("reading {path}")),
        };
        let mut read = self.read_at.borrow_mut();
        if read.0 != self.fetched {
            read.0 = self.fetched.clone();
            read.1.clear();
            read.2.clear();
        }
        read.1.insert(path.to_string(), text.clone());
        Ok(text)
    }

    /// The paths under a prefix as of the fetched commit, remembered the same
    /// way and for the same reason: a directory listing of the checkout.
    fn tree(&self, prefix: &str) -> Result<Vec<String>> {
        {
            let read = self.read_at.borrow();
            if read.0 == self.at() {
                if let Some(held) = read.2.get(prefix) {
                    return Ok(held.clone());
                }
            }
        }
        let mut paths = Vec::new();
        under(&self.repo.dir, prefix.trim_end_matches('/'), &mut paths);
        paths.sort();
        let mut read = self.read_at.borrow_mut();
        if read.0 != self.at() {
            read.0 = self.at().to_string();
            read.1.clear();
            read.2.clear();
        }
        read.2.insert(prefix.to_string(), paths.clone());
        Ok(paths)
    }

    /// Put the checkout on the fetched head, ready to commit. In process: the
    /// paths that differ are written or removed and the branch is moved, which
    /// is what `git reset --hard` does without the process (169).
    fn on_fetched_head(&self) -> Result<()> {
        if self.fetched.is_empty() || self.fetched == git::ZERO {
            return Ok(());
        }
        // Only move to the fetched head when nothing local is ahead of it: a
        // disconnected host's own commits are intentions it keeps (161, D4a).
        let head = objects::rev(&self.odb, "HEAD")?;
        if head.as_deref() == Some(self.fetched.as_str()) {
            return Ok(());
        }
        let behind = match &head {
            Some(head) => objects::reaches(&self.odb, head, &self.fetched)?,
            None => true,
        };
        if behind {
            self.hard_reset(&self.fetched)?;
        }
        Ok(())
    }

    /// Put the branch and the working tree at one commit.
    fn hard_reset(&self, to: &str) -> Result<()> {
        let from = objects::rev(&self.odb, "HEAD")?.unwrap_or_default();
        objects::put_worktree_at(&self.odb, &self.repo.dir, &from, to)?;
        objects::set_ref(&self.odb, layout::MAIN, to, None)?;
        self.read_at.borrow_mut().0 = String::new();
        Ok(())
    }

    /// Push a local commit at a remote ref, expecting it where we left it. The
    /// one process a write makes, and the compare-and-swap the whole profile
    /// rests on (128, 134, 162, I15, 169).
    fn push(&mut self, remote_ref: &str, expected_old: &str) -> Result<bool> {
        self.pushes += 1;
        // The local branch by name, not `HEAD`: a push whose source is the
        // branch is what moves this host's remote-tracking ref for it, and the
        // tracking ref is what the next write's expected-old is read from
        // (134, 162, 165).
        git::push_expecting(&self.repo, layout::MAIN, remote_ref, expected_old)
    }

    /// What a push that landed leaves behind: the shared line is where this
    /// host just put it, so nothing is fetched (165, 169).
    fn landed(&mut self, sha: &str) {
        self.fetched = sha.to_string();
        self.forget();
    }

    /// Forget what was read at the old point. A commit is immutable, so a read
    /// is remembered while the point stands and dropped the moment it moves
    /// (126, 135).
    fn forget(&self) {
        let mut read = self.read_at.borrow_mut();
        read.0 = String::new();
        read.1.clear();
        read.2.clear();
    }

    /// Write one file into the checkout. The commit's tree is written from the
    /// checkout, so this is the whole of a staged write (169).
    fn write_file(&self, path: &str, body: &str) -> Result<()> {
        git::stage(&self.repo, path, body)?;
        self.forget();
        Ok(())
    }

    /// Commit the checkout as it stands on the local branch, in process.
    fn commit_worktree(&self, message: &str) -> Result<String> {
        let parent = objects::rev(&self.odb, "HEAD")?.unwrap_or_default();
        let tree = objects::write_tree(&self.odb, &self.repo.dir)?;
        let sha = objects::write_commit(
            &self.odb,
            tree,
            std::slice::from_ref(&parent),
            message,
            self.now,
        )?;
        objects::set_ref(&self.odb, layout::MAIN, &sha, None)?;
        Ok(sha)
    }

    /// Commit what has been staged and push `main` with expected-old. On
    /// rejection: fetch, rebase the one-file commit, push again; three
    /// rejections report and re-read (`git-only.yaml records.put`, D4).
    fn commit_and_push(&mut self, message: &str) -> Result<Landed> {
        self.commit_and_push_guarded(message, None)
    }

    /// The same, with the object the commit writes and the sequence it was
    /// written against. On a rejection the host fetches and looks: if the
    /// object moved under it, another writer got there first and the local
    /// commit is discarded — a rebase would put this host's state over one it
    /// never read (134, 162, I15).
    fn commit_and_push_guarded(
        &mut self,
        message: &str,
        guard: Option<(&str, u64)>,
    ) -> Result<Landed> {
        let sha = self.commit_worktree(message)?;
        self.writes_attempted += 1;
        if let Some((id, base_seq)) = guard {
            self.guards.insert(id.to_string(), base_seq);
        }
        if self.disconnected {
            // A local commit is an intention, not a fact (161).
            return Ok(Landed::Pending { sha });
        }
        // Inside a tick the commit is made here and the push is the tick's:
        // one write to the central service per tick, carrying every commit the
        // tick made, which is what the profile's mechanism spends (167, 169,
        // `git-only.yaml` cost). Outside one — seeding, a report from a place,
        // a call from the page — the write goes at the shared line at once,
        // because nothing else will send it.
        match self.in_tick {
            true => Ok(Landed::Written { sha }),
            false => self.flush(),
        }
    }

    /// Send this tick's commits at the shared line, once. On rejection: fetch,
    /// look at what moved, replay and push again; three rejections report and
    /// re-read (`git-only.yaml records.put`, D4, 134, 162).
    ///
    /// An object another writer moved under us is a loss, not a rebase: putting
    /// this host's state over one it never read is the thing the guard is for
    /// (134, 162, I15).
    pub fn flush(&mut self) -> Result<Landed> {
        let guards = std::mem::take(&mut self.guards);
        let Some(mut sha) = objects::rev(&self.odb, "HEAD")? else {
            return Ok(Landed::Written { sha: String::new() });
        };
        if self.disconnected || sha == self.fetched {
            return Ok(Landed::Written { sha });
        }
        for attempt in 0..RETRIES {
            let expected = if self.fetched.is_empty() {
                git::ZERO.to_string()
            } else {
                self.fetched.clone()
            };
            if self.push(layout::MAIN, &expected)? {
                // A push that landed is the new shared line: there is nothing
                // to fetch, and the read this host holds is still the one it
                // just wrote (165, 169).
                self.landed(&sha);
                return Ok(Landed::Written { sha });
            }
            self.writes_rejected += 1;
            // The loser learns that it lost and reads again before deciding
            // anything (134).
            self.fetch()?;
            self.reread_after_rejection = true;
            let onto = self.fetched.clone();
            for (id, base_seq) in &guards {
                let held = self.get(id)?.map(|o| o.seq).unwrap_or(0);
                if held != *base_seq {
                    self.hard_reset(&onto)?;
                    return Ok(Landed::Lost);
                }
            }
            if attempt + 1 == RETRIES {
                break;
            }
            if !self.replay_onto(&sha, &onto)? {
                // Two writers on one file is a loss: the host discards its
                // local commits and takes what it found (3, 164, I15, S19).
                self.hard_reset(&onto)?;
                return Ok(Landed::Lost);
            }
            sha = self.commit_worktree("this tick's writes, replayed on what it found")?;
        }
        Ok(Landed::Refused)
    }

    /// Put this host's own commit on top of what it found, in process: the
    /// paths its commit touched written over the new base. A path the other
    /// writer touched too is a content conflict, and a conflict is a loss
    /// (3, 134, 164, I15).
    fn replay_onto(&mut self, ours: &str, onto: &str) -> Result<bool> {
        let parent = objects::first_parent(&self.odb, ours)?;
        let mine = objects::changed_paths(&self.odb, &parent, ours)?;
        let theirs = objects::changed_paths(&self.odb, &parent, onto)?;
        if mine.iter().any(|path| theirs.contains(path)) {
            return Ok(false);
        }
        self.hard_reset(onto)?;
        for path in &mine {
            let full = self.repo.dir.join(path);
            match objects::blob_at(&self.odb, ours, path)? {
                Some(body) => {
                    if let Some(parent) = full.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&full, body)
                        .with_context(|| format!("writing {}", full.display()))?;
                }
                None => {
                    let _ = std::fs::remove_file(&full);
                }
            }
        }
        self.forget();
        Ok(true)
    }

    /// The lease branches, by object.
    fn lease_record(&self, object: &str) -> Result<Option<LeaseRecord>> {
        let reference = layout::lease_ref(object);
        let Some(sha) = objects::rev(&self.odb, &layout::lease_remote(object))?
            .or(objects::rev(&self.odb, &reference)?)
        else {
            return Ok(None);
        };
        let Some(body) = objects::blob_at(&self.odb, &sha, layout::RECORD)? else {
            return Ok(None);
        };
        let text = String::from_utf8_lossy(&body).to_string();
        let parsed = rec::parse(&text);
        let Some(record) = parsed.first() else {
            return Ok(None);
        };
        Ok(Some(records::lease_from_record(record)?))
    }

    /// Write one orphan commit whose tree is the record, and push the branch
    /// with expected-old. The push *is* the compare-and-swap (D5).
    ///
    /// The blob, the tree, the commit and the local ref are written in process,
    /// so a renewal costs one process and disturbs nothing in the checkout: a
    /// lease branch is never a file on `main` and never touches the work in the
    /// tree (D5, 169).
    fn put_orphan(&mut self, reference: &str, record: &rec::Record, expected: &str) -> Result<bool> {
        let text = rec::write(std::slice::from_ref(record));
        let tree = objects::write_one_file_tree(&self.odb, layout::RECORD, &text)?;
        let commit = objects::write_commit(
            &self.odb,
            tree,
            &[],
            &format!("{reference} at {}", self.now.to_rfc3339()),
            self.now,
        )?;
        objects::set_ref(&self.odb, reference, &commit, None)?;
        self.pushes += 1;
        git::push_expecting(&self.repo, &commit, reference, expected)
    }
}

/// What became of a commit that tried to land.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Landed {
    Written { sha: String },
    /// Made locally while disconnected: an intention until its push lands (161).
    Pending { sha: String },
    /// The operator's own commit stands and ours was discarded (3, 164, I15).
    Lost,
    /// Three rejections: report and re-read.
    Refused,
}

// ------------------------------------------------------------- record layer

impl Records for GitStore {
    fn get(&self, id: &str) -> Result<Option<Object>> {
        // A lease lives on a branch, never as a file on the shared line (D5):
        // its object is made from the lease record.
        if let Some(object) = flywheel_domain::leases::object_of(id) {
            let held = self.lease_record(object)?;
            return Ok(Some(flywheel_domain::leases::as_object(
                object,
                held.as_ref(),
                self.now,
            )));
        }
        let Some(text) = self.read_file(&layout::object(id))? else {
            return Ok(None);
        };
        let parsed = rec::parse(&text);
        let Some(record) = parsed.first() else {
            return Ok(None);
        };
        Ok(Some(envelope::from_record(record)?))
    }

    fn put(&mut self, id: &str, record: &Object, base_seq: u64) -> Result<PutOutcome> {
        // The state a lease's machine reached is marked on the lease, so
        // `main`'s history stays state changes and nothing else (D5, 167).
        if let Some(object) = flywheel_domain::leases::object_of(id) {
            let state = record.config.get("hold").cloned().unwrap_or_default();
            self.lease(&LeaseOp::Mark {
                object: object.to_string(),
                state,
            })?;
            return Ok(PutOutcome::Written { seq: record.seq + 1 });
        }
        let held = self.get(id)?.map(|o| o.seq).unwrap_or(0);
        if held != base_seq {
            return Ok(PutOutcome::Rejected { held_seq: held });
        }
        self.on_fetched_head()?;
        let mut next = record.clone();
        next.seq = held + 1;
        self.write_file(&layout::object(id),
            &envelope::write_all(std::slice::from_ref(&next)),
        )?;
        let message = format!("{id} seq {}\n\nreason: state written", next.seq);
        match self.commit_and_push_guarded(&message, Some((id, base_seq)))? {
            Landed::Written { .. } | Landed::Pending { .. } => {
                Ok(PutOutcome::Written { seq: next.seq })
            }
            Landed::Lost | Landed::Refused => {
                let held = self.get(id)?.map(|o| o.seq).unwrap_or(0);
                Ok(PutOutcome::Rejected { held_seq: held })
            }
        }
    }

    fn append(&mut self, id: &str, entry: &ThreadEntry) -> Result<()> {
        self.on_fetched_head()?;
        let path = layout::thread(id);
        let mut existing = self.read_file(&path)?.unwrap_or_default();
        let written = rec::write(std::slice::from_ref(&records::thread_to_record(entry)));
        // Append-only: the thread is the object's history and nothing rewrites
        // it (144).
        existing.push_str(&written);
        self.write_file(&path, &existing)?;
        self.commit_and_push(&format!("{id} thread {}", entry.kind))?;
        Ok(())
    }

    fn list_records(&self, scope: &Scope) -> Result<Vec<Object>> {
        let mut out = Vec::new();
        for path in self.tree("objects/")? {
            let Some(id) = layout::id_of(&path) else { continue };
            let Some(object) = self.get(id)? else { continue };
            let keep = match scope {
                Scope::All => true,
                Scope::Machine(m) => &object.machine == m,
                Scope::Under(under) => {
                    &object.id == under || object.parent.as_deref() == Some(under.as_str())
                }
            };
            if keep {
                out.push(object);
            }
        }
        Ok(out)
    }

    fn thread(&self, id: &str) -> Result<Vec<ThreadEntry>> {
        let Some(text) = self.read_file(&layout::thread(id))? else {
            return Ok(vec![]);
        };
        rec::parse(&text)
            .iter()
            .map(records::thread_from_record)
            .collect()
    }

    fn responses(&self, id: &str) -> Result<Vec<Response>> {
        let mut out = Vec::new();
        for path in self.tree(layout::RESPONSES)? {
            let Some(text) = self.read_file(&path)? else { continue };
            for record in rec::parse(&text) {
                let response = records::response_from_record(&record)?;
                // The responses in hand, or those this object's own record and
                // register entries name (`record-derived.yaml`).
                if id == "rail" || response.object.as_deref() == Some(id) {
                    out.push(response);
                }
            }
        }
        out.sort_by(|a, b| a.given_at.cmp(&b.given_at).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    fn leases(&self, id: &str) -> Result<Option<LeaseRecord>> {
        self.lease_record(id)
    }

    fn hosts(&self) -> Result<Vec<HostRecord>> {
        if let Some(held) = self.heartbeats.borrow().as_ref() {
            return Ok(held.clone());
        }
        let mut hosts = Vec::new();
        for (reference, sha) in objects::refs_under(&self.odb, layout::HOSTS_PREFIX)? {
            // A heartbeat branch and no other: the leaf is what says so
            // (`git-only.yaml layout`).
            if layout::host_of_ref(&reference).is_none() {
                continue;
            }
            let Some(body) = objects::blob_at(&self.odb, &sha, layout::RECORD)? else { continue };
            let text = String::from_utf8_lossy(&body).to_string();
            if let Some(record) = rec::parse(&text).first() {
                if let Ok(host) = records::host_from_record(record) {
                    hosts.push(host);
                }
            }
        }
        *self.heartbeats.borrow_mut() = Some(hosts.clone());
        Ok(hosts)
    }
}

impl GitStore {
    /// A lease whose holder stopped renewing past the expiry is free to take
    /// (128, 163).
    pub fn lease_expired(&self, lease: &LeaseRecord) -> bool {
        self.now - lease.renewed_at > self.lease_expiry
    }

    /// A lease not renewed inside the stale window is shown as stale, which is
    /// not yet expired (163).
    pub fn lease_stale(&self, lease: &LeaseRecord) -> bool {
        self.now - lease.renewed_at > self.lease_stale
    }

    /// Write this host's heartbeat on its own branch, never on `main` (D5).
    pub fn heartbeat(&mut self, bound: u32, intermittent: bool) -> Result<()> {
        if self.disconnected {
            return Ok(());
        }
        let reference = layout::host_ref(&self.host);
        let expected = objects::rev(&self.odb, &layout::host_remote(&self.host))?
            .unwrap_or_else(|| git::ZERO.to_string());
        let record = records::host_to_record(&HostRecord {
            host: self.host.clone(),
            last_seen: self.now,
            bound,
            intermittent,
        });
        self.put_orphan(&reference, &record, &expected)?;
        *self.heartbeats.borrow_mut() = None;
        Ok(())
    }
}

// ------------------------------------------------------------ the state store

impl StateStore for GitStore {
    /// The tick's writes go at the shared line here, in one push (167, 169).
    fn end_tick(&mut self) -> Result<()> {
        self.in_tick = false;
        self.flush()?;
        Ok(())
    }

    /// A tick's cost is counted from here (169, `git-only.yaml` cost).
    fn begin_tick(&mut self) {
        self.in_tick = true;
        self.fetches = 0;
        self.pushes = 0;
        self.lease_renewals = 0;
        self.spawned_at_tick = self.repo.spawned();
    }

    /// What this tick has cost. The read path spawns nothing: a read is the
    /// checkout as of the point the tick fetched, never a call per object
    /// (126, 165, 169). Nor does the fetch, which is `gix` in this process, so
    /// every process a tick spawns is a push (`git-only.yaml` Tools, §12.8).
    fn cost(&self) -> flywheel_atoms::Cost {
        let spawned = self.repo.spawned().saturating_sub(self.spawned_at_tick);
        flywheel_atoms::Cost {
            fetches: self.fetches,
            pushes: self.pushes,
            lease_renewals: self.lease_renewals,
            read_processes: spawned.saturating_sub(self.pushes),
            subprocesses: spawned,
        }
    }

    fn read(&self, id: &str) -> Result<EvidenceRead> {
        let object = self.get(id)?;
        let mut evidence: BTreeMap<String, Value> = BTreeMap::new();
        if let Some(o) = &object {
            evidence.insert("state".into(), serde_json::to_value(&o.config)?);
            evidence.insert("entered_at".into(), serde_json::to_value(&o.entered_at)?);
            evidence.insert("seq".into(), serde_json::json!(o.seq));
            evidence.insert(
                "applied_responses".into(),
                serde_json::to_value(&o.applied_responses)?,
            );
        }
        evidence.insert("now".into(), serde_json::json!(self.now.to_rfc3339()));
        // What the world reports, which this profile inherits (B.3).
        for source in [id, "*"] {
            if let Some(per) = self.given.get(source) {
                for (name, value) in per {
                    evidence.entry(name.clone()).or_insert_with(|| value.clone());
                }
            }
        }
        Ok(EvidenceRead {
            object,
            evidence,
            as_of: self.as_of(),
        })
    }

    fn list(&self, scope: &Scope) -> Result<Listing> {
        Ok(Listing {
            objects: self.list_records(scope)?,
            as_of: self.as_of(),
        })
    }

    fn status(&self) -> Result<StatusView> {
        // Written from `list` and `get` alone, stating the commit and time it
        // is as of (132, 141–146).
        let as_of = self.as_of();
        let mut body = format!(
            "<!-- as of {} at {} -->\n",
            as_of.mark,
            as_of.at.to_rfc3339()
        );
        for object in self.list_records(&Scope::All)? {
            body.push_str(&format!(
                "{}\t{}\t{}\n",
                object.id,
                object.machine,
                object.top_states().join(",")
            ));
        }
        Ok(StatusView { as_of, body })
    }

    fn write_effect(&mut self, write: &EffectWrite) -> Result<WriteOutcome> {
        // A repeat is found by scanning the history for the effect id before
        // committing, and changes nothing (127).
        //
        // From `HEAD`, not from the fetched point: a tick's commits are local
        // until it ends, so `self.fetched` does not move inside one, and a
        // second write of the same effect id in the same tick would find
        // nothing and commit again. `HEAD` is what this host has written,
        // landed or not.
        let scanning = objects::rev(&self.odb, "HEAD")?
            .filter(|head| !head.is_empty() && head != git::ZERO)
            .or_else(|| {
                Some(self.fetched.clone()).filter(|f| !f.is_empty() && f != git::ZERO)
            });
        if let Some(from) = scanning {
            if objects::message_holds(&self.odb, &from, &write.effect_id)? {
                return Ok(WriteOutcome::AlreadyWritten {
                    effect_id: write.effect_id.clone(),
                });
            }
        }
        if self.pending.contains(&write.effect_id) {
            return Ok(WriteOutcome::AlreadyWritten {
                effect_id: write.effect_id.clone(),
            });
        }
        self.on_fetched_head()?;
        let evidence = write
            .evidence
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ");
        // The message carries the identity, the reason and the evidence the
        // guard read (79, 127, 167).
        let message = format!(
            "{}\n\nreason: {}\nevidence: {}\nobject: {}\neffect: {}",
            write.effect_id, write.reason, evidence, write.object, write.effect
        );
        let landed = self.commit_and_push(&message)?;
        // Every id this host has written goes on the list, whatever the
        // connectivity: a commit that has not been scanned for yet is still one
        // this host made, and the effect is not written twice (127).
        if !self.pending.contains(&write.effect_id) {
            self.pending.push(write.effect_id.clone());
        }
        match landed {
            Landed::Written { .. } => Ok(WriteOutcome::Written {
                effect_id: write.effect_id.clone(),
            }),
            Landed::Pending { .. } | Landed::Lost | Landed::Refused => {
                Ok(WriteOutcome::Pending {
                    effect_id: write.effect_id.clone(),
                })
            }
        }
    }

    fn lease(&mut self, op: &LeaseOp) -> Result<LeaseOutcome> {
        // A disconnected host may not take a new lease (151).
        let (object, holder) = match op {
            LeaseOp::Take { object, holder }
            | LeaseOp::Renew { object, holder }
            | LeaseOp::Release { object, holder } => (object.clone(), holder.clone()),
            LeaseOp::Mark { object, .. } => (object.clone(), String::new()),
        };
        let reference = layout::lease_ref(&object);
        let remote = layout::lease_remote(&object);
        let expected = objects::rev(&self.odb, &remote)?.unwrap_or_else(|| git::ZERO.to_string());
        let held = self.lease_record(&object)?;

        match op {
            LeaseOp::Take { .. } => {
                if self.disconnected {
                    bail!("a host that cannot reach the store takes no lease (151)");
                }
                if let Some(h) = &held {
                    if !h.holder.is_empty() && h.holder != holder && !self.lease_expired(h) {
                        return Ok(LeaseOutcome::HeldByAnother(h.clone()));
                    }
                    // Taking a lease this host already holds is renewing it,
                    // and a renewal that is not due changes nothing, so it is
                    // not a write: a host asks for what it holds on every pass
                    // over the object, and the branch moves once a minute
                    // (127, 128, 150, `git-only.yaml` records.leases).
                    if h.holder == holder && self.now - h.renewed_at < RENEW_EVERY {
                        return Ok(LeaseOutcome::Held(h.clone()));
                    }
                }
                let record = LeaseRecord {
                    object: object.clone(),
                    holder: holder.clone(),
                    taken_at: held
                        .as_ref()
                        .filter(|h| h.holder == holder)
                        .map(|h| h.taken_at)
                        .unwrap_or(self.now),
                    renewed_at: self.now,
                    // Taking is what the machine reads as held; the machine
                    // says so itself on the next tick.
                    state: held
                        .as_ref()
                        .map(|h| h.state.clone())
                        .unwrap_or_else(flywheel_atoms::traits::free),
                };
                let landed =
                    self.put_orphan(&reference, &records::lease_to_record(&record), &expected)?;
                if landed {
                    Ok(LeaseOutcome::Held(record))
                } else {
                    // A rejected push means another host holds it; the loser
                    // fetches and reads who does (134, 162).
                    self.reread_after_rejection = true;
                    match self.lease_record(&object)? {
                        Some(h) => Ok(LeaseOutcome::HeldByAnother(h)),
                        None => Err(anyhow!("the lease on {object} was refused and holds nobody")),
                    }
                }
            }
            LeaseOp::Renew { .. } => {
                let Some(h) = held else {
                    bail!("no lease on {object} to renew")
                };
                if h.holder != holder {
                    return Ok(LeaseOutcome::HeldByAnother(h));
                }
                if self.disconnected {
                    // Renewals push first on reconnect (165, D4a).
                    return Ok(LeaseOutcome::Held(h));
                }
                // A lease is renewed on its own cadence while its holder
                // works, never once per pass over the object it covers: a
                // renewal that is not due is not a write (127, 128, 150,
                // `git-only.yaml` records.leases).
                if self.now - h.renewed_at < RENEW_EVERY {
                    return Ok(LeaseOutcome::Held(h));
                }
                let record = LeaseRecord {
                    renewed_at: self.now,
                    ..h
                };
                self.lease_renewals += 1;
                self.put_orphan(&reference, &records::lease_to_record(&record), &expected)?;
                Ok(LeaseOutcome::Held(record))
            }
            LeaseOp::Release { .. } => {
                match &held {
                    Some(h) if !h.holder.is_empty() && h.holder != holder => {
                        Ok(LeaseOutcome::HeldByAnother(h.clone()))
                    }
                    _ => {
                        objects::delete_ref(&self.odb, &reference).ok();
                        self.pushes += 1;
                        git::delete_expecting(&self.repo, &reference, &expected)?;
                        Ok(LeaseOutcome::Released)
                    }
                }
            }
            // What the lease machine made of it, kept with the lease (128).
            LeaseOp::Mark { state, .. } => {
                let record = LeaseRecord {
                    object: object.clone(),
                    holder: held.as_ref().map(|h| h.holder.clone()).unwrap_or_default(),
                    taken_at: held.as_ref().map(|h| h.taken_at).unwrap_or(self.now),
                    renewed_at: held.as_ref().map(|h| h.renewed_at).unwrap_or(self.now),
                    state: state.clone(),
                };
                self.put_orphan(&reference, &records::lease_to_record(&record), &expected)?;
                Ok(LeaseOutcome::Held(record))
            }
        }
    }

    fn notify(&self, since: &ReadPoint) -> Result<Notice> {
        // After a fetch, `git diff --name-only <old>..<new>` names the object
        // files that moved, so a host re-reads only those (130, 166).
        let mut objects: Vec<String> = objects::changed_paths(&self.odb, &since.mark, self.at())?
            .iter()
            .filter_map(|p| layout::touched(p).map(str::to_string))
            .collect();
        objects.sort();
        objects.dedup();
        Ok(Notice {
            as_of: self.as_of(),
            objects,
        })
    }

    fn present(&mut self, presentation: &Presentation) -> Result<()> {
        // A disconnected host delivers to no sink (151). The sinks themselves
        // are `surfaces.yaml`'s, which group 7 binds; the mark is a record.
        if self.disconnected {
            return Ok(());
        }
        let _ = presentation;
        Ok(())
    }

    fn receive(&mut self, response: &Response) -> Result<Received> {
        // The response file is named by its delivery id, so a second delivery
        // writes the same file and is a no-op (137).
        let path = layout::response(&response.id);
        if self.read_file(&path)?.is_some() {
            return Ok(Received::AlreadyApplied {
                id: response.id.clone(),
            });
        }
        self.on_fetched_head()?;
        self.write_file(&path,
            &rec::write(std::slice::from_ref(&records::response_to_record(response))),
        )?;
        // Written before the transition it causes fires (129, 153).
        self.commit_and_push(&format!(
            "response {} by {}",
            response.id, response.given_by
        ))?;
        Ok(Received::Recorded {
            id: response.id.clone(),
        })
    }
}

impl GitStore {
    /// Commits on `main` this host has made that the git host does not hold.
    pub fn unpushed(&self) -> Result<Vec<String>> {
        let Some(head) = objects::rev(&self.odb, "HEAD")? else {
            return Ok(vec![]);
        };
        if self.fetched.is_empty() || self.fetched == git::ZERO {
            return Ok(vec![head]);
        }
        objects::commits_between(&self.odb, &self.fetched, &head)
    }

    /// Push the commits made while there was no route. A rejection is replayed
    /// and retried like any other write (165, D4a).
    pub fn push_unpushed(&mut self) -> Result<bool> {
        if self.disconnected || self.unpushed()?.is_empty() {
            return Ok(false);
        }
        // The fetch that starts a reconnect: what landed while the route was
        // down is not this host's to discard (165, D4a). The checkout keeps
        // this host's own commits, which are ahead of it.
        let ours = objects::rev(&self.odb, "HEAD")?.unwrap_or_default();
        self.fetch()?;
        let mut ours = ours;
        for _ in 0..RETRIES {
            let expected = if self.fetched.is_empty() {
                git::ZERO.to_string()
            } else {
                self.fetched.clone()
            };
            // Its own commits go on top of what it finds, and the push that
            // follows carries both (134, 165, D4a).
            if !self.fetched.is_empty()
                && self.fetched != git::ZERO
                && !objects::reaches(&self.odb, &self.fetched, &ours)?
            {
                let onto = self.fetched.clone();
                if !self.replay_onto(&ours, &onto)? {
                    // A conflict on content is a loss: the other commit stands
                    // and this host re-reads (3, 164, I15).
                    self.hard_reset(&onto)?;
                    return Ok(false);
                }
                ours = self.commit_worktree(&format!("{} replayed", &ours[..7.min(ours.len())]))?;
            }
            if self.push(layout::MAIN, &expected)? {
                self.landed(&ours);
                self.pending.clear();
                return Ok(true);
            }
            self.fetch()?;
            self.reread_after_rejection = true;
            let onto = self.fetched.clone();
            if !self.replay_onto(&ours, &onto)? {
                self.hard_reset(&onto)?;
                return Ok(false);
            }
            ours = self.commit_worktree(&format!("{} replayed", &ours[..7.min(ours.len())]))?;
        }
        Ok(false)
    }

    /// The bound the profile states for notification: one `git ls-remote`
    /// round trip every 30 seconds is what a laptop runs (130, 166, D6).
    pub fn poll_bound(&self) -> Duration {
        Duration::seconds(30)
    }

    /// The path of the checkout, for a test that wants to look.
    pub fn root(&self) -> &PathBuf {
        &self.repo.dir
    }

    /// Set the world's answers, which this profile inherits (B.3).
    pub fn set_given(&mut self, object: &str, name: &str, value: Value) {
        self.given
            .entry(object.to_string())
            .or_default()
            .insert(name.to_string(), value);
    }
}

impl GitStore {
    /// Append entries to today's run record and commit them: what this host did
    /// and why, readable with no host running (79-82, 167).
    pub fn append_run(&mut self, entries: &[records::RunEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let path = layout::run_record(&self.host, &self.now.format("%Y-%m-%d").to_string());
        let mut held = self
            .read_file(&path)?
            .map(|text| rec::parse(&text))
            .unwrap_or_default();
        held.extend(entries.iter().map(records::run_to_record));
        self.on_fetched_head()?;
        self.write_file(&path, &rec::write(&held))?;
        self.commit_and_push(&format!(
            "run record: {} {} on {}\n\nreason: what this host did and why (79)",
            entries.len(),
            match entries.len() {
                1 => "entry",
                _ => "entries",
            },
            self.host
        ))?;
        Ok(())
    }

    /// Today's run record, as written.
    pub fn run_record(&self) -> Result<Vec<records::RunEntry>> {
        let path = layout::run_record(&self.host, &self.now.format("%Y-%m-%d").to_string());
        let Some(text) = self.read_file(&path)? else {
            return Ok(vec![]);
        };
        rec::parse(&text).iter().map(records::run_from_record).collect()
    }

    /// Every host's run record on the shared line, in the order it was written.
    ///
    /// The record is what a host says it did and why, and it is readable with no
    /// host running (79–82, 167). A reader that is not a host — the conformance
    /// runner under `--hosts real` — reads what every host did from here and
    /// from nowhere else (D15).
    pub fn all_run_records(&self) -> Result<Vec<records::RunEntry>> {
        let mut out: Vec<records::RunEntry> = self
            .run_records_by_file()?
            .into_iter()
            .flat_map(|(_, entries)| entries)
            .collect();
        out.sort_by(|a, b| a.at.cmp(&b.at));
        Ok(out)
    }

    /// The same, file by file. One host appends to its own file and never to
    /// another's, so a reader that wants to know what is new since it last
    /// looked reads each file on its own: the whole set is reordered by time as
    /// hosts come and go, and one file never is.
    pub fn run_records_by_file(&self) -> Result<Vec<(String, Vec<records::RunEntry>)>> {
        if self.fetched.is_empty() || self.fetched == git::ZERO {
            return Ok(vec![]);
        }
        let mut out = Vec::new();
        for path in self.tree("runs")? {
            let Some(text) = self.read_file(&path)? else {
                continue;
            };
            let entries: Result<Vec<records::RunEntry>> =
                rec::parse(&text).iter().map(records::run_from_record).collect();
            out.push((path, entries?));
        }
        Ok(out)
    }

    /// Commit the status projection on the shared line, stating the commit and
    /// time it is as of. Only the rail's lease holder writes it (D12, 132, 145).
    pub fn commit_status(&mut self, body: &str) -> Result<()> {
        // The projection is rewritten whenever what it projects moved, and the
        // point it is as of is not something it projects: a body that differs
        // only in its stamp is the body that is already there, and writing what
        // is already there is not a write (77, 78, 127, 131, 142, D12).
        let held = self.read_file(layout::STATUS)?;
        if held.as_deref().map(without_the_stamp) == Some(without_the_stamp(body)) {
            return Ok(());
        }
        self.on_fetched_head()?;
        self.write_file(layout::STATUS, body)?;
        self.commit_and_push(&format!(
            "{}\n\nreason: the status view is rewritten from its source on every tick (77, 132)",
            layout::STATUS
        ))?;
        Ok(())
    }

    /// The newest commit on the shared line that wrote a state record. The
    /// projection is as-of the state, and the commits after it are the
    /// projection's own and the run record's (145, 167).
    pub fn newest_state_write(&self) -> Result<Option<String>> {
        if self.fetched.is_empty() || self.fetched == git::ZERO {
            return Ok(None);
        }
        Ok(objects::newest_touching(&self.odb, &self.fetched, "objects")?.map(|(sha, _)| sha))
    }

    /// Whether one commit is in another's history.
    pub fn is_ancestor(&self, older: &str, newer: &str) -> bool {
        objects::reaches(&self.odb, older, newer).unwrap_or(false)
    }

    /// The status file as it stands on the shared line: what the operator reads
    /// with no host running (145, S20).
    pub fn committed_status(&self) -> Result<Option<String>> {
        self.read_file(layout::STATUS)
    }
}

impl flywheel_engine::runtime::EvidenceSource for GitStore {
    fn evidence(&self, object: &str, _region: &str, name: &str) -> Option<Value> {
        // What this process was told first, then what the shared line holds:
        // a host that has just been told something has not yet committed it,
        // and both are the same world (B.3).
        for held in [&self.given, &self.shared] {
            if let Some(value) = held
                .get(object)
                .and_then(|m| m.get(name))
                .or_else(|| held.get("*").and_then(|m| m.get(name)))
            {
                return Some(value.clone());
            }
        }
        None
    }
}

impl GitStore {
    /// The operator's own commit on an object's file: written by a person, on
    /// the shared line, outside anything the machinery put there. It carries no
    /// sequence of its own, which is how the next fetch tells it from a write
    /// the machinery made (3, 159, 164).
    pub fn commit_as_operator(&mut self, object: &Object, delivery: &str, by: &str) -> Result<()> {
        self.on_fetched_head()?;
        self.write_file(&layout::object(&object.id),
            &envelope::write_all(std::slice::from_ref(object)),
        )?;
        self.commit_and_push(&format!("{OPERATOR} {delivery} by {by}"))?;
        Ok(())
    }

    /// The delivery id of the operator's commit that last touched an object's
    /// file, where the last commit on it was one. Read from the fetched
    /// history, so every host derives the same id from the same fetch (164).
    pub fn operator_commit_on(&self, id: &str) -> Result<Option<String>> {
        if self.fetched.is_empty() || self.fetched == git::ZERO {
            return Ok(None);
        }
        let Some((_, subject)) =
            objects::newest_touching(&self.odb, &self.fetched, &layout::object(id))?
        else {
            return Ok(None);
        };
        let subject = subject.trim();
        let Some(rest) = subject.strip_prefix(OPERATOR) else {
            return Ok(None);
        };
        Ok(rest.split_whitespace().next().map(String::from))
    }

    /// Put many described objects on the shared line in one commit: a described
    /// state is one write, not one per object (94, D15).
    pub fn seed_objects(&mut self, objects: &[Object]) -> Result<()> {
        if objects.is_empty() {
            return Ok(());
        }
        let was = self.in_tick;
        self.in_tick = true;
        let outcome = (|| -> Result<()> {
            for object in objects {
                self.seed_object(object)?;
            }
            Ok(())
        })();
        self.in_tick = was;
        outcome?;
        self.flush()?;
        Ok(())
    }

    /// Put a described state in place: the envelope exactly as given, sequence
    /// and all. Seeding is not a write the machinery made, so it takes no
    /// sequence of its own and the first tick's write is the object's first
    /// (94, D15).
    pub fn seed_object(&mut self, object: &Object) -> Result<()> {
        self.on_fetched_head()?;
        self.write_file(&layout::object(&object.id),
            &envelope::write_all(std::slice::from_ref(object)),
        )?;
        self.commit_and_push(&format!("{} seeded\n\nreason: the scenario's given state", object.id))?;
        Ok(())
    }
}

/// Make a state repository and a host's checkout of it: what `flywheel init`
/// and `flywheel host join` leave behind, and what a sandbox run starts from
/// (204, 205).
pub fn sandbox(base: &Path, host: &str, now: DateTime<Utc>) -> Result<GitStore> {
    let remote = base.join("flywheel-state.git");
    if !remote.exists() {
        let bare = git::init_bare(&remote)?;
        // An empty repository has no `main` until something lands on it; the
        // first host's first write makes it.
        let _ = bare;
    }
    let root = base.join(host);
    let mut store = GitStore::open(&remote, &root, host, now)
        .with_context(|| format!("opening the state repository at {}", remote.display()))?;
    // The shared line exists from the first commit, so every host starts from
    // the same base.
    if store.fetched.is_empty() || store.fetched == git::ZERO {
        git::stage(&store.repo, ".flywheel", "state\n")?;
        let _ = store.commit_and_push("the state repository's shared line");
    }
    Ok(store)
}

/// Every file under a prefix of the checkout, by path, `.git` left out. What
/// `list` reads, with no process on the read path (131, 169).
fn under(root: &Path, prefix: &str, into: &mut Vec<String>) {
    let dir = match prefix.is_empty() {
        true => root.to_path_buf(),
        false => root.join(prefix),
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" {
            continue;
        }
        let path = match prefix.is_empty() {
            true => name,
            false => format!("{prefix}/{name}"),
        };
        match entry.path().is_dir() {
            true => under(root, &path, into),
            false => into.push(path),
        }
    }
}

/// A status body without the line stating the point it is as of, which is what
/// tells a projection that moved from one that only says when it was read
/// (77, 145).
fn without_the_stamp(body: &str) -> String {
    body.lines()
        .filter(|line| !line.contains("as-of") && !line.contains("as of"))
        .collect::<Vec<_>>()
        .join("\n")
}
