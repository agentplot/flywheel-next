//! The repository's objects and refs, in process.
//!
//! `gix` for reads and for writing blobs, trees, commits and refs, as
//! `profiles/git-only.yaml` and model.md §13 name it. Nothing here spawns a
//! process: a tick's read path is the checkout and the object database, and the
//! only two processes a tick makes are the fetch from the remote and the push
//! with the expected-old guard (169, `git-only.yaml` cost).

use anyhow::{anyhow, Context, Result};
use std::path::Path;

/// The repository at a checkout.
pub fn open(dir: &Path) -> Result<gix::Repository> {
    gix::open(dir).with_context(|| format!("opening {}", dir.display()))
}

/// The commit a ref points at, or none where the ref is not there.
///
/// `HEAD` and a remote-tracking short name (`origin/main`) are the two forms
/// the store asks for besides a full ref name.
pub fn rev(repo: &gix::Repository, reference: &str) -> Result<Option<String>> {
    if reference == "HEAD" {
        return Ok(repo.head_id().ok().map(|id| id.to_string()));
    }
    if !reference.starts_with("refs/") {
        // A short name the way `git rev-parse` reads it: `origin/main` is the
        // remote-tracking ref, wherever the ref store keeps it — loose or
        // packed.
        return Ok(repo.rev_parse_single(reference).ok().map(|id| id.to_string()));
    }
    match repo.find_reference(reference) {
        Ok(mut found) => Ok(Some(found.peel_to_id()?.to_string())),
        Err(gix::reference::find::existing::Error::NotFound { .. }) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Every ref under a prefix, with the commit each points at.
pub fn refs_under(repo: &gix::Repository, prefix: &str) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    let platform = repo.references()?;
    for reference in platform.prefixed(prefix)? {
        let mut reference = match reference {
            Ok(found) => found,
            Err(e) => return Err(anyhow!("reading a ref under {prefix}: {e}")),
        };
        let name = reference.name().as_bstr().to_string();
        let id = reference.peel_to_id()?.to_string();
        out.push((name, id));
    }
    Ok(out)
}

/// A file's bytes at a commit, or none where the commit does not hold it.
pub fn blob_at(repo: &gix::Repository, commit: &str, path: &str) -> Result<Option<Vec<u8>>> {
    let Some(mut tree) = tree_of(repo, commit)? else {
        return Ok(None);
    };
    let Some(entry) = tree.peel_to_entry_by_path(path)? else {
        return Ok(None);
    };
    let object = entry.object()?;
    Ok(Some(object.data.clone()))
}

/// Every path under a tree, with the blob id at each.
pub fn paths_at(repo: &gix::Repository, commit: &str) -> Result<Vec<String>> {
    let Some(tree) = tree_of(repo, commit)? else {
        return Ok(vec![]);
    };
    let mut out = Vec::new();
    walk(repo, &tree, "", &mut out)?;
    Ok(out)
}

fn walk(repo: &gix::Repository, tree: &gix::Tree<'_>, under: &str, into: &mut Vec<String>) -> Result<()> {
    for entry in tree.iter() {
        let entry = entry?;
        let name = entry.filename().to_string();
        let path = match under.is_empty() {
            true => name,
            false => format!("{under}/{name}"),
        };
        if entry.mode().is_tree() {
            let inner = entry.object()?.into_tree();
            walk(repo, &inner, &path, into)?;
        } else {
            into.push(path);
        }
    }
    Ok(())
}

fn tree_of<'r>(repo: &'r gix::Repository, commit: &str) -> Result<Option<gix::Tree<'r>>> {
    if commit.is_empty() || commit == crate::git::ZERO {
        return Ok(None);
    }
    let id = gix::ObjectId::from_hex(commit.as_bytes())
        .with_context(|| format!("`{commit}` is no object id"))?;
    let Ok(object) = repo.find_object(id) else {
        return Ok(None);
    };
    Ok(Some(object.peel_to_tree()?))
}

/// Write the working tree as it stands as a tree object, `.git` left out.
pub fn write_tree(repo: &gix::Repository, dir: &Path) -> Result<gix::ObjectId> {
    write_tree_of(repo, dir)
}

