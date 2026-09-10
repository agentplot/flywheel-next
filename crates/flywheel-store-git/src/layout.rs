//! Where each thing lives in the state repository (`git-only.yaml layout`).
//!
//! One place states every path, so a reader with no host running knows where to
//! look and a second implementation cannot drift from the first (132, 145).

/// An object's record.
pub fn object(id: &str) -> String {
    format!("objects/{id}/object.rec")
}

/// An object's thread: append-only entries — question, answer, note, exit,
/// offer, refusal, moved, op-response.
pub fn thread(id: &str) -> String {
    format!("objects/{id}/thread.rec")
}

/// The object id a record path names, or none when the path is not one.
pub fn id_of(path: &str) -> Option<&str> {
    path.strip_prefix("objects/")?.strip_suffix("/object.rec")
}

/// The object id a path under `objects/` concerns, whatever file it is: what a
/// notice names after a fetch (130).
pub fn touched(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("objects/")?;
    let cut = rest.rfind('/')?;
    Some(&rest[..cut])
}

/// The op-response record, named by its delivery id, so a second delivery
/// writes the same file (137).
pub fn response(delivery: &str) -> String {
    format!("responses/{}.rec", delivery.replace('/', "-"))
}

pub const RESPONSES: &str = "responses";

pub fn ask(id: &str) -> String {
    format!("asks/{id}.rec")
}

/// The run record: one file per host per day (79–82, 167).
pub fn run_record(host: &str, date: &str) -> String {
    format!("runs/{host}/{date}.rec")
}

/// The status view, rebuilt by `render_status`, stating the commit and time it
/// is as of (132, 145).
pub const STATUS: &str = "status.html";

/// The branch a lease lives on. Never a file on `main` (D5).
///
/// The fixed leaf is load-bearing: object ids nest — `unit/atlas/rail-tail`
/// owns `unit/atlas/rail-tail/wi-1` — and git refuses a ref whose name is a
/// directory prefix of another, so `lease/<id>` alone could never hold a unit
/// and its item at once. With the leaf they are siblings
/// (`git-only.yaml layout`, 128, 163).
pub fn lease_ref(object: &str) -> String {
    format!("refs/heads/lease/{object}/{LEAF}")
}

/// The remote-tracking name of the same branch.
pub fn lease_remote(object: &str) -> String {
    format!("refs/remotes/origin/lease/{object}/{LEAF}")
}

/// A sink's presenter lease.
pub fn sink_lease_ref(sink: &str) -> String {
    format!("refs/heads/lease/sink/{sink}/{LEAF}")
}

/// The branch a host's heartbeat lives on, with its own fixed leaf and for the
/// same reason; the dispatcher heartbeats as `host/dispatcher/heartbeat`
/// (`git-only.yaml layout`).
pub fn host_ref(host: &str) -> String {
    format!("refs/heads/host/{host}/{HEARTBEAT}")
}

pub fn host_remote(host: &str) -> String {
    format!("refs/remotes/origin/host/{host}/{HEARTBEAT}")
}

/// Every host's heartbeat branch, as a pattern.
pub const HOSTS_REMOTE: &str = "refs/remotes/origin/host/*/heartbeat";

/// The leaf every lease branch ends in.
pub const LEAF: &str = "lease";

/// The leaf every heartbeat branch ends in.
pub const HEARTBEAT: &str = "heartbeat";

/// The host a heartbeat branch names.
pub fn host_of_ref(reference: &str) -> Option<&str> {
    reference
        .strip_prefix("refs/remotes/origin/host/")
        .or_else(|| reference.strip_prefix("refs/heads/host/"))?
        .strip_suffix(&format!("/{HEARTBEAT}"))
}

/// The shared line: the only branch a host reads state from.
pub const MAIN: &str = "refs/heads/main";
pub const ORIGIN_MAIN: &str = "origin/main";

/// The record inside a lease or host branch's one orphan commit.
pub const RECORD: &str = "record.rec";

/// What the world reports, which this profile inherits from the host and the
/// sessions bindings rather than owning (B.3, `record-derived.yaml`).
///
/// A host reads it from the shared line like anything else, so several hosts of
/// one instance answer the same evidence the same way, and a host started after
/// the fact reads what was already true rather than starting blind (136, I14).
pub const GIVEN: &str = "given.rec";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_names_its_object() {
        assert_eq!(object("lamp/1"), "objects/lamp/1/object.rec");
        assert_eq!(id_of("objects/lamp/1/object.rec"), Some("lamp/1"));
        assert_eq!(
            id_of("objects/elaboration/atlas/research-1/object.rec"),
            Some("elaboration/atlas/research-1")
        );
        assert_eq!(id_of("status.html"), None);
        assert_eq!(touched("objects/lamp/1/thread.rec"), Some("lamp/1"));
    }

    /// A lease ref never becomes a directory prefix of another, which is what
    /// lets a unit and its item both be held (`git-only.yaml layout`).
    #[test]
    fn a_lease_ref_is_never_a_prefix_of_another() {
        let unit = lease_ref("unit/atlas/rail-tail");
        let item = lease_ref("unit/atlas/rail-tail/wi-1");
        assert_eq!(unit, "refs/heads/lease/unit/atlas/rail-tail/lease");
        assert_eq!(item, "refs/heads/lease/unit/atlas/rail-tail/wi-1/lease");
        assert!(!item.starts_with(&format!("{unit}/")));
        assert!(!unit.starts_with(&format!("{item}/")));
        assert_eq!(host_ref("mac-mini"), "refs/heads/host/mac-mini/heartbeat");
        assert_eq!(
            host_of_ref("refs/remotes/origin/host/mac-mini/heartbeat"),
            Some("mac-mini")
        );
    }

    #[test]
    fn a_delivery_id_names_one_file() {
        // A second delivery of one response writes the same file (137).
        assert_eq!(response("discord/1001"), "responses/discord-1001.rec");
        assert_eq!(response("discord/1001"), response("discord/1001"));
    }
}
