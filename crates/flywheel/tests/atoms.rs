//! Every atom a machine guard reads is answered by a real binding (B.3, 138).
//!
//! The scenario runner's stand-in store answers every atom, so an acceptance
//! row that reads one no shipped binding answers is proving the harness and
//! not the binary: by construction it cannot fail. Three defects came out of
//! that gap in one day — `capture.source`, the cost contract's `observe()`,
//! `rail.status_current`. This is the gate that stops it coming back: a real
//! `Host` over the manifest `init` writes, asked for every atom every guard
//! names, on an object of the machine that names it.

use flywheel::host::Host;
use flywheel_engine::defs::{Guard, Machine, MachineKind, Region};
use flywheel_atoms::StateStore;
use flywheel_engine::runtime::EvidenceSource;
use flywheel_engine::{Definitions, Object};
use chrono::TimeZone;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

/// The atoms a guard names that this release does not answer, each with the
/// phase whose bindings answer it. A deferral is stated here or it is a
/// failure; one that a binding has since answered is a failure too, so the
/// list shrinks as the phases land.
const DEFERRED: &[(&str, &str)] = &[
    // 221's removal. Two of its three conjuncts are readable now — no session
    // record alive, no place under the instance root — but "its state
    // archived" names a place on the shared line the git-only layout does not
    // have, and `retire_instance`, whose proof this is, has no binding. Binding
    // the read would mean inventing where the archive lives; the model names
    // it first (AGENTS.md), and the effect and the read land together.
    ("instance.retired", "phase 1 — 221 removal; the layout names no archive and `retire_instance` is unbound"),
    // Construction (A.5): lines, places, stages, work items, services,
    // planning and proposals.
    ("bolt.services_declared", "phase 2 — construction (47)"),
    ("host.layout_current", "phase 2 — construction (186, 196)"),
    ("host.no_stray_places", "phase 2 — construction (55)"),
    ("host.stray_places", "phase 2 — construction (55)"),
    ("item.change_archived", "phase 2 — construction"),
    ("item.retry_max", "phase 2 — construction"),
    ("item.send_backs", "phase 2 — construction (41)"),
    ("line.conflict_job_done", "phase 2 — construction"),
    ("line.conflict_job_seeded", "phase 2 — construction"),
    ("line.policy", "phase 2 — construction"),
    ("line.request", "phase 2 — construction"),
    ("line.request_opened", "phase 2 — construction"),
    ("line.request_review_pending", "phase 2 — construction (176)"),
    ("line.request_review_recorded", "phase 2 — construction (176)"),
    ("line.retries", "phase 2 — construction"),
    ("line.take_conflicts", "phase 2 — construction"),
    ("place.conflict_retries", "phase 2 — construction"),
    ("place.held", "phase 2 — construction (55)"),
    ("place.job_seeded", "phase 2 — construction"),
    ("place.line_moved_told", "phase 2 — construction"),
    ("planning.fingerprint", "phase 2 — construction (28)"),
    ("planning.fingerprint_recorded", "phase 2 — construction (28)"),
    ("planning.proposal_recorded", "phase 2 — construction (172)"),
    ("planning.units_proposed", "phase 2 — construction (29)"),
    ("proposal.answer_forwarded", "phase 2 — construction (172)"),
    ("cell.verdict_recorded", "phase 2 — construction; the ledger is phase 3"),
    ("repository.covered_by_app", "phase 2 — construction (206)"),
    ("repository.exists", "phase 2 — construction (206)"),
    ("repository.registered", "phase 2 — construction (206)"),
    ("service.declared", "phase 2 — construction (47)"),
    ("service.endpoint_recorded", "phase 2 — construction (46)"),
    ("service.place_present", "phase 2 — construction (47)"),
    ("service.process", "phase 2 — construction (45)"),
    ("service.process_absent", "phase 2 — construction (45)"),
    ("service.serving", "phase 2 — construction (46)"),
    ("stage.agents", "phase 2 — construction"),
    ("stage.join_met", "phase 2 — construction"),
    ("stage.verdict", "phase 2 — construction"),
    // Context (A.14–A.16): claims by anchor, the ledger, packages.
    ("cell.challenged", "phase 3 — context; the ledger"),
    ("cell.claim_version", "phase 3 — context; the ledger"),
    ("cell.evidence_present", "phase 3 — context; the ledger"),
    ("cell.in_scope", "phase 3 — context; the ledger"),
    ("cell.verdict", "phase 3 — context; the ledger"),
    ("cell.verdict_claim_version", "phase 3 — context; the ledger"),
    ("claim.attached", "phase 3 — context (97)"),
    ("claim.detached", "phase 3 — context (97)"),
    ("claim.on_shared_line", "phase 3 — context (97)"),
    ("package.absent", "phase 3 — context; packages (229)"),
    ("package.configured", "phase 3 — context; packages (229)"),
    ("package.installed", "phase 3 — context; packages (229)"),
    ("package.secrets_placed", "phase 3 — context; packages (229)"),
    ("package.stopped", "phase 3 — context; packages (229)"),
    // Dispatch (A.25–A.30): users and ownership.
    ("rail.owner_recorded", "phase 4 — dispatch; ownership (237)"),
    // Scale: pools, enrolment and the provisioned host's disk.
    ("host.disk_layout", "phase 5 — scale; provisioned hosts (222)"),
    ("host.environment_satisfied", "phase 5 — scale; provisioned hosts"),
    ("host.layout_repaired", "phase 5 — scale; provisioned hosts (222)"),
    ("host.repositories_cloned", "phase 5 — scale; provisioned hosts"),
    ("host.secrets_placed", "phase 5 — scale; enrolment"),
    ("host.token_issued", "phase 5 — scale; enrolment"),
    ("pool.bound", "phase 5 — scale; pools (239)"),
    ("pool.cost_allows", "phase 5 — scale; pools"),
    ("pool.demand", "phase 5 — scale; pools"),
    ("pool.host_added", "phase 5 — scale; pools (242)"),
    ("pool.host_retired", "phase 5 — scale; pools (242)"),
    ("pool.idle_host", "phase 5 — scale; pools"),
    ("pool.image_current", "phase 5 — scale; pools (240)"),
    ("pool.live", "phase 5 — scale; pools (239)"),
];

