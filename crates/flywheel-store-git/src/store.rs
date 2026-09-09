//! `GitStore`: the eight operations of 125 and the six record operations under
//! them, over the state repository.

use crate::git::{self, Repo};
use crate::layout;
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
use std::path::{Path, PathBuf};

/// What the operator's own commit says of itself in its subject line, so every
/// host reads the same delivery id out of the same fetched history (164).
pub const OPERATOR: &str = "operator commit";

/// How many times a rejected push is rebased and retried before the host
/// reports and re-reads (`git-only.yaml records.put`).
pub const RETRIES: usize = 3;

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
    /// What this run has done, for the observations a scenario asserts.
    pub writes_attempted: usize,
    pub writes_rejected: usize,
    pub reread_after_rejection: bool,
    pub reads_since_notify: usize,
    /// Effect ids written locally but not yet landed (161, D4a).
    pub pending: Vec<String>,
}

impl GitStore {
    /// Open a host's checkout of a state repository.
    pub fn open(remote: &Path, root: &Path, host: &str, now: DateTime<Utc>) -> Result<GitStore> {
        let repo = if root.join(".git").is_dir() {
            Repo::at(root)
        } else {
            git::clone(remote, root)?
        };
        let mut store = GitStore {
            repo,
            host: host.to_string(),
            fetched: String::new(),
            now,
            lease_expiry: Duration::hours(24),
            lease_stale: Duration::minutes(5),
            disconnected: false,
            given: BTreeMap::new(),
            writes_attempted: 0,
            writes_rejected: 0,
            reread_after_rejection: false,
            reads_since_notify: 0,
            pending: vec![],
        };
        store.fetch()?;
        Ok(store)
    }

