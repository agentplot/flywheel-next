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

/// A repository on disk, bare or checked out.
#[derive(Debug, Clone)]
pub struct Repo {
    pub dir: PathBuf,
}

impl Repo {
    pub fn at(dir: impl Into<PathBuf>) -> Repo {
        Repo { dir: dir.into() }
    }

    pub fn run(&self, args: &[&str]) -> Result<Said> {
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
pub fn show(repo: &Repo, line: &str, path: &str) -> Result<Option<String>> {
    let said = repo.run(&["show", &format!("{line}:{path}")])?;
    Ok(if said.ok { Some(said.out) } else { None })
}

/// Every path at a repository's shared line under a prefix.
pub fn ls_tree(repo: &Repo, line: &str, prefix: &str) -> Result<Vec<String>> {
    let mut args = vec!["ls-tree", "-r", "--name-only", line];
    if !prefix.is_empty() {
        args.push(prefix);
    }
    let said = repo.run(&args)?;
    if !said.ok {
        return Ok(vec![]);
    }
    Ok(said
        .out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
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