fn write_tree_of(repo: &gix::Repository, dir: &Path) -> Result<gix::ObjectId> {
    let mut entries: Vec<gix::objs::tree::Entry> = Vec::new();
    let mut names: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .collect();
    names.sort();
    for path in names {
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string()) else {
            continue;
        };
        if name == ".git" {
            continue;
        }
        if path.is_dir() {
            let id = write_tree_of(repo, &path)?;
            entries.push(gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Tree.into(),
                filename: name.into(),
                oid: id,
            });
        } else {
            let body = std::fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
            let id = repo.write_blob(&body)?.detach();
            entries.push(gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Blob.into(),
                filename: name.into(),
                oid: id,
            });
        }
    }
    // A tree's entries are sorted by name, which is what git requires.
    entries.sort();
    let tree = gix::objs::Tree { entries };
    Ok(repo.write_object(&tree)?.detach())
}

/// One tree holding one file: what a lease or a heartbeat branch's orphan
/// commit carries (`git-only.yaml layout`, D5).
pub fn write_one_file_tree(repo: &gix::Repository, name: &str, body: &str) -> Result<gix::ObjectId> {
    let blob = repo.write_blob(body.as_bytes())?.detach();
    let tree = gix::objs::Tree {
        entries: vec![gix::objs::tree::Entry {
            mode: gix::objs::tree::EntryKind::Blob.into(),
            filename: name.into(),
            oid: blob,
        }],
    };
    Ok(repo.write_object(&tree)?.detach())
}

/// Write a commit. The machinery's commits are the machinery's, whatever the
/// operator's own git configuration says.
pub fn write_commit(
    repo: &gix::Repository,
    tree: gix::ObjectId,
    parents: &[String],
    message: &str,
    at: chrono::DateTime<chrono::Utc>,
) -> Result<String> {
    let when = gix::date::Time {
        seconds: at.timestamp(),
        offset: 0,
    };
    let who = gix::actor::Signature {
        name: "flywheel".into(),
        email: "flywheel@localhost".into(),
        time: when,
    };
    let parents: Vec<gix::ObjectId> = parents
        .iter()
        .filter(|p| !p.is_empty() && p.as_str() != crate::git::ZERO)
        .map(|p| gix::ObjectId::from_hex(p.as_bytes()).map_err(|e| anyhow!("{p}: {e}")))
        .collect::<Result<_>>()?;
    let commit = gix::objs::Commit {
        tree,
        parents: parents.into(),
        author: who.clone(),
        committer: who,
        encoding: None,
        message: message.into(),
        extra_headers: vec![],
    };
    Ok(repo.write_object(&commit)?.detach().to_string())
}

/// Move a ref, expecting it where we left it. A ref that moved under us is not
/// ours to move: the caller reads again (134, 162).
pub fn set_ref(
    repo: &gix::Repository,
    reference: &str,
    to: &str,
    expected_old: Option<&str>,
) -> Result<()> {
    use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};
    let new = gix::ObjectId::from_hex(to.as_bytes()).map_err(|e| anyhow!("{to}: {e}"))?;
    let previous = match expected_old {
        None => PreviousValue::Any,
        Some(old) if old.is_empty() || old == crate::git::ZERO => PreviousValue::MustNotExist,
        Some(old) => PreviousValue::MustExistAndMatch(gix::refs::Target::Object(
            gix::ObjectId::from_hex(old.as_bytes()).map_err(|e| anyhow!("{old}: {e}"))?,
        )),
    };
    repo.edit_reference(RefEdit {
        change: Change::Update {
            log: LogChange {
                mode: RefLog::AndReference,
                force_create_reflog: false,
                message: "flywheel".into(),
            },
            expected: previous,
            new: gix::refs::Target::Object(new),
        },
        name: reference.try_into()?,
        deref: false,
    })?;
    Ok(())
}

/// Delete a ref.
pub fn delete_ref(repo: &gix::Repository, reference: &str) -> Result<()> {
    use gix::refs::transaction::{Change, PreviousValue, RefEdit, RefLog};
    repo.edit_reference(RefEdit {
        change: Change::Delete {
            expected: PreviousValue::Any,
            log: RefLog::AndReference,
        },
        name: reference.try_into()?,
        deref: false,
    })?;
    Ok(())
}

