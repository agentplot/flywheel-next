//! flywheel-workspace-host: `Workspace` over git, on the clones a host keeps
//! (42, 49, 205, `host.yaml` disk and workspace).
//!
//! A line is a branch of the host's bare clone of the repository, with one
//! worktree of its own where the machinery takes, merges and lands; a place
//! is a worktree off that line where one session works; a landing pushes the
//! shared line to the git host. The facts the recorded binding keeps
//! (`fact/place/*`, `fact/line/*`) are kept here too and written at the same
//! moments, so every `place.*` and `line.*` read the machines make is the
//! recorded crate's `evidence` unchanged, and a tick reads with no git
//! process (169). What changes is that the acts reach a repository.
//!
//! Layout under the instance root (`<root>/<instance>/`), beside the bare
//! clones and the shared-line checkouts a join makes:
//!
//! ```text
//! lines/<repo>/<line slug>/    one worktree per line, at the line's head
//! places/<repo>/<place slug>/  one worktree per place, on branch place/<slug>
//! ```
//!
//! Every object's own place is `<object>#own` (`regions::place_key`). A bolt's
//! own place (44) is the line's worktree itself — the operator's place at the
//! head of the line, refreshed by every merge — because the bolt is its line;
//! a work item's or an elaboration's own place is a worktree off its owner's
//! line.
//!
//! The git binary does the merging, as `host.yaml`'s Tools header binds it;
//! every spawn goes through the world host's `Repo`, so the count a tick
//! reports includes them (169).

use anyhow::{bail, Context, Result};
use flywheel_atoms::{Endpoint, LandingPolicy, Records, TakeOutcome, Workspace};
use flywheel_workspace_recorded::{place_fact, RecordedWorkspace};
use flywheel_world_host::git::Repo;
use serde_json::json;
use std::path::{Path, PathBuf};

/// The manifest's name for the blueprints clone, where an intent's line and an
/// elaboration's place live (`intent.yaml` line, `manifest.rs`
/// all_repositories).
pub const BLUEPRINTS: &str = "flywheel-blueprints";

/// `Workspace` over the clones under one instance root, with the facts in the
/// store.
pub struct HostWorkspace<'a, S: Records> {
    pub store: &'a mut S,
    pub root: PathBuf,
}

/// An id as a directory or branch segment: `/` and `#` become `-`.
pub fn slug(id: &str) -> String {
    id.chars()
        .map(|c| match c {
            '/' | '#' | ' ' | ':' => '-',
            c => c,
        })
        .collect()
}

/// Where a line's worktree is.
pub fn line_dir(root: &Path, repository: &str, line: &str) -> PathBuf {
    root.join("lines").join(repository).join(slug(line))
}

/// Whether a place is its line's own worktree: the object it belongs to is
/// the line (a bolt's own place, 44).
pub fn is_the_lines_own(place: &str, line: &str) -> bool {
    !line.is_empty() && place.strip_suffix("#own") == Some(line)
}

/// Where a place's worktree is. A line's own place is its worktree.
pub fn place_dir(root: &Path, repository: &str, place: &str, line: &str) -> PathBuf {
    match is_the_lines_own(place, line) {
        true => line_dir(root, repository, line),
        false => root.join("places").join(repository).join(slug(place)),
    }
}

/// The branch a place works on.
pub fn place_branch(place: &str) -> String {
    format!("place/{}", slug(place))
}

impl<'a, S: Records> HostWorkspace<'a, S> {
    pub fn new(store: &'a mut S, root: impl Into<PathBuf>) -> Self {
        HostWorkspace {
            store,
            root: root.into(),
        }
    }

    fn bare(&self, repository: &str) -> PathBuf {
        self.root.join(format!("{repository}.git"))
    }

    fn checkout(&self, repository: &str) -> PathBuf {
        self.root.join(repository).join("main")
    }

    /// The repository an object's line is on: the `repository` its record or
    /// an ancestor's carries, or the blueprints for everything on the design
    /// side (an intent's line is off the blueprints' shared line).
    pub fn repository_of(&self, object: &str) -> Result<String> {
        flywheel_domain::regions::repository_of(&*self.store, object)
    }

