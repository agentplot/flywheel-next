//! Running git, with explicit arguments and nothing of the operator's own
//! configuration (183).
//!
//! This is the world's git, not the state store's: the repositories a host
//! clones and reads, which is a different door from the one durable state goes
//! through (125, D8). The two do not share a module for that reason.

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Said {
    pub ok: bool,
    pub out: String,
    pub err: String,
}

impl Said {
    pub fn text(self) -> Result<String> {
        if self.ok {
            Ok(self.out)
        } else {
            Err(anyhow!(self.err.trim().to_string()))
        }
    }
}

// Every process this crate has spawned at any repository.
//
// `host.yaml`'s Tools header binds `gix` for reads, so reading the blueprints
// spawns nothing; this is the count that holds the crate to it, and the one a
// tick's cost is taken as a difference of (169, audit 8, audit 9). A host makes
// a `Repo` per read, so the count is the crate's and not a repository's.
// Counted per thread, because that is what a tick is: `Host::tick` runs the
// world's reads and writes on the thread it was called on, so this counts the
// tick's own processes and no other test's or host's.
thread_local! {
    static SPAWNED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// How many processes this crate has spawned on this thread.
pub fn spawned() -> u32 {
    SPAWNED.with(|n| n.get())
}

/// A repository on disk, bare or checked out.
#[derive(Debug, Clone, Default)]
pub struct Repo {
    pub dir: PathBuf,
}

impl Repo {
    pub fn at(dir: impl Into<PathBuf>) -> Repo {
        Repo { dir: dir.into() }
    }

    pub fn run(&self, args: &[&str]) -> Result<Said> {
        SPAWNED.with(|n| n.set(n.get() + 1));
        let out = Command::new("git")
            .current_dir(&self.dir)
            .args(args)
            .env("GIT_AUTHOR_NAME", "flywheel")
            .env("GIT_AUTHOR_EMAIL", "flywheel@localhost")
            .env("GIT_COMMITTER_NAME", "flywheel")
            .env("GIT_COMMITTER_EMAIL", "flywheel@localhost")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("HOME", &self.dir)
            .output()
            .with_context(|| format!("running git {}", args.join(" ")))?;
        Ok(Said {
            ok: out.status.success(),
            out: String::from_utf8_lossy(&out.stdout).to_string(),
            err: String::from_utf8_lossy(&out.stderr).to_string(),
        })
    }

    pub fn git(&self, args: &[&str]) -> Result<String> {
        self.run(args)?
            .text()
            .with_context(|| format!("git {}", args.join(" ")))
    }

    pub fn is_bare(&self) -> bool {
        self.dir.join("HEAD").is_file() && !self.dir.join(".git").exists()
    }

    pub fn exists(&self) -> bool {
        self.dir.join(".git").is_dir() || self.is_bare()
    }
}

/// A bare repository with `main` as its shared line: what a repository on the
/// git host is, as far as a host is concerned.
pub fn init_bare(path: &Path) -> Result<Repo> {
    std::fs::create_dir_all(path)?;
    let repo = Repo::at(path);
    repo.git(&["init", "--bare", "--initial-branch=main", "--quiet", "."])?;
    Ok(repo)
}

/// Clone bare under a root: what a host keeps, one per repository (205).
pub fn clone_bare(remote: &Path, into: &Path) -> Result<Repo> {
    if let Some(parent) = into.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let parent = Repo::at(into.parent().unwrap_or(Path::new(".")));
    parent.git(&[
        "clone",
        "--bare",
        "--quiet",
        &remote.to_string_lossy(),
        &into.to_string_lossy(),
    ])?;
    Ok(Repo::at(into))
}

/// One checkout of a shared line, for the machinery's own merges (205). Places
/// are worktrees made later; this is not one.
pub fn checkout_line(bare: &Path, into: &Path, line: &str) -> Result<Repo> {
    if into.join(".git").is_dir() {
        return Ok(Repo::at(into));
    }
    if let Some(parent) = into.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let parent = Repo::at(into.parent().unwrap_or(Path::new(".")));
    parent.git(&[
        "clone",
        "--quiet",
        &bare.to_string_lossy(),
        &into.to_string_lossy(),
    ])?;
    let repo = Repo::at(into);
    if repo.run(&["rev-parse", "--verify", "HEAD"])?.ok {
        repo.run(&["checkout", "--quiet", line]).ok();
    } else {
        repo.git(&["checkout", "--quiet", "-b", line])?;
    }
    Ok(repo)
}

/// One file at a repository's shared line, or none. Read from the bare
/// repository, so no checkout is needed to read the world.
///
/// In process, through `gix`, as `host.yaml`'s Tools header binds it: a tick
/// that reads N signals used to fork N `git show` children, which is the cost
/// the state store had already stopped paying (audit 9).
pub fn show(repo: &Repo, line: &str, path: &str) -> Result<Option<String>> {
    let Some(opened) = opened(repo) else {
        return Ok(None);
    };
    let Some(mut tree) = tree_at(&opened, line)? else {
        return Ok(None);
    };
    let Some(entry) = tree.peel_to_entry_by_path(path)? else {
        return Ok(None);
    };
    if entry.mode().is_tree() {
        return Ok(None);
    }
    let object = entry.object()?;
    Ok(Some(String::from_utf8_lossy(&object.data).to_string()))
}

/// Every path at a repository's shared line under a prefix, in process.
///
/// A prefix is a path the caller already spells with its trailing slash, and an
/// empty one is the whole tree — which is `git ls-tree -r --name-only` without
/// the spawn.
pub fn ls_tree(repo: &Repo, line: &str, prefix: &str) -> Result<Vec<String>> {
    let Some(opened) = opened(repo) else {
        return Ok(vec![]);
    };
    let Some(tree) = tree_at(&opened, line)? else {
        return Ok(vec![]);
    };
    let mut out = Vec::new();
    walk(&tree, "", &mut out)?;
    if prefix.is_empty() {
        return Ok(out);
    }
    let under = prefix.trim_end_matches('/');
    Ok(out
        .into_iter()
        .filter(|path| path == under || path.starts_with(&format!("{under}/")))
        .collect())
}

/// What a checkout's HEAD names, and whether it holds a file at a path.
pub struct Head {
    pub revision: String,
    pub holds: bool,
}

/// The commit HEAD names in the repository `dir` is in, and whether that
/// commit holds a file at `path`, in process: what an offer made in a place
/// names as the revision its document is read at (62). None where the
/// directory is in no repository or its HEAD names no commit yet.
pub fn head_holding(dir: &Path, path: &str) -> Result<Option<Head>> {
    let Ok(repo) = gix::discover(dir) else {
        return Ok(None);
    };
    let Ok(id) = repo.head_id() else {
        return Ok(None);
    };
    let id = id.detach();
    let mut tree = repo.find_object(id)?.peel_to_tree()?;
    let holds = tree
        .peel_to_entry_by_path(path.trim_start_matches("./"))?
        .is_some_and(|entry| !entry.mode().is_tree());
    Ok(Some(Head { revision: id.to_string(), holds }))
}

/// The repository, or none where the directory is not one — a host that has not
/// cloned yet reads an empty world rather than failing.
fn opened(repo: &Repo) -> Option<gix::Repository> {
    gix::open(&repo.dir).ok()
}

/// The tree at a line, or none where the repository does not carry it.
fn tree_at<'r>(repo: &'r gix::Repository, line: &str) -> Result<Option<gix::Tree<'r>>> {
    let Some(id) = commit_of(repo, line) else {
        return Ok(None);
    };
    let Ok(object) = repo.find_object(id) else {
        return Ok(None);
    };
    Ok(Some(object.peel_to_tree()?))
}