/// Whether the history reachable from a commit carries a message holding this
/// text. This is how a repeat of an already-written effect finds itself (127).
pub fn message_holds(repo: &gix::Repository, commit: &str, needle: &str) -> Result<bool> {
    for id in commits_from(repo, commit)? {
        let object = repo.find_object(id)?;
        let commit = object.into_commit();
        if commit.message_raw_sloppy().to_string().contains(needle) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A commit's first parent, or the empty string where it has none.
pub fn first_parent(repo: &gix::Repository, commit: &str) -> Result<String> {
    let id = gix::ObjectId::from_hex(commit.as_bytes()).map_err(|e| anyhow!("{commit}: {e}"))?;
    let object = repo.find_object(id)?.into_commit();
    let parent = object.parent_ids().next().map(|p| p.detach());
    Ok(parent.map(|p| p.to_string()).unwrap_or_default())
}

/// Every commit reachable from one, newest first.
pub fn commits_from(repo: &gix::Repository, commit: &str) -> Result<Vec<gix::ObjectId>> {
    if commit.is_empty() || commit == crate::git::ZERO {
        return Ok(vec![]);
    }
    let id = gix::ObjectId::from_hex(commit.as_bytes()).map_err(|e| anyhow!("{commit}: {e}"))?;
    if repo.find_object(id).is_err() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for info in repo.rev_walk([id]).all()? {
        out.push(info?.id);
    }
    Ok(out)
}

/// The commits of `to` that `from` does not reach: what a host has made and not
/// yet landed (161, D4a).
pub fn commits_between(repo: &gix::Repository, from: &str, to: &str) -> Result<Vec<String>> {
    let reached: std::collections::BTreeSet<gix::ObjectId> =
        commits_from(repo, from)?.into_iter().collect();
    Ok(commits_from(repo, to)?
        .into_iter()
        .filter(|id| !reached.contains(id))
        .map(|id| id.to_string())
        .collect())
}

/// Whether one commit is reachable from another.
pub fn reaches(repo: &gix::Repository, from: &str, to: &str) -> Result<bool> {
    if from == to {
        return Ok(true);
    }
    let Ok(wanted) = gix::ObjectId::from_hex(from.as_bytes()) else {
        return Ok(false);
    };
    Ok(commits_from(repo, to)?.into_iter().any(|id| id == wanted))
}

/// The newest commit reachable from a tip that changed anything under a path
/// prefix, with its subject. What `git log -1 -- <path>` answers, in process.
pub fn newest_touching(
    repo: &gix::Repository,
    tip: &str,
    prefix: &str,
) -> Result<Option<(String, String)>> {
    for id in commits_from(repo, tip)? {
        let sha = id.to_string();
        let parent = first_parent(repo, &sha)?;
        if changed_paths(repo, &parent, &sha)?
            .iter()
            .any(|path| path == prefix || path.starts_with(&format!("{prefix}/")))
        {
            let object = repo.find_object(id)?.into_commit();
            let message = object.message_raw_sloppy().to_string();
            let subject = message.lines().next().unwrap_or_default().to_string();
            return Ok(Some((sha, subject)));
        }
    }
    Ok(None)
}

/// The paths that differ between two commits — what a fetch tells a host to
/// re-read, rather than everything (130, 166).
pub fn changed_paths(repo: &gix::Repository, from: &str, to: &str) -> Result<Vec<String>> {
    if from == to {
        return Ok(vec![]);
    }
    let before: std::collections::BTreeMap<String, Option<Vec<u8>>> = paths_at(repo, from)?
        .into_iter()
        .map(|p| (p.clone(), blob_at(repo, from, &p).unwrap_or_default()))
        .collect();
    let after: std::collections::BTreeMap<String, Option<Vec<u8>>> = paths_at(repo, to)?
        .into_iter()
        .map(|p| (p.clone(), blob_at(repo, to, &p).unwrap_or_default()))
        .collect();
    let mut out: Vec<String> = Vec::new();
    for (path, body) in &after {
        if before.get(path) != Some(body) {
            out.push(path.clone());
        }
    }
    for path in before.keys() {
        if !after.contains_key(path) {
            out.push(path.clone());
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Put the working tree at a commit: every path that differs written or
/// removed. What `git reset --hard` does, without the process (D6).
pub fn put_worktree_at(repo: &gix::Repository, dir: &Path, from: &str, to: &str) -> Result<()> {
    for path in changed_paths(repo, from, to)? {
        let full = dir.join(&path);
        match blob_at(repo, to, &path)? {
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
    Ok(())
}