    /// The line an object's place is off: a bolt or an intent is its own line;
    /// anything else works off the nearest ancestor that is one (`place.yaml`
    /// params.line: bolt-line, intent's line).
    pub fn line_of(&self, object: &str) -> Result<String> {
        let mut at = object.strip_suffix("#own").unwrap_or(object).to_string();
        for _ in 0..8 {
            let Some(held) = self.store.get(&at)? else {
                // A chore of a repository's shared line stands under the
                // repository, which the state holds no record of: its line is
                // that repository's own (60, `unit.yaml` parent).
                if at.starts_with("repository/") {
                    return Ok(String::new());
                }
                bail!("`{object}`: no record for `{at}`, so its line is unknown");
            };
            if matches!(held.machine.as_str(), "bolt" | "intent") {
                return Ok(at);
            }
            // The instance and a repository own only the chores of a shared
            // line, which work off that line and merge there (60, 123).
            if matches!(held.machine.as_str(), "repository" | "instance") {
                return Ok(String::new());
            }
            match held.parent {
                Some(parent) => at = parent,
                // Curation, planning and the operator's own session work off
                // the shared line directly: their line is the repository's.
                None => return Ok(String::new()),
            }
        }
        bail!("`{object}`: the parent chain is deeper than any object of the model's")
    }

    /// The shared line of a repository: what the bare clone's HEAD names.
    fn shared_line(&self, repository: &str) -> Result<String> {
        let head = Repo::at(self.bare(repository)).git(&["symbolic-ref", "--short", "HEAD"])?;
        Ok(head.trim().to_string())
    }

    fn branch_exists(&self, repository: &str, branch: &str) -> Result<bool> {
        Ok(Repo::at(self.bare(repository))
            .run(&["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")])?
            .ok)
    }

    /// A worktree of the bare clone at `dir`, on `branch`, made when absent;
    /// `from` names what a new branch starts at.
    fn worktree(&self, repository: &str, dir: &Path, branch: &str, from: Option<&str>) -> Result<()> {
        if dir.join(".git").exists() {
            return Ok(());
        }
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bare = Repo::at(self.bare(repository));
        let dir_s = dir.to_string_lossy().to_string();
        // A worktree registered for a directory that is gone is pruned first,
        // so a place re-made after a removal by hand is made rather than
        // refused (222).
        bare.run(&["worktree", "prune"])?;
        match (self.branch_exists(repository, branch)?, from) {
            (true, _) => bare.git(&["worktree", "add", "--quiet", &dir_s, branch])?,
            (false, Some(from)) => bare.git(&["worktree", "add", "--quiet", "-b", branch, &dir_s, from])?,
            (false, None) => bail!("`{branch}` is no branch of {repository} and nothing says where it starts"),
        };
        Ok(())
    }

    /// `.flywheel/` inside any worktree is untracked (203, `host.yaml`
    /// untracked): excluded once in the bare clone, which every worktree of
    /// it reads.
    fn exclude_flywheel_dir(&self, repository: &str) -> Result<()> {
        let info = self.bare(repository).join("info");
        std::fs::create_dir_all(&info)?;
        let exclude = info.join("exclude");
        let held = std::fs::read_to_string(&exclude).unwrap_or_default();
        if !held.lines().any(|l| l.trim() == ".flywheel/") {
            std::fs::write(&exclude, format!("{held}\n.flywheel/\n"))?;
        }
        Ok(())
    }

    fn facts(&mut self) -> RecordedWorkspace<'_, S> {
        RecordedWorkspace::new(self.store)
    }

    /// Merge `what` into the worktree at `dir`; aborted whole on conflict (179).
    fn merge_into(&self, dir: &Path, what: &str, message: &str) -> Result<TakeOutcome> {
        let tree = Repo::at(dir);
        let said = tree.run(&["merge", "--quiet", "--no-edit", "-m", message, what])?;
        if said.ok {
            return Ok(TakeOutcome::Done);
        }
        let detail = format!("{}{}", said.out.trim(), said.err.trim());
        tree.run(&["merge", "--abort"])?;
        Ok(TakeOutcome::Conflicted { detail })
    }

    fn remove_worktree(&self, repository: &str, dir: &Path, branch: Option<&str>) -> Result<()> {
        let bare = Repo::at(self.bare(repository));
        if dir.join(".git").exists() {
            bare.git(&["worktree", "remove", "--force", &dir.to_string_lossy()])?;
        }
        bare.run(&["worktree", "prune"])?;
        if let Some(branch) = branch {
            if self.branch_exists(repository, branch)? {
                bare.git(&["branch", "-D", "--quiet", branch])?;
            }
        }
        Ok(())
    }
}

