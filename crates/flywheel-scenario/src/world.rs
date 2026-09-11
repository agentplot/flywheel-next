//! Performing effects in the stand-in world. Each effect is bound by name to
//! a small simulation; an unbound effect is logged and counts as done.

use crate::store::{place_key, ServiceFact, SessionFact, Store};
use flywheel_atoms::Workspace;
use flywheel_engine::runtime::Object;
use flywheel_engine::{Definitions, PlannedEffect};
use serde_json::{json, Value};

/// Perform one effect, and say whether the world actually did anything.
///
/// A retry the world refuses is not a second act: the pane and the agent are
/// named by the session id, so a second start of the same name is refused by
/// the multiplexer and the machinery reads the refusal (72). The count a
/// scenario asserts is of acts, not of attempts.
pub fn perform(defs: &Definitions, store: &mut Store, object: &str, region: &str, e: &PlannedEffect) -> bool {
    let text = format!("{}{}", e.name, e.note.as_ref().map(|n| format!(" — {n}")).unwrap_or_default());
    store.log("effect", object, text);
    let mut acted = true;
    let skey = store.session_of(object, region);
    let pkey = place_key(object, region);
    match e.name.as_str() {
        // The effects of 42 are the recorded workspace's, so there is one
        // implementation of them and the runner and the host share it (93a, D8).
        "prepare_place" => { let _ = crate::bindings::RecordedWorkspace::new(store).prepare_place(&pkey, object, ""); }
        "rebase_place" | "tell_moved" | "seed_conflict_job" | "record_endpoints" => {
            if let Some(p) = store.world.places.get_mut(&pkey) { p.endpoints_recorded = true; }
        }
        "merge_place" => { let _ = crate::bindings::RecordedWorkspace::new(store).merge_place(&pkey); }
        "remove_place" => { let _ = crate::bindings::RecordedWorkspace::new(store).remove_place(&pkey); }
        "create_line" => { if !store.world.lines.contains_key(object) { let _ = crate::bindings::RecordedWorkspace::new(store).create_line(object, ""); } }
        "take_parent" => { let _ = crate::bindings::RecordedWorkspace::new(store).take_parent(object); }
        "land_line" => { let _ = crate::bindings::RecordedWorkspace::new(store).land_line(object, flywheel_atoms::LandingPolicy::Direct); }
        "remove_line" => { store.world.lines.entry(object.to_string()).or_default().absent = true; }
        "start_session" => {
            let s = store.world.sessions.entry(skey.clone()).or_insert_with(|| SessionFact { pane: false, activity: "working".into(), ..Default::default() });
            if !s.pane { s.pane = true; s.activity = "working".into(); s.exit = None; s.ticks_alive = 0; s.played.clear(); }
            else {
                // A second start of the same name is refused by the
                // multiplexer, and the machinery reads the refusal (72).
                acted = false;
                store.duplicate_starts += 1;
            }
            store.play_scripts_for(&skey);
        }
        "end_session" => { if let Some(s) = store.world.sessions.get_mut(&skey) { s.pane = false; } }
        "deliver_answer" => {
            let ans = e.args.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if let Some(s) = store.world.sessions.get_mut(&skey) { s.inbox.push(ans); s.exit = None; s.activity = "working".into(); s.question = None; }
            store.play_after_answer(&skey);
        }
        "create_items" => {
            if store.objects.values().any(|o| o.parent.as_deref() == Some(object) && o.machine == "work-item") { return true; }
            let unit = store.objects.get(object).cloned();
            let n = unit.as_ref().and_then(|u| u.record.get("items")).and_then(|v| v.as_u64()).unwrap_or(2) as usize;
            let ty = unit.as_ref().and_then(|u| u.record.get("type").cloned()).unwrap_or(json!("default"));
            let serial = unit.as_ref().and_then(|u| u.record.get("serial")).and_then(|v| v.as_bool()).unwrap_or(false);
            for i in 1..=n {
                let id = format!("{object}/wi-{i}");
                let mut rec = serde_json::Map::new();
                rec.insert("ordinal".into(), json!(i));
                rec.insert("type".into(), ty.clone());
                if serial && i > 1 { rec.insert("depends_on".into(), json!([format!("{object}/wi-{}", i - 1)])); }
                new_object(defs, store, &id, "work-item", Some(object), rec.into_iter().collect());
            }
        }
        "create_bolt" => {
            let Some(unit) = store.objects.get(object).cloned() else { return true };
            let target = unit.record.get("target").cloned().unwrap_or(Value::Null);
            let has_bolt = target.get("bolt").and_then(|v| v.as_str()).map(|s| !s.is_empty()).unwrap_or(false);
            if has_bolt { return true; }
            let name = target.get("new_name").and_then(|v| v.as_str()).unwrap_or("new-bolt").to_string();
            let repo = unit.record.get("repository").and_then(|v| v.as_str()).unwrap_or("repo").to_string();
            let bid = format!("bolt/{repo}/{name}");
            if !store.objects.contains_key(&bid) {
                let rec = [("repository".to_string(), json!(repo)), ("name".to_string(), json!(name))].into_iter().collect();
                new_object(defs, store, &bid, "bolt", None, rec);
            }
            amend(store, object, |u| {
                u.parent = Some(bid.clone());
                let mut t = target.as_object().cloned().unwrap_or_default();
                t.insert("bolt".into(), json!(bid));
                u.record.insert("target".into(), Value::Object(t));
            });
        }
        "route_unit" => {
            let b = e.args.get("bolt").and_then(|v| v.as_str()).unwrap_or("").to_string();
            amend(store, object, |u| {
                let mut t = u.record.get("target").and_then(|v| v.as_object().cloned()).unwrap_or_default();
                let existing = store_bolt_by_name(&store_snapshot_bolts(&t, &b));
                if let Some(bid) = existing { t.insert("bolt".into(), json!(bid)); t.remove("new_name"); } else { t.insert("new_name".into(), json!(b)); t.remove("bolt"); }
                u.record.insert("target".into(), Value::Object(t));
            });
        }
        "rename_bolt" => {
            let name = e.args.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            amend(store, object, |u| {
                let mut t = u.record.get("target").and_then(|v| v.as_object().cloned()).unwrap_or_default();
                t.insert("new_name".into(), json!(name));
                u.record.insert("target".into(), Value::Object(t));
            });
        }
        "set_type" => {
            let ty = e.args.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
            amend(store, object, |u| { u.record.insert("type".into(), json!(ty)); });
        }
        "archive_intent" | "archive_change" => { store.world.archived.insert(object.to_string(), true); }
        "declare_services" => {
            let Some(bolt) = store.objects.get(object).cloned() else { return true };
            let repo = bolt.record.get("repository").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let decls = store.world.declarations.get(&repo).cloned().unwrap_or_default();
            let live = |store: &Store, id: &str| store.objects.get(id).and_then(|o| o.top_state()).map(|s| s != "gone").unwrap_or(false);
            for d in decls {
                let id = Store::service_id(object, &d.name);
                if live(store, &id) { continue; }
                let rec = [("name", json!(d.name)), ("command", json!(d.command)), ("serves", json!(d.serves))].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
                new_object(defs, store, &id, "service", Some(object), rec);
            }
        }
        "start_service" => {
            let moved_by = store.objects.get(object).and_then(|o| o.applied_responses.last().cloned());
            let tick = store.tick;
            let f = store.world.services.entry(object.to_string()).or_default();
            if f.process != "present" { *f = ServiceFact { process: "present".into(), started_tick: tick, ..Default::default() }; }
            if let Some(r) = moved_by {
                amend(store, object, |o| { o.record.insert("moved_by".into(), json!(r)); });
            }
        }
        "stop_service" => {
            let moved_by = store.objects.get(object).and_then(|o| o.applied_responses.last().cloned());
            store.world.services.insert(object.to_string(), ServiceFact { process: "absent".into(), ..Default::default() });
            amend(store, object, |o| {
                o.record.remove("endpoint");
                if let Some(r) = moved_by { o.record.insert("moved_by".into(), json!(r)); }
            });
        }
        "record_service_endpoint" => {
            let ep = store.world.services.get(object).and_then(|f| f.endpoint.clone());
            amend(store, object, |o| {
                match ep { Some(e) => { o.record.insert("endpoint".into(), json!(e)); } None => { o.record.remove("endpoint"); } }
            });
        }
        // The status projection, written from `list` and `get` alone and
        // stating the point it is as of (132, 145, D12). It is the rail's own
        // effect, and the tick writes it again whenever what it projects moved.
        "render_status" => store.write_status(defs),
        // Every standing decision without a number takes the next one, in one
        // atomic write of the register and the counter (15, `engine/rail.yaml`
        // number_decisions).
        "number_decisions" => store.number_decisions(),
        // One record per uncited offer, pointing at its document and citing
        // the thread entry it came from; the session is not interrupted (58,
        // 62, I5).
        "record_offers" => {
            let at = store.now;
            let session = skey.clone();
            let defs = defs.clone();
            let _ = flywheel_domain::offers::record(store, &defs, &session, object, at);
        }
        // A capture that is its own excerpt writes its one signal here, and
        // never a second: no judgment is involved (19, 112, `atoms.yaml`
        // ensure_signal).
        "ensure_signal" => {
            let at = store.now;
            let by = store
                .objects
                .get(object)
                .and_then(|o| o.record.get("captured_by"))
                .and_then(|v| v.as_str())
                .unwrap_or("operator")
                .to_string();
            let _ = crate::bindings::with_files(store, |store, world| {
                flywheel_domain::signals::ensure_signal(store, world, defs, object, &by, at)
            });
        }
        // Every judged signal gets its one standing move, and each join becomes
        // or grows a proposed intent (107, 109, 116, `curation.yaml` applying).
        // The moves are the session's delivery: curation decides and the
        // flywheel accepts its output whoever produced it (20, 110).
        "record_moves" | "propose_intents" => {
            let at = store.now;
            let commits = store
                .world
                .sessions
                .get(&skey)
                .map(|s| s.commits.clone())
                .unwrap_or_default();
            // The capture this run read, for numbering the signals a script
            // counted rather than named (93).
            let key = store
                .objects
                .values()
                .find(|o| o.machine == "capture")
                .and_then(|o| o.record.get("event_key").and_then(|v| v.as_str()))
                .map(String::from)
                .unwrap_or_else(|| "curation".to_string());
            let delivered = crate::delivery::curation(&commits, &key, &at.to_rfc3339());
            match e.name.as_str() {
                "record_moves" => {
                    let _ = crate::bindings::with_files(store, |store, world| {
                        flywheel_domain::signals::record_moves(store, world, &delivered.moves, at)
                    });
                }
                _ => {
                    let _ = flywheel_domain::signals::propose_intents(
                        store,
                        defs,
                        &delivered.proposals,
                        at,
                    );
                }
            }
        }
        // Every signal a dropped intent cited keeps a move naming the drop, and
        // they are not clustered again unless new signals join them (117).
        "drop_signals" => {
            let at = store.now;
            let reason = e
                .args
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or("intent dropped")
                .to_string();
            let _ = crate::bindings::with_files(store, |store, world| {
                flywheel_domain::signals::drop_signals(store, world, defs, object, &reason, at)
            });
        }
        "record_exit" => {
            if let Some(s) = store.world.sessions.get(&skey) {
                if let Some(q) = &s.question {
                    let q = q.clone();
                    amend(store, object, |o| { o.record.insert("question".into(), json!(q)); });
                }
            }
        }
        _ => {}
    }
    acted
}

