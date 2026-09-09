//! The instance machine's effects, performed against the git host, and the
//! evidence its guards read (204, 207, 208, 221).
//!
//! Every effect proves itself: the evidence below is read from the manifest and
//! the git host, never from a flag the machinery set, so a second run changes
//! nothing and a half-finished bootstrap is finished by the next tick (204).

use crate::bootstrap::Bootstrap;
use crate::git::Repo;
use crate::manifest::Manifest;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

/// What the world reports about an instance right now. Each name is one of
/// `atoms.yaml`'s, read the way `profiles/host.yaml` says (204, 207).
pub fn evidence(manifest: &Manifest) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    out.insert(
        "instance.blueprints_ready".into(),
        json!(exists(&manifest.blueprints.remote)),
    );
    out.insert(
        "instance.state_ready".into(),
        json!(exists(&manifest.state.remote)),
    );
    out.insert(
        "instance.app_recorded".into(),
        json!(!manifest.app.id.is_empty() && manifest.app.key_from.is_some()),
    );
    // Seen through the git host with the secret the operator placed; never
    // something an agent can make true (207, 207a).
    out.insert(
        "instance.app_installed".into(),
        json!(Bootstrap::app_installed(manifest)),
    );
    out.insert("instance.app_id".into(), json!(manifest.app.id));
    out.insert(
        "instance.host_registered".into(),
        json!(!manifest.hosts.is_empty()),
    );
    out
}

/// Perform one of the instance's effects. Returns whether the world moved; an
/// effect whose proof already holds moves nothing (204).
pub fn perform(
    effect: &str,
    manifest: &mut Manifest,
    bootstrap: &Bootstrap,
    host: &str,
    root: &Path,
    app_id: &str,
    key_from: &str,
) -> Result<bool> {
    let instance = manifest.instance.clone();
    Ok(match effect {
        "create_blueprints" => {
            if exists(&manifest.blueprints.remote) {
                false
            } else {
                manifest.blueprints = bootstrap.create_blueprints(&instance)?;
                true
            }
        }
        "create_state" => {
            if exists(&manifest.state.remote) {
                false
            } else {
                manifest.state = bootstrap.create_state(&instance)?;
                true
            }
        }
        "register_app" => {
            // Writes the App's id and records that its installation is
            // required. This installs nothing (207a).
            let was = (manifest.app.id.clone(), manifest.app.key_from.clone());
            Bootstrap::register_app(manifest, app_id, key_from);
            was != (manifest.app.id.clone(), manifest.app.key_from.clone())
        }
        "register_host" => {
            let was = manifest.hosts.len();
            Bootstrap::register_host(manifest, host, root);
            manifest.hosts.len() != was
        }
        // Not this crate's: the sessions, places and archive belong to the host
        // loop and the state store (221).
        _ => false,
    })
}

fn exists(remote: &str) -> bool {
    !remote.is_empty() && Repo::at(Path::new(remote)).exists()
}