impl<S: Records> Workspace for HostWorkspace<'_, S> {
    fn create_line(&mut self, line: &str, parent: &str) -> Result<()> {
        let repository = self.repository_of(line)?;
        let from = match parent.is_empty() {
            true => self.shared_line(&repository)?,
            false => parent.to_string(),
        };
        let dir = line_dir(&self.root, &repository, line);
        self.worktree(&repository, &dir, line, Some(&from))
            .with_context(|| format!("making the line `{line}` on {repository}"))?;
        self.exclude_flywheel_dir(&repository)?;
        self.facts().create_line(line, parent)
    }

    fn take_parent(&mut self, line: &str) -> Result<TakeOutcome> {
        let repository = self.repository_of(line)?;
        let parent = self.shared_line(&repository)?;
        let dir = line_dir(&self.root, &repository, line);
        // The bare clone's shared line is what the checkout last pulled; the
        // take is against that, which is what a tick fetched (165).
        let outcome = self.merge_into(&dir, &parent, &format!("take {parent} into {line}"))?;
        match &outcome {
            TakeOutcome::Done => {
                self.facts().take_parent(line)?;
            }
            TakeOutcome::Conflicted { detail } => {
                let fact = flywheel_workspace_recorded::line_fact(line);
                self.facts()
                    .set(&fact, &[("take_conflicts", json!(true)), ("conflict_detail", json!(detail))])?;
            }
        }
        Ok(outcome)
    }

    fn remove_line(&mut self, line: &str) -> Result<()> {
        let repository = self.repository_of(line)?;
        let dir = line_dir(&self.root, &repository, line);
        self.remove_worktree(&repository, &dir, Some(line))?;
        self.facts().remove_line(line)
    }

    fn land_line(&mut self, line: &str, policy: LandingPolicy) -> Result<()> {
        let repository = self.repository_of(line)?;
        let parent = self.shared_line(&repository)?;
        match policy {
            LandingPolicy::PullRequest => bail!(
                "landing `{line}` by pull request: this release lands direct alone; the request is the tracker profile's (175, phase 2)"
            ),
            LandingPolicy::Direct => {}
        }
        let dir = line_dir(&self.root, &repository, line);
        let tree = Repo::at(&dir);
        // The final take happened (50): the shared line is an ancestor of the
        // line's head, so the landing is one fast-forward of the shared line.
        if !tree.run(&["merge-base", "--is-ancestor", &parent, "HEAD"])?.ok {
            bail!("`{line}` does not contain `{parent}`; the take before landing has not happened (50)");
        }
        let bare = self.bare(&repository);
        tree.git(&["push", "--quiet", &bare.to_string_lossy(), &format!("HEAD:{parent}")])
            .with_context(|| format!("landing `{line}` on {parent}"))?;
        // The commit that reached the git host is the fact (161).
        Repo::at(&bare)
            .git(&["push", "--quiet", "origin", &parent])
            .with_context(|| format!("pushing {repository}'s {parent} to the git host"))?;
        // The machinery's own checkout follows.
        let checkout = self.checkout(&repository);
        if checkout.join(".git").is_dir() {
            Repo::at(&checkout).run(&["pull", "--quiet", "--ff-only"])?;
        }
        self.facts().land_line(line, LandingPolicy::Direct)
    }

    fn write_acceptance(&mut self, line: &str, body: &str) -> Result<()> {
        let repository = self.repository_of(line)?;
        let dir = line_dir(&self.root, &repository, line);
        // One commit on the line, under the machinery's prefix (192, 203); an
        // intent's line gets none.
        if !line.starts_with("intent/") {
            let path = format!("flywheel/acceptance/{}.md", slug(line));
            let full = dir.join(&path);
            if let Some(parent) = full.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&full, body)?;
            let tree = Repo::at(&dir);
            tree.git(&["add", "--", &path])?;
            let said = tree.run(&["commit", "--quiet", "-m", &format!("chore({line}): acceptance")])?;
            if !said.ok && !said.out.contains("nothing to commit") && !said.err.contains("nothing to commit") {
                bail!("{}", said.err.trim());
            }
        }
        self.facts().write_acceptance(line, body)
    }

    fn prepare_place(&mut self, place: &str, _line: &str, work_order: &str) -> Result<()> {
        // The line the place is off is the owner's, whatever the caller
        // passed as the region's own name (`place.yaml` params.line).
        let repository = self.repository_of(place)?;
        let line = self.line_of(place)?;
        let base = match line.is_empty() {
            true => self.shared_line(&repository)?,
            false => line.clone(),
        };
        let dir = place_dir(&self.root, &repository, place, &base);
        if is_the_lines_own(place, &line) {
            // The line's own worktree: made with the line (44).
            self.worktree(&repository, &dir, &base, None)
                .with_context(|| format!("the own place of `{base}`"))?;
        } else {
            self.worktree(&repository, &dir, &place_branch(place), Some(&base))
                .with_context(|| format!("making the place `{place}` off `{base}`"))?;
        }
        self.exclude_flywheel_dir(&repository)?;
        let order = dir.join(".flywheel").join("work-order.md");
        std::fs::create_dir_all(order.parent().expect("the .flywheel directory"))?;
        std::fs::write(&order, work_order)?;
        let fact_line = match line.is_empty() {
            true => base.clone(),
            false => line.clone(),
        };
        self.facts().prepare_place(place, &fact_line, work_order)?;
        self.facts().set(
            &place_fact(place),
            &[("dir", json!(dir.to_string_lossy())), ("branch", json!(place_branch(place)))],
        )
    }

    fn rebase_place(&mut self, place: &str) -> Result<TakeOutcome> {
        let repository = self.repository_of(place)?;
        let line = self.line_of(place)?;
        let base = match line.is_empty() {
            true => self.shared_line(&repository)?,
            false => line,
        };
        if is_the_lines_own(place, &base) {
            return self.facts().rebase_place(place);
        }
        let dir = place_dir(&self.root, &repository, place, &base);
        let tree = Repo::at(&dir);
        let said = tree.run(&["rebase", "--quiet", &base])?;
        if said.ok {
            self.facts().rebase_place(place)?;
            return Ok(TakeOutcome::Done);
        }
        let detail = format!("{}{}", said.out.trim(), said.err.trim());
        tree.run(&["rebase", "--abort"])?;
        self.facts()
            .set(&place_fact(place), &[("conflicted", json!(true)), ("conflict_detail", json!(detail))])?;
        Ok(TakeOutcome::Conflicted { detail })
    }

    fn merge_place(&mut self, place: &str) -> Result<TakeOutcome> {
        let repository = self.repository_of(place)?;
        let line = self.line_of(place)?;
        let base = match line.is_empty() {
            true => self.shared_line(&repository)?,
            false => line,
        };
        if is_the_lines_own(place, &base) {
            // The own place is the line: nothing to merge (44).
            return self.facts().merge_place(place);
        }
        let dir = line_dir(&self.root, &repository, &base);
        if !dir.join(".git").exists() {
            // A place off the shared line itself (curation, planning): its
            // merge is a landing of the place's branch on the shared line.
            let bare = self.bare(&repository);
            let place_tree = Repo::at(place_dir(&self.root, &repository, place, &base));
            place_tree.git(&["push", "--quiet", &bare.to_string_lossy(), &format!("HEAD:{base}")])?;
            Repo::at(&bare).git(&["push", "--quiet", "origin", &base])?;
            return self.facts().merge_place(place);
        }
        let outcome = self.merge_into(&dir, &place_branch(place), &format!("merge {place} into {base}"))?;
        match &outcome {
            TakeOutcome::Done => {
                self.facts().merge_place(place)?;
            }
            TakeOutcome::Conflicted { detail } => {
                self.facts()
                    .set(&place_fact(place), &[("conflicted", json!(true)), ("conflict_detail", json!(detail))])?;
            }
        }
        Ok(outcome)
    }

    fn remove_place(&mut self, place: &str) -> Result<()> {
        let repository = self.repository_of(place)?;
        let line = self.line_of(place)?;
        let base = match line.is_empty() {
            true => self.shared_line(&repository)?,
            false => line,
        };
        if !is_the_lines_own(place, &base) {
            let dir = place_dir(&self.root, &repository, place, &base);
            self.remove_worktree(&repository, &dir, Some(&place_branch(place)))?;
        }
        self.facts().remove_place(place)
    }

    fn endpoints(&self, _place: &str) -> Result<Vec<Endpoint>> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests;