fn dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "flywheel-atoms-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A host over the manifest `flywheel init` writes, so every binding the
/// binary loads is the one asked (D8).
fn hosted(name: &str) -> Host {
    let dir = dir(name);
    let report = flywheel::init::run(flywheel::init::Init {
        instance: "willdan".into(),
        host: "mac-mini".into(),
        root: dir.join("root"),
        git_host: dir.join("git-host"),
        app: "12345".into(),
        app_key_from: format!("FLYWHEEL_TEST_KEY_ATOMS_{}", name.to_uppercase()),
        app_key: Some("the operator placed this".into()),
        address: "http://laptop.example".into(),
        manifest: dir.join("flywheel.yaml"),
        repositories: vec![],
        curation: None,
        at: chrono::Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap(),
    })
    .expect("init runs");
    assert_eq!(report.state, "hosted", "{report:?}");
    // The host joins by one command: its clones and the checkout of every
    // shared line, the blueprints among them (205).
    let manifest = flywheel_world_host::Manifest::read(&dir.join("flywheel.yaml")).expect("the manifest");
    let mut world = flywheel_world_host::HostWorld::open(manifest, "mac-mini").expect("the world opens");
    flywheel_world_host::join::join(&mut world).expect("the host joins");
    let now = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 9, 0, 0).unwrap();
    Host::open(&dir.join("flywheel.yaml"), "mac-mini", None, now).expect("the host opens")
}

/// Where a guard reads an atom: the machine of the object it is asked about,
/// and the region path the engine hands the binding.
type Reads = BTreeMap<String, BTreeSet<(String, String)>>;

/// The capture and the signal the probe made through the catalogue, by id.
static CAPTURE: std::sync::OnceLock<(String, String)> = std::sync::OnceLock::new();

fn ev_names(guard: &Guard, into: &mut Vec<String>) {
    match guard {
        Guard::Ev(e) => {
            into.push(e.ev.clone());
            for other in [&e.eq_ev, &e.ne_ev, &e.gte_ev, &e.lt_ev, &e.gt_ev].into_iter().flatten() {
                into.push(other.clone());
            }
        }
        Guard::All { all } => all.iter().for_each(|g| ev_names(g, into)),
        Guard::Any { any } => any.iter().for_each(|g| ev_names(g, into)),
        Guard::Not { not } => ev_names(not, into),
        _ => {}
    }
}