    /// Fetch and integrate the shared line. Every tick does this first, so no
    /// host decides on a read older than the bound (165, D7).
    pub fn fetch(&mut self) -> Result<String> {
        if !self.disconnected {
            // A fetch that cannot reach the git host is not an error: the host
            // keeps working what it already holds (151).
            let _ = self.repo.run(&["fetch", "--quiet", "origin"]);
        }
        self.fetched = git::rev(&self.repo, layout::ORIGIN_MAIN)?
            .or(git::rev(&self.repo, "HEAD")?)
            .unwrap_or_else(|| git::ZERO.to_string());
        Ok(self.fetched.clone())
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

    fn read_file(&self, path: &str) -> Result<Option<String>> {
        if self.fetched.is_empty() || self.fetched == git::ZERO {
            return Ok(None);
        }
        git::show(&self.repo, &self.fetched, path)
    }

    /// Put the checkout on a local branch at the fetched head, ready to commit.
    fn on_fetched_head(&self) -> Result<()> {
        if self.fetched.is_empty() || self.fetched == git::ZERO {
            return Ok(());
        }
        // Only move to the fetched head when nothing local is ahead of it: a
        // disconnected host's own commits are intentions it keeps (161, D4a).
        let head = git::rev(&self.repo, "HEAD")?;
        if head.as_deref() == Some(self.fetched.as_str()) {
            return Ok(());
        }
        let behind = self
            .repo
            .run(&["merge-base", "--is-ancestor", "HEAD", &self.fetched])?
            .ok;
        if behind {
            self.repo.git(&["reset", "--hard", "--quiet", &self.fetched])?;
        }
        Ok(())
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
        let sha = git::commit(&self.repo, message, self.now)?;
        self.writes_attempted += 1;
        if self.disconnected {
            // A local commit is an intention, not a fact (161).
            return Ok(Landed::Pending { sha });
        }
        for attempt in 0..RETRIES {
            let expected = if self.fetched.is_empty() {
                git::ZERO.to_string()
            } else {
                self.fetched.clone()
            };
            if git::push_expecting(&self.repo, "HEAD", layout::MAIN, &expected)? {
                self.fetch()?;
                return Ok(Landed::Written { sha });
            }
            self.writes_rejected += 1;
            // The loser learns that it lost and reads again before deciding
            // anything (134).
            self.fetch()?;
            self.reread_after_rejection = true;
            let onto = self.fetched.clone();
            if let Some((id, base_seq)) = guard {
                let held = self.get(id)?.map(|o| o.seq).unwrap_or(0);
                if held != base_seq {
                    self.repo.git(&["reset", "--hard", "--quiet", &onto])?;
                    return Ok(Landed::Lost);
                }
            }
            if attempt + 1 == RETRIES {
                break;
            }
            if !self.repo.run(&["rebase", "--quiet", &onto])?.ok {
                let _ = self.repo.run(&["rebase", "--abort"]);
                // A rebase that conflicts on content is a loss: the host
                // discards its local commit and takes what it found (3, 164,
                // I15, S19).
                self.repo.git(&["reset", "--hard", "--quiet", &onto])?;
                return Ok(Landed::Lost);
            }
        }
        Ok(Landed::Refused)
    }

    /// The lease branches, by object.
    fn lease_record(&self, object: &str) -> Result<Option<LeaseRecord>> {
        let reference = layout::lease_ref(object);
        let Some(sha) = git::rev(&self.repo, &format!("refs/remotes/origin/lease/{object}"))?
            .or(git::rev(&self.repo, &reference)?)
        else {
            return Ok(None);
        };
        let Some(text) = git::show(&self.repo, &sha, layout::RECORD)? else {
            return Ok(None);
        };
        let parsed = rec::parse(&text);
        let Some(record) = parsed.first() else {
            return Ok(None);
        };
        Ok(Some(records::lease_from_record(record)?))
    }

    /// Write one orphan commit whose tree is the record, and push the branch
    /// with expected-old. The push *is* the compare-and-swap (D5).
    fn put_orphan(&self, reference: &str, record: &rec::Record, expected: &str) -> Result<bool> {
        // One orphan commit whose tree is the record. Built through a
        // temporary index so the host's checkout is untouched: a lease renewal
        // must not disturb the work in the tree (D5).
        let text = rec::write(std::slice::from_ref(record));
        let scratch = self.repo.dir.join(".git").join("flywheel-orphan.rec");
        std::fs::write(&scratch, &text)?;
        let hash = self
            .repo
            .git(&["hash-object", "-w", &scratch.to_string_lossy()])?
            .trim()
            .to_string();
        let _ = std::fs::remove_file(&scratch);

        let index = self.repo.dir.join(".git").join("flywheel-orphan.index");
        let _ = std::fs::remove_file(&index);
        let with_index = |args: &[&str]| -> Result<String> {
            let out = std::process::Command::new("git")
                .current_dir(&self.repo.dir)
                .env("GIT_INDEX_FILE", &index)
                .env("GIT_AUTHOR_NAME", "flywheel")
                .env("GIT_AUTHOR_EMAIL", "flywheel@localhost")
                .env("GIT_COMMITTER_NAME", "flywheel")
                .env("GIT_COMMITTER_EMAIL", "flywheel@localhost")
                .args(args)
                .output()?;
            if !out.status.success() {
                bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr));
            }
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        };
        with_index(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("100644,{hash},{}", layout::RECORD),
        ])?;
        let tree = with_index(&["write-tree"])?;
        let _ = std::fs::remove_file(&index);

        let commit = self
            .repo
            .git(&[
                "commit-tree",
                &tree,
                "-m",
                &format!("{reference} at {}", self.now.to_rfc3339()),
            ])?
            .trim()
            .to_string();
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
        git::stage(
            &self.repo,
            &layout::object(id),
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
        git::stage(&self.repo, &path, &existing)?;
        self.commit_and_push(&format!("{id} thread {}", entry.kind))?;
        Ok(())
    }

    fn list_records(&self, scope: &Scope) -> Result<Vec<Object>> {
        let mut out = Vec::new();
        for path in git::ls_tree(&self.repo, self.at(), "objects/")? {
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
        for path in git::ls_tree(&self.repo, self.at(), layout::RESPONSES)? {
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
        let out = self.repo.run(&["for-each-ref", "--format=%(refname)", "refs/remotes/origin/host/"])?;
        let mut hosts = Vec::new();
        for reference in out.stdout.lines() {
            let Some(name) = reference.rsplit('/').next() else { continue };
            let Some(sha) = git::rev(&self.repo, reference)? else { continue };
            let Some(text) = git::show(&self.repo, &sha, layout::RECORD)? else { continue };
            if let Some(record) = rec::parse(&text).first() {
                if let Ok(host) = records::host_from_record(record) {
                    hosts.push(host);
                }
            }
            let _ = name;
        }
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
        let expected = git::rev(&self.repo, &format!("refs/remotes/origin/host/{}", self.host))?
            .unwrap_or_else(|| git::ZERO.to_string());
        let record = records::host_to_record(&HostRecord {
            host: self.host.clone(),
            last_seen: self.now,
            bound,
            intermittent,
        });
        self.put_orphan(&reference, &record, &expected)?;
        let _ = self.repo.run(&["fetch", "--quiet", "origin"]);
        Ok(())
    }
}

// ------------------------------------------------------------ the state store

impl StateStore for GitStore {
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
        // A repeat is found by `git log --grep=<effect id>` on the fetched main
        // before committing, and changes nothing (127).
        if !self.fetched.is_empty()
            && self.fetched != git::ZERO
            && git::log_grep(&self.repo, &self.fetched, &write.effect_id)?
        {
            return Ok(WriteOutcome::AlreadyWritten {
                effect_id: write.effect_id.clone(),
            });
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
        match self.commit_and_push(&message)? {
            Landed::Written { .. } => Ok(WriteOutcome::Written {
                effect_id: write.effect_id.clone(),
            }),
            Landed::Pending { .. } => {
                self.pending.push(write.effect_id.clone());
                Ok(WriteOutcome::Pending {
                    effect_id: write.effect_id.clone(),
                })
            }
            Landed::Lost | Landed::Refused => Ok(WriteOutcome::Pending {
                effect_id: write.effect_id.clone(),
            }),
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
        let remote = format!("refs/remotes/origin/lease/{object}");
        let expected = git::rev(&self.repo, &remote)?.unwrap_or_else(|| git::ZERO.to_string());
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
                let _ = self.repo.run(&["fetch", "--quiet", "origin"]);
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
                let record = LeaseRecord {
                    renewed_at: self.now,
                    ..h
                };
                self.put_orphan(&reference, &records::lease_to_record(&record), &expected)?;
                let _ = self.repo.run(&["fetch", "--quiet", "origin"]);
                Ok(LeaseOutcome::Held(record))
            }
            LeaseOp::Release { .. } => {
                match &held {
                    Some(h) if !h.holder.is_empty() && h.holder != holder => {
                        Ok(LeaseOutcome::HeldByAnother(h.clone()))
                    }
                    _ => {
                        git::delete_expecting(&self.repo, &reference, &expected)?;
                        let _ = self.repo.run(&["fetch", "--quiet", "--prune", "origin"]);
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
                let _ = self.repo.run(&["fetch", "--quiet", "origin"]);
                Ok(LeaseOutcome::Held(record))
            }
        }
    }

    fn notify(&self, since: &ReadPoint) -> Result<Notice> {
        // After a fetch, `git diff --name-only <old>..<new>` names the object
        // files that moved, so a host re-reads only those (130, 166).
        let mut objects: Vec<String> = git::diff_names(&self.repo, &since.mark, self.at())?
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
        git::stage(
            &self.repo,
            &path,
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
        let Some(head) = git::rev(&self.repo, "HEAD")? else {
            return Ok(vec![]);
        };
        if self.fetched.is_empty() || self.fetched == git::ZERO {
            return Ok(vec![head]);
        }
        let range = format!("{}..HEAD", self.fetched);
        let out = self.repo.run(&["rev-list", &range])?;
        Ok(out
            .stdout
            .lines()
            .map(str::to_string)
            .filter(|l| !l.is_empty())
            .collect())
    }

    /// Push the commits made while there was no route. A rejection is rebased
    /// and retried like any other write (165, D4a).
    pub fn push_unpushed(&mut self) -> Result<bool> {
        if self.disconnected || self.unpushed()?.is_empty() {
            return Ok(false);
        }
        self.fetch()?;
        for _ in 0..RETRIES {
            let expected = if self.fetched.is_empty() {
                git::ZERO.to_string()
            } else {
                self.fetched.clone()
            };
            if git::push_expecting(&self.repo, "HEAD", layout::MAIN, &expected)? {
                self.fetch()?;
                self.pending.clear();
                return Ok(true);
            }
            self.fetch()?;
            self.reread_after_rejection = true;
            let onto = self.fetched.clone();
            if !self.repo.run(&["rebase", "--quiet", &onto])?.ok {
                let _ = self.repo.run(&["rebase", "--abort"]);
                self.repo.git(&["reset", "--hard", "--quiet", &onto])?;
                return Ok(false);
            }
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
        git::stage(&self.repo, &path, &rec::write(&held))?;
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

    /// Commit the status projection on the shared line, stating the commit and
    /// time it is as of. Only the rail's lease holder writes it (D12, 132, 145).
    pub fn commit_status(&mut self, body: &str) -> Result<()> {
        if self.read_file(layout::STATUS)?.as_deref() == Some(body) {
            return Ok(());
        }
        self.on_fetched_head()?;
        git::stage(&self.repo, layout::STATUS, body)?;
        self.commit_and_push(&format!(
            "{}\n\nreason: the status view is rewritten from its source on every tick (77, 132)",
            layout::STATUS
        ))?;
        Ok(())
    }

    /// The status file as it stands on the shared line: what the operator reads
    /// with no host running (145, S20).
    pub fn committed_status(&self) -> Result<Option<String>> {
        self.read_file(layout::STATUS)
    }
}

impl flywheel_engine::runtime::EvidenceSource for GitStore {
    fn evidence(&self, object: &str, _region: &str, name: &str) -> Option<Value> {
        self.given
            .get(object)
            .and_then(|m| m.get(name))
            .or_else(|| self.given.get("*").and_then(|m| m.get(name)))
            .cloned()
    }
}

impl GitStore {
    /// The operator's own commit on an object's file: written by a person, on
    /// the shared line, outside anything the machinery put there. It carries no
    /// sequence of its own, which is how the next fetch tells it from a write
    /// the machinery made (3, 159, 164).
    pub fn commit_as_operator(&mut self, object: &Object, delivery: &str, by: &str) -> Result<()> {
        self.on_fetched_head()?;
        git::stage(
            &self.repo,
            &layout::object(&object.id),
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
        let subject = self.repo.git(&[
            "log",
            "-1",
            "--format=%s",
            &self.fetched,
            "--",
            &layout::object(id),
        ])?;
        let subject = subject.trim();
        let Some(rest) = subject.strip_prefix(OPERATOR) else {
            return Ok(None);
        };
        Ok(rest.split_whitespace().next().map(String::from))
    }

    /// Put a described state in place: the envelope exactly as given, sequence
    /// and all. Seeding is not a write the machinery made, so it takes no
    /// sequence of its own and the first tick's write is the object's first
    /// (94, D15).
    pub fn seed_object(&mut self, object: &Object) -> Result<()> {
        self.on_fetched_head()?;
        git::stage(
            &self.repo,
            &layout::object(&object.id),
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
