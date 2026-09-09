//! `flywheel host join` and `flywheel host doctor`.
//!
//! A host joins by one command and never by hand: it clones the state, the
//! blueprints and every tracked built repository bare under one root the
//! manifest names, keeps one checkout of each shared line for the machinery's
//! own merges, and refuses to start on a hand-made layout, saying what differs
//! (205, 222).

use crate::git::Repo;
use crate::world::HostWorld;
use anyhow::Result;
use flywheel_atoms::World;

/// What a join added. A repeat adds only what is missing (205).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Joined {
    pub cloned: Vec<String>,
    pub checked_out: Vec<String>,
}

/// Clone what the manifest names, and check out each shared line once. Places
/// are worktrees made later by `prepare_place`; a join makes none (205, 52).
pub fn join(world: &mut HostWorld) -> Result<Joined> {
    let mut joined = Joined::default();
    for (name, _) in world.manifest.all_repositories() {
        if !Repo::at(world.bare(&name)).exists() {
            joined.cloned.push(name.clone());
        }
        if !world.checkout(&name).join(".git").is_dir() {
            joined.checked_out.push(name.clone());
        }
    }
    world.clone_repositories()?;
    Ok(joined)
}

/// The first difference between the layout the manifest describes and what is
/// on disk, or none. A host refuses to start on a hand-made layout and says
/// what differs rather than working around it (205, 222).
pub fn doctor(world: &HostWorld) -> Vec<String> {
    let mut differences = Vec::new();
    if !world.root.is_dir() {
        differences.push(format!(
            "the root {} does not exist; `flywheel host join` makes it (205)",
            world.root.display()
        ));
        return differences;
    }
    for (name, repository) in world.manifest.all_repositories() {
        let bare = world.bare(&name);
        let repo = Repo::at(&bare);
        if !repo.exists() {
            differences.push(format!(
                "{name}: no bare clone at {} (205)",
                bare.display()
            ));
            continue;
        }
        if !repo.is_bare() {
            differences.push(format!(
                "{name}: {} is a working checkout where the layout wants a bare clone; \
                 a host does not work in the repositories it keeps (205)",
                bare.display()
            ));
        }
        let checkout = world.checkout(&name);
        if !checkout.join(".git").is_dir() {
            differences.push(format!(
                "{name}: no checkout of `{}` at {} (205)",
                repository.shared_line,
                checkout.display()
            ));
        }
    }
    differences
}