/// Walk a machine's regions, into nested regions and into the templates its
/// states run, recording each atom against the enclosing object's machine and
/// the region path as the engine spells it (`tick::state_def`).
fn walk(defs: &Definitions, object_machine: &str, regions: &BTreeMap<String, Region>, prefix: &str, reads: &mut Reads, seen: &mut BTreeSet<String>) {
    for (name, region) in regions {
        let path = match prefix.is_empty() {
            true => name.clone(),
            false => format!("{prefix}.{name}"),
        };
        for (state_name, state) in &region.states {
            let mut names = vec![];
            for transition in &state.transitions {
                ev_names(&transition.when, &mut names);
                // An effect's proof is read by the engine before the effect is
                // performed: unanswered, the effect runs on every tick (73, 127).
                for effect in &transition.effects {
                    names.extend(defs.atoms.proof_of(&effect.name));
                }
            }
            for effect in state.entry.iter().chain(&state.exit) {
                names.extend(defs.atoms.proof_of(&effect.name));
            }
            for atom in names {
                reads.entry(atom).or_default().insert((object_machine.to_string(), path.clone()));
            }
            let below = format!("{path}.{state_name}");
            walk(defs, object_machine, &state.regions, &below, reads, seen);
            if let Some(template) = &state.machine {
                // A literal template by name; `$type` and its kin name whichever
                // type the record holds, so every template of that family is
                // walked (`elaboration.yaml` running, `work-item.yaml`).
                let run: Vec<&Machine> = match template.starts_with('$') {
                    false => defs.get(template).into_iter().collect(),
                    true => defs
                        .machines
                        .values()
                        .filter(|m| m.kind == MachineKind::Template)
                        .filter(|m| !matches!(m.machine.as_str(), "line" | "place" | "session" | "stage"))
                        .collect(),
                };
                for machine in run {
                    let key = format!("{object_machine}:{below}:{}", machine.machine);
                    if seen.insert(key) {
                        walk(defs, object_machine, &machine.regions, &below, reads, seen);
                    }
                }
            }
        }
    }
}

/// Every atom every guard reads, with where it is read.
fn atoms_read(defs: &Definitions) -> Reads {
    let mut reads = Reads::new();
    let mut seen = BTreeSet::new();
    for machine in defs.machines.values() {
        if machine.kind == MachineKind::Template {
            continue;
        }
        let kind = machine.object.clone().unwrap_or_else(|| machine.machine.clone());
        walk(defs, &kind, &machine.regions, "", &mut reads, &mut seen);
    }
    reads
}

/// One object of each machine, at the machine's initial state, with the id
/// shape and the record fields the bindings key on.
fn probe(defs: &Definitions, machine: &str, now: chrono::DateTime<chrono::Utc>) -> Object {
    let (id, record): (&str, Vec<(&str, serde_json::Value)>) = match machine {
        "bolt" => ("bolt/atlas/plan-rows", vec![
            ("repository", json!("atlas")),
            ("name", json!("plan-rows")),
            ("held_at", json!("2026-01-01T09:00:00Z")),
        ]),
        "unit" => ("unit/atlas/u", vec![
            ("repository", json!("atlas")),
            ("type", json!("default")),
            ("claims", json!(["one-writer@3"])),
        ]),
        "intent" => ("intent/atlas-provider-limits", vec![("close_declined_at", json!("2026-01-01T09:00:00Z"))]),
        "elaboration" => ("elaboration/atlas-provider-limits/1", vec![
            ("type", json!("standing")),
            ("kept_at", json!("2026-01-01T09:00:00Z")),
            ("covers", json!(["intent/atlas-provider-limits"])),
        ]),
        "capture" => (CAPTURE.get().map(|c| c.0.as_str()).unwrap_or("capture/page-1"), vec![("source", json!("page"))]),
        "signal" => (CAPTURE.get().map(|c| c.1.as_str()).unwrap_or("signal/page-1/1"), vec![("kind", json!("ask"))]),
        "curation" => ("curation/willdan", vec![("threshold", json!(12))]),
        "planning" => ("planning/atlas", vec![
            ("repository", json!("atlas")),
            ("planned_fingerprint", json!("f0")),
            ("redo_notes", json!("")),
        ]),
        "proposal" => ("proposal/atlas/1", vec![("repository", json!("atlas"))]),
        "operator-session" => ("operator-session/chuck-1", vec![("repository", json!("blueprints"))]),
        "instance" => ("instance/willdan", vec![("name", json!("willdan"))]),
        "work-item" => ("work-item/atlas/u/1", vec![("repository", json!("atlas")), ("ordinal", json!(1))]),
        "host" => ("host/mac-mini", vec![("bound", json!(4))]),
        "lease" => ("lease/bolt/atlas/plan-rows", vec![]),
        "rail" => (flywheel_domain::RAIL, vec![]),
        "response" => ("response/page-7", vec![]),
        "sink" => ("sink/page", vec![("kind", json!("page"))]),
        "claim" => ("claim/atlas/one-writer", vec![("repository", json!("atlas"))]),
        "ledger-cell" => ("ledger-cell/atlas/c1", vec![("repository", json!("atlas"))]),
        "repository" => ("repository/atlas", vec![("name", json!("atlas"))]),
        "service" => ("service/atlas/plan-rows/web", vec![("repository", json!("atlas"))]),
        other => (&*Box::leak(format!("{other}/atlas/x").into_boxed_str()), vec![]),
    };
    let mut object = Object {
        id: id.to_string(),
        machine: machine.to_string(),
        parent: match machine {
            "elaboration" => Some("intent/atlas-provider-limits".into()),
            "work-item" => Some("unit/atlas/u".into()),
            "signal" => Some("capture/page-1".into()),
            "unit" => Some("bolt/atlas/plan-rows".into()),
            _ => None,
        },
        config: Default::default(),
        entered_at: Default::default(),
        record: record.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        counters: Default::default(),
        applied_responses: vec![],
        seq: 0,
        created: 0,
    };
    flywheel_engine::initialise(defs, &mut object, now);
    object
}