/// The commit a line names: the head itself where the repository carries it,
/// the remote-tracking head where it is a clone that has not checked it out.
fn commit_of(repo: &gix::Repository, line: &str) -> Option<gix::ObjectId> {
    for name in [
        format!("refs/heads/{line}"),
        format!("refs/remotes/origin/{line}"),
    ] {
        if let Ok(mut found) = repo.find_reference(&name) {
            if let Ok(id) = found.peel_to_id() {
                return Some(id.detach());
            }
        }
    }
    repo.rev_parse_single(line).ok().map(|id| id.detach())
}

fn walk(tree: &gix::Tree<'_>, under: &str, into: &mut Vec<String>) -> Result<()> {
    for entry in tree.iter() {
        let entry = entry?;
        let name = entry.filename().to_string();
        let path = match under.is_empty() {
            true => name,
            false => format!("{under}/{name}"),
        };
        if entry.mode().is_tree() {
            let inner = entry.object()?.into_tree();
            walk(&inner, &path, into)?;
        } else {
            into.push(path);
        }
    }
    Ok(())
}

/// Write a file into a checkout, commit it and push the shared line.
pub fn commit_file(repo: &Repo, line: &str, path: &str, content: &str, message: &str) -> Result<()> {
    let full = repo.dir.join(path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&full, content)?;
    repo.git(&["add", "--", path])?;
    let said = repo.run(&["commit", "--quiet", "-m", message])?;
    if !said.ok && !said.out.contains("nothing to commit") && !said.err.contains("nothing to commit")
    {
        return Err(anyhow!(said.err.trim().to_string()));
    }
    repo.git(&["push", "--quiet", "origin", line])?;
    Ok(())
}

/// The last `limit` commits reachable from `rev`, newest first (185, S28).
pub fn log(repo: &Repo, rev: &str, limit: usize) -> Result<Vec<flywheel_atoms::CommitRef>> {
    let said = repo.run(&[
        "log",
        "--format=%h%x1f%s%x1f%an%x1f%cI",
        &format!("-n{limit}"),
        rev,
        "--",
    ])?;
    if !said.ok {
        return Ok(vec![]);
    }
    Ok(said
        .text()?
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\x1f');
            Some(flywheel_atoms::CommitRef {
                hash: parts.next()?.to_string(),
                subject: parts.next()?.to_string(),
                author: parts.next()?.to_string(),
                at: parts.next()?.to_string(),
            })
        })
        .collect())
}
