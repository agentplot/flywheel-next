use chrono::{TimeZone, Utc};
use flywheel_atoms::{HostRecord, LeaseRecord, ThreadEntry};
use flywheel_domain::{envelope, records};
use flywheel_engine::runtime::{Object, Response, ResponseKind};
use serde_json::json;

fn an_object() -> Object {
    let at = Utc.with_ymd_and_hms(2026, 9, 8, 12, 0, 0).unwrap();
    let mut o = Object {
        id: "elaboration/willdan/7".into(),
        machine: "elaboration".into(),
        parent: Some("intent/willdan/3".into()),
        config: Default::default(),
        entered_at: Default::default(),
        record: Default::default(),
        counters: Default::default(),
        applied_responses: vec!["discord/1001".into(), "page/2".into()],
        seq: 12,
        created: 4,
    };
    o.config.insert("life".into(), "approved".into());
    o.config.insert("life.approved.session".into(), "alive".into());
    o.entered_at.insert("life".into(), at);
    o.entered_at.insert("life.approved.session".into(), at);
    o.record.insert("type".into(), json!("self-closing"));
    o.record.insert("type_version".into(), json!(1));
    // A field whose value is a string that reads as a number must come back a
    // string, which is why the envelope encodes record fields as JSON.
    o.record.insert("note".into(), json!("123"));
    o.record.insert("covers".into(), json!(["intent/willdan/3"]));
    o.counters.insert("attempts".into(), 2);
    o
}

#[test]
fn envelope_roundtrip() {
    let o = an_object();
    let text = envelope::write_all(std::slice::from_ref(&o));
    let back = envelope::read_all(&text).expect("the envelope reads back");
    assert_eq!(back.len(), 1);
    let b = &back[0];
    assert_eq!(b.id, o.id);
    assert_eq!(b.machine, o.machine);
    assert_eq!(b.parent, o.parent);
    assert_eq!(b.config, o.config);
    assert_eq!(b.entered_at, o.entered_at);
    assert_eq!(b.record, o.record);
    assert_eq!(b.counters, o.counters);
    assert_eq!(b.applied_responses, o.applied_responses);
    assert_eq!(b.seq, o.seq);
    assert_eq!(b.created, o.created);
    // Written twice, the same bytes: the record is the fact and nothing else.
    assert_eq!(envelope::write_all(&back), text);
}

#[test]
fn the_envelope_fields_are_closed() {
    // Anything not an envelope field is the object's own and is carried
    // through untouched, so a new type needs no change here (57, 85).
    let o = an_object();
    let r = envelope::to_record(&o);
    let own: Vec<&str> = r
        .fields
        .iter()
        .map(|(k, _)| k.as_str())
        .filter(|k| !envelope::ENVELOPE_FIELDS.contains(k))
        .collect();
    assert_eq!(own, vec!["covers", "note", "type", "type_version"]);
}

#[test]
fn response_thread_lease_and_host_roundtrip() {
    let at = Utc.with_ymd_and_hms(2026, 9, 8, 12, 0, 0).unwrap();

    let response = Response {
        id: "discord/1001".into(),
        kind: ResponseKind::Answer,
        decision: Some(412),
        object: Some("elaboration/willdan/7".into()),
        answer: "yes".into(),
        given_by: "chuck".into(),
        given_at: at,
        delivery: "discord/1001".into(),
    };
    let back = records::response_from_record(&records::response_to_record(&response)).unwrap();
    assert_eq!(back.id, response.id);
    assert_eq!(back.decision, response.decision);
    assert_eq!(back.given_by, response.given_by);
    assert_eq!(back.given_at, response.given_at);

    let mut entry = ThreadEntry {
        at,
        kind: "exit".into(),
        by: Some("chuck".into()),
        fields: Default::default(),
    };
    entry.fields.insert("exit".into(), json!("done"));
    let back = records::thread_from_record(&records::thread_to_record(&entry)).unwrap();
    assert_eq!(back.kind, entry.kind);
    assert_eq!(back.by, entry.by);
    assert_eq!(back.fields, entry.fields);

    let lease = LeaseRecord {
        object: "unit/atlas/x".into(),
        holder: "mac-mini".into(),
        taken_at: at,
        renewed_at: at,
    };
    let back = records::lease_from_record(&records::lease_to_record(&lease)).unwrap();
    assert_eq!(back.holder, lease.holder);
    assert_eq!(back.renewed_at, lease.renewed_at);

    let host = HostRecord {
        host: "mac-mini".into(),
        last_seen: at,
        bound: 3,
        intermittent: true,
    };
    let back = records::host_from_record(&records::host_to_record(&host)).unwrap();
    assert_eq!(back.host, host.host);
    assert_eq!(back.bound, host.bound);
    assert!(back.intermittent);
}
