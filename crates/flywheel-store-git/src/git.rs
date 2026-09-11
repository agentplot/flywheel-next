//! Running git. The one place in the workspace that reaches a repository, and
//! the reason the storage-dependency boundary holds (125, task 1.7).
//!
//! The profile names `gix` for reads and the `git` binary for pushes with
//! `--force-with-lease`. Phase 1 uses the binary for both: it binds every name
//! the profile binds, correctness comes before speed (design — Non-Goals), and
//! it keeps the workspace free of a large dependency. Swapping the reads to a
//! library later changes nothing above this file.

use anyhow::{anyhow, bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// One git invocation's result.
#[derive(Debug, Clone)]
pub struct Output {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    /// The output as text, or the error git gave.
    pub fn text(self) -> Result<String> {
        if self.ok {
            Ok(self.stdout)
        } else {
            Err(anyhow!("git failed: {}", self.stderr.trim()))
        }
    }
}

/// A git repository on disk.
///
/// Every process the machinery spawns at a repository goes through `run`, and
/// `spawned` counts them: what a tick costs is a fact the profile states and
/// the store answers for, not a guess (169, `git-only.yaml` cost).
#[derive(Debug, Clone, Default)]
pub struct Repo {
    pub dir: PathBuf,
    pub spawned: std::sync::Arc<std::sync::atomic::AtomicU32>,
}

impl Repo {
    pub fn at(dir: impl Into<PathBuf>) -> Repo {
        Repo {
            dir: dir.into(),
            spawned: Default::default(),
        }
    }

    /// How many processes this repository has been asked for.
    pub fn spawned(&self) -> u32 {
        self.spawned.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Run git in this repository, and say what it said.
    pub fn run(&self, args: &[&str]) -> Result<Output> {
        self.spawned.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // `FLYWHEEL_GIT_TRACE=1` prints every process and what it cost, which
        // is how the cost the profile states is checked by hand (169).
        let traced = std::env::var("FLYWHEEL_GIT_TRACE").is_ok();
        let began = std::time::Instant::now();
        let out = Command::new("git")
            .current_dir(&self.dir)
            .args(args)
            // The machinery's commits are the machinery's, whatever the
            // operator's own git configuration says.
            .env("GIT_AUTHOR_NAME", "flywheel")
            .env("GIT_AUTHOR_EMAIL", "flywheel@localhost")
            .env("GIT_COMMITTER_NAME", "flywheel")
            .env("GIT_COMMITTER_EMAIL", "flywheel@localhost")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("HOME", &self.dir)
            .output()
            .with_context(|| format!("running git {}", args.join(" ")))?;
        if traced {
            eprintln!("GITCALL {:>8}us {}", began.elapsed().as_micros(), args.join(" "));
        }
        Ok(Output {
            ok: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        })
    }

    /// Run git and fail if it did.
    pub fn git(&self, args: &[&str]) -> Result<String> {
        self.run(args)?.text().with_context(|| format!("git {}", args.join(" ")))
    }

    pub fn exists(&self) -> bool {
        self.dir.join(".git").is_dir() || self.dir.join("HEAD").is_file()
    }
}

/// Make a bare repository with an empty `main`: the state repository as
/// `flywheel init` leaves it (204, C.2).
pub fn init_bare(path: &Path) -> Result<Repo> {
    std::fs::create_dir_all(path)?;
    let bare = Repo::at(path);
    bare.git(&["init", "--bare", "--initial-branch=main", "--quiet", "."])?;
    Ok(bare)
}

/// Clone a bare repository into a working checkout of its shared line.
pub fn clone(remote: &Path, into: &Path) -> Result<Repo> {
    std::fs::create_dir_all(into.parent().unwrap_or(into))?;
    let parent = Repo::at(into.parent().unwrap_or(Path::new(".")));
    let name = into
        .file_name()
        .ok_or_else(|| anyhow!("{} has no name", into.display()))?
        .to_string_lossy()
        .to_string();
    parent.git(&[
        "clone",
        "--quiet",
        &remote.to_string_lossy(),
        &name,
    ])?;
    let repo = Repo::at(into);
    // A clone of an empty repository has no branch yet.
    if repo.run(&["rev-parse", "--verify", "HEAD"])?.ok {
        repo.git(&["checkout", "--quiet", "main"]).ok();
    } else {
        repo.git(&["checkout", "--quiet", "-b", "main"])?;
    }
    Ok(repo)
}

/// The sha a ref points at, or none.
pub fn rev(repo: &Repo, reference: &str) -> Result<Option<String>> {
    let out = repo.run(&["rev-parse", "--verify", "--quiet", reference])?;
    Ok(if out.ok {
        let sha = out.stdout.trim().to_string();
        (!sha.is_empty()).then_some(sha)
    } else {
        None
    })
}

/// The zero object id: the expected-old of a ref that does not exist yet.
pub const ZERO: &str = "0000000000000000000000000000000000000000";

/// A file's content at a commit, or none when the commit does not hold it.
pub fn show(repo: &Repo, commit: &str, path: &str) -> Result<Option<String>> {
    let spec = format!("{commit}:{path}");
    let out = repo.run(&["show", &spec])?;
    Ok(out.ok.then_some(out.stdout))
}

/// Every path under a prefix at a commit.
pub fn ls_tree(repo: &Repo, commit: &str, prefix: &str) -> Result<Vec<String>> {
    let out = repo.run(&["ls-tree", "-r", "--name-only", commit, prefix])?;
    if !out.ok {
        // An empty tree, or a prefix nothing is under yet.
        return Ok(vec![]);
    }
    Ok(out
        .stdout
        .lines()
        .map(str::to_string)
        .filter(|l| !l.is_empty())
        .collect())
}

/// The paths that moved between two commits — what a fetch tells a host to
/// re-read, rather than everything (130, 166).
pub fn diff_names(repo: &Repo, from: &str, to: &str) -> Result<Vec<String>> {
    if from == to {
        return Ok(vec![]);
    }
    let range = format!("{from}..{to}");
    let out = repo.run(&["diff", "--name-only", &range])?;
    if !out.ok {
        return Ok(vec![]);
    }
    Ok(out
        .stdout
        .lines()
        .map(str::to_string)
        .filter(|l| !l.is_empty())
        .collect())
}

/// Whether the history reachable from a commit carries a message holding this
/// text. This is how a repeat of an already-written effect finds itself (127).
pub fn log_grep(repo: &Repo, commit: &str, needle: &str) -> Result<bool> {
    let out = repo.run(&[
        "log",
        "--fixed-strings",
        &format!("--grep={needle}"),
        "--format=%H",
        commit,
    ])?;
    Ok(out.ok && !out.stdout.trim().is_empty())
}

/// Push a ref with expected-old: the compare-and-swap the whole profile rests
/// on (128, 134, 162, I15).
pub fn push_expecting(
    repo: &Repo,
    local: &str,
    remote_ref: &str,
    expected_old: &str,
) -> Result<bool> {
    let lease = format!("--force-with-lease={remote_ref}:{expected_old}");
    let spec = format!("{local}:{remote_ref}");
    let out = repo.run(&["push", "--quiet", &lease, "origin", &spec])?;
    if !out.ok && std::env::var("FLYWHEEL_GIT_TRACE").is_ok() {
        eprintln!("PUSHFAIL {spec}: {}", out.stderr.replace('\n', " / "));
    }
    Ok(out.ok)
}

/// Delete a remote ref, expecting it to be where we left it.
pub fn delete_expecting(repo: &Repo, remote_ref: &str, expected_old: &str) -> Result<bool> {
    let lease = format!("--force-with-lease={remote_ref}:{expected_old}");
    let spec = format!(":{remote_ref}");
    let out = repo.run(&["push", "--quiet", &lease, "origin", &spec])?;
    Ok(out.ok)
}

/// Write a file into the checkout. Nothing is staged: the commit's tree is
/// written from the checkout itself, in process (169, model.md §13).
pub fn stage(repo: &Repo, path: &str, content: &str) -> Result<()> {
    let full = repo.dir.join(path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&full, content).with_context(|| format!("writing {}", full.display()))?;
    Ok(())
}

/// Commit what is staged, with a message. An empty commit is allowed, because
/// an effect's write may change no file and still be a fact with an identity
/// (127, 167).
pub fn commit(repo: &Repo, message: &str, at: chrono::DateTime<chrono::Utc>) -> Result<String> {
    let when = at.to_rfc3339();
    let out = repo.run(&[
        "-c",
        &format!("user.name=flywheel"),
        "commit",
        "--quiet",
        "--allow-empty",
        "--date",
        &when,
        "-m",
        message,
    ])?;
    if !out.ok {
        bail!("committing: {}", out.stderr.trim());
    }
    rev(repo, "HEAD")?.ok_or_else(|| anyhow!("no HEAD after committing"))
}