fn store_snapshot_bolts(_t: &serde_json::Map<String, Value>, name: &str) -> String { name.to_string() }
fn store_bolt_by_name(name: &str) -> Option<String> { if name.starts_with("bolt/") { Some(name.to_string()) } else { None } }

/// An object an effect made, written through the store.
///
/// Through `put` and never into the map alone: the store is the state
/// repository, and an object only this process held would be gone at the next
/// fetch, so the effect would fire again on every tick for ever (92, 125, 131).
/// Change an object's record through the store.
///
/// An effect's write is a write like any other: the store is the state
/// repository, and what only this process held would be gone at the next fetch.
/// The transition's own commit is already made by the time an effect runs, so
/// what the effect changes is its own to write (92, 125, 127, D4).
fn amend(store: &mut Store, id: &str, change: impl FnOnce(&mut Object)) {
    let Ok(Some(mut held)) = flywheel_atoms::Records::get(store, id) else {
        return;
    };
    let seq = held.seq;
    let before = held.record.clone();
    change(&mut held);
    // Writing what is already there is not a write (127).
    if held.record == before {
        return;
    }
    let _ = flywheel_atoms::Records::put(store, id, &held, seq);
}

pub fn new_object(defs: &Definitions, store: &mut Store, id: &str, machine: &str, parent: Option<&str>, record: std::collections::BTreeMap<String, Value>) {
    let mut o = Object { id: id.to_string(), machine: machine.to_string(), parent: parent.map(String::from), config: Default::default(), entered_at: Default::default(), record, counters: Default::default(), applied_responses: vec![], seq: 0, created: store.next_created };
    flywheel_engine::initialise(defs, &mut o, store.now);
    store.log("create", id, format!("{machine} created"));
    // Before a repository is bound the store is being seeded, and what seeding
    // put in the map is written into the repository the moment it is bound.
    if store.durable().is_none() || std::env::var("FLYWHEEL_NEW_MAP").is_ok() {
        store.next_created += 1;
        store.objects.insert(id.to_string(), o);
        return;
    }
    if let Err(e) = flywheel_atoms::Records::put(store, id, &o, 0) {
        store.log("refused", id, format!("{machine} could not be written: {e:#}"));
    }
}

impl Store {
    pub fn play_scripts_for(&mut self, key: &str) {
        let now = self.now;
        let script = self.world.script.get(key).cloned().unwrap_or_default();
        let Some(s) = self.world.sessions.get_mut(key) else { return };
        for (i, e) in script.iter().enumerate() {
            if s.played.contains(&i) { continue; }
            if matches!(e.after.as_deref(), None | Some("") | Some("0") | Some("0t")) {
                crate::store::apply_entry_pub(s, e, now);
                s.played.push(i);
            }
        }
    }
}