/// Every atom a machine guard names is answered by a shipped binding on a
/// real host, or is deferred here to a named phase — never answered only by
/// the scenario runner's stand-in (B.3, 87, 138, D8; audit 8, 14).
#[test]
fn every_atom_a_guard_reads_has_a_real_binding() {
    let mut host = hosted("parity");
    let defs = host.defs.clone();
    let now = host.now();
    let reads = atoms_read(&defs);
    assert!(reads.len() > 50, "the walk found {} atoms; the machines read more than that", reads.len());
    // Every name a guard reads is an atom of `atoms.yaml` (87, I13).
    for atom in reads.keys() {
        assert!(defs.atoms.evidence.contains_key(atom), "`{atom}` is read by a guard and is no atom");
    }

    // The world with one of everything a binding reads: this host's own
    // heartbeat and record, a lease it holds, and a capture with its signal
    // written into the blueprints through the catalogue, as the page's box
    // writes one (19, 111, 128, 147).
    host.declare().expect("the host declares");
    let call = flywheel_surface::catalogue::Call::new("capture", "chuck", "page")
        .arg("text", json!("the rows lose their numbers on the second page"))
        .arg("source", json!("page"));
    host.store
        .with_world(|store, world| flywheel_surface::catalogue::call(&mut store.git, world, &defs, &call))
        .expect("a capture through the catalogue");
    let (capture, signal) = host.store.with_world(|_, world| {
        let captures = flywheel_domain::signals::captures(world).expect("the captures read");
        let key = captures.first().expect("the capture is in the blueprints").key.clone();
        (
            flywheel_domain::signals::object_of(&key),
            flywheel_domain::signals::signal_object(&key, 1),
        )
    });
    CAPTURE.set((capture, signal)).unwrap();

    // One object per machine, on the shared line, so a binding that reads the
    // object has one to read. The host's own record is the one it declared.
    let machines: BTreeSet<String> = reads.values().flatten().map(|(m, _)| m.clone()).collect();
    let objects: Vec<Object> = machines
        .iter()
        .filter(|m| !matches!(m.as_str(), "host" | "capture" | "signal" | "rail"))
        .map(|m| probe(&defs, m, now))
        .collect();
    host.store.git.seed_objects(&objects).expect("the probes are seeded");
    host.store
        .git
        .lease(&flywheel_atoms::LeaseOp::Take {
            object: "bolt/atlas/plan-rows".into(),
            holder: "mac-mini".into(),
        })
        .expect("a lease is taken");
    host.store.git.fetch().expect("the shared line");

    let mut answered = BTreeSet::new();
    let mut unanswered: BTreeMap<String, BTreeSet<(String, String)>> = BTreeMap::new();
    for (atom, where_read) in &reads {
        let any = where_read.iter().any(|(machine, region)| {
            let object = probe(&defs, machine, now);
            host.store.evidence(&object.id, region, atom).is_some()
        });
        match any {
            true => {
                answered.insert(atom.clone());
            }
            false => {
                unanswered.insert(atom.clone(), where_read.clone());
            }
        }
    }

    let deferred: BTreeMap<&str, &str> = DEFERRED.iter().copied().collect();
    let gap: Vec<String> = unanswered
        .iter()
        .filter(|(atom, _)| !deferred.contains_key(atom.as_str()))
        .map(|(atom, where_read)| {
            let sites: Vec<String> = where_read.iter().map(|(m, r)| format!("{m} {r}")).collect();
            format!("{atom} — read at {}", sites.join(", "))
        })
        .collect();
    let stale: Vec<String> = deferred
        .keys()
        .filter(|atom| answered.contains(**atom))
        .map(|a| a.to_string())
        .collect();
    let phantom: Vec<String> = deferred
        .keys()
        .filter(|atom| !reads.contains_key(**atom))
        .map(|a| a.to_string())
        .collect();
    assert!(
        gap.is_empty(),
        "{} atom(s) a guard reads are answered by no shipped binding — only the scenario runner's stand-in answers them, so every scenario reading one proves the harness and not the binary:\n  {}\n\n({} answered, {} deferred)",
        gap.len(),
        gap.join("\n  "),
        answered.len(),
        deferred.len()
    );
    assert!(stale.is_empty(), "deferred atoms a binding now answers; take them off the list: {stale:?}");
    assert!(phantom.is_empty(), "deferred atoms no guard reads: {phantom:?}");
}
