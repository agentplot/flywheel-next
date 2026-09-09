//! The chat sink: one presenter, the same numbers as the page, the two message
//! shapes it accepts, its own mark and the kinds routed to it
//! (`surfaces/chat-sink`, D9).

mod store;
mod world;

use flywheel_atoms::Records;
use flywheel_domain::commands;
use flywheel_engine::Definitions;
use flywheel_store_git::GitStore;
use flywheel_surface::chat::{Chat, Heard, Message, Recorded};
use flywheel_domain::signals;
use flywheel_domain::sinks::{self, Spec};
use serde_json::json;
use std::collections::BTreeSet;

/// Where the page is served from in these tests: a private-network name with
/// the instance in the path, never a localhost port (205a, 308, D10a).
const ADDRESS: &str = "http://studio.tailnet.ts.net/willdan";

/// A store with a chat sink this host presents, and the definitions behind it.
fn a_chat(name: &str) -> (store::Sandbox, GitStore, world::Files, Definitions, Chat<Recorded>) {
    let sandbox = store::Sandbox::new(name);
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let mut store = sandbox.store();
    let sink = sinks::ensure(
        &mut store,
        &defs,
        &Spec::chat("chat-chuck", "chuck", "#willdan"),
    )
    .expect("the chat sink");
    let mut chat = Chat::new(&sink.id, "studio", ADDRESS, Recorded::new());
    assert!(chat.present(&mut store).expect("the presenter lease"));
    (sandbox, store, world::Files::new(), defs, chat)
}

/// A bolt whose close is offered: one decision, standing (22, 39).
fn a_decision(store: &mut GitStore, defs: &Definitions, id: &str) {
    let at = commands::now(store).expect("a point");
    commands::put_new(
        store,
        defs,
        id,
        "bolt",
        None,
        [("repository".to_string(), json!("atlas"))]
            .into_iter()
            .collect(),
        at,
    )
    .expect("the bolt");
    let mut bolt = Records::get(store, id).expect("a read").expect("the bolt");
    bolt.config.insert("life".into(), "open".into());
    bolt.config.insert("life.open.close".into(), "offered".into());
    let base = bolt.seq;
    Records::put(store, id, &bolt, base).expect("the bolt with its close offered");
    commands::rail(store, defs).expect("the rail derives");
}

/// Exactly one presenter delivers to each sink at a time (148). Two hosts could
/// present the chat; the holder of the sink's lease delivers and the other does
/// not, so the operator sees each delivery once.
#[test]
fn one_presenter_per_sink() {
    let sandbox = store::Sandbox::new("one-presenter");
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let mut store = sandbox.store();

    let sink = sinks::ensure(
        &mut store,
        &defs,
        &Spec::chat("chat-chuck", "chuck", "#willdan"),
    )
    .expect("the chat sink the manifest names");
    assert_eq!(sink.kind, "chat");
    assert_eq!(sink.member.as_deref(), Some("chuck"));

    // Nobody holds it yet, so nobody presents it: a presenter is taken, never
    // assumed (148).
    assert_eq!(
        sinks::presenter(&store, &sink.id).expect("a read"),
        None,
        "a sink nobody holds has a presenter"
    );
    assert!(!sinks::presents(&store, &sink.id, "studio").expect("a read"));

    // One host takes it.
    let mut studio = Chat::new(&sink.id, "studio", ADDRESS, Recorded::new());
    assert!(studio.present(&mut store).expect("the lease"));
    assert_eq!(
        sinks::presenter(&store, &sink.id).expect("a read").as_deref(),
        Some("studio")
    );

    // The other asks for the same lease and does not get it. It is not refused
    // an answer — it learns it lost and moves on (128, 148).
    let mut mac = Chat::new(&sink.id, "mac-mini", ADDRESS, Recorded::new());
    assert!(
        !mac.present(&mut store).expect("the lease"),
        "two hosts both present the one sink"
    );
    assert_eq!(
        sinks::presenter(&store, &sink.id).expect("a read").as_deref(),
        Some("studio"),
        "the second host took a lease the first held"
    );

    // The presenter gives it up and the other may take it: one at a time, and
    // never one for good (148).
    sinks::release_presenter(&mut store, &sink.id, "studio").expect("released");
    assert!(mac.present(&mut store).expect("the lease"));
    assert!(sinks::presents(&store, &sink.id, "mac-mini").expect("a read"));
    assert!(!sinks::presents(&store, &sink.id, "studio").expect("a read"));
}

/// 148 gives two ways to hold a presenter — a lease, or the manifest's pin —
/// and this release binds the lease alone. A sink carrying a pin is refused and
/// said so, rather than delivered to by whoever happens to hold the lease.
#[test]
fn a_manifest_pin_is_refused_rather_than_half_honoured() {
    let sandbox = store::Sandbox::new("pinned");
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let mut store = sandbox.store();
    let sink = sinks::ensure(
        &mut store,
        &defs,
        &Spec::chat("chat-chuck", "chuck", "#willdan"),
    )
    .expect("the chat sink");

    // The operator pinned it in the manifest.
    let mut object = flywheel_atoms::Records::get(&store, &sink.id)
        .expect("a read")
        .expect("the sink");
    let base = object.seq;
    object
        .record
        .insert("pinned_to".into(), serde_json::json!("studio"));
    flywheel_atoms::Records::put(&mut store, &sink.id, &object, base).expect("written");

    let refused = sinks::presenter(&store, &sink.id).expect_err("a pin this release does not bind");
    assert!(
        refused.to_string().contains("pinned to `studio`"),
        "{refused}"
    );
    assert!(refused.to_string().contains("148"), "{refused}");
}

// ---- 8.2 one line per decision, with its number and a link

/// A chat rendering carries the same decisions and numbers as the page, one
/// line each, with a link to the page (18, 15).
#[test]
fn chat_and_page_show_one_number() {
    let (_sandbox, mut store, _world, defs, chat) = a_chat("one-number");
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");
    a_decision(&mut store, &defs, "bolt/atlas/drop-the-tail");

    let post = chat.rendering(&mut store, &defs).expect("a rendering");
    let decisions = commands::rail(&mut store, &defs).expect("the rail");
    assert_eq!(
        post.lines.len(),
        decisions.len(),
        "one line per decision, and no more: {post:?}"
    );

    // The numbers the register gave are the numbers the chat shows, and they
    // are the page's own (15, 18).
    let page = flywheel_surface::page::read(&mut store, &defs, ADDRESS, "chuck").expect("the page");
    let on_page: Vec<u32> = page.decisions.iter().filter_map(|d| d.number).collect();
    let in_chat: Vec<u32> = post.lines.iter().filter_map(|l| l.number).collect();
    assert!(!in_chat.is_empty(), "the chat carried no numbered decision");
    assert_eq!(in_chat, on_page, "the chat and the page number differently");

    for line in &post.lines {
        let number = line.number.expect("a decision line carries its number");
        assert!(
            line.text().contains(&format!("#{number}")),
            "the line does not show its number: {}",
            line.text()
        );
        // A link to that object on the page, at the host's address with the
        // instance in the path (308, 205a).
        assert!(
            line.link.starts_with(ADDRESS) && line.link.ends_with(&line.object),
            "the line links at `{}`",
            line.link
        );
    }
    // And the rendering itself links to the page (18).
    assert_eq!(post.link, ADDRESS);
    assert!(post.text().contains(ADDRESS));
}

/// Each kind keeps its form in one line: a decision line is answerable and no
/// other line is (18, 209).
#[test]
fn only_a_decision_line_is_answerable() {
    let (_sandbox, mut store, _world, defs, chat) = a_chat("answerable-lines");
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");

    // A signal, which is never a decision, and an intent that reached its tail
    // state, which is a tail entry and not a decision either (14, 209).
    let at = commands::now(&store).expect("a point");
    commands::put_new(&mut store, &defs, "signal/s1", "signal", None, Default::default(), at)
        .expect("the signal");
    commands::put_new(&mut store, &defs, "intent/rows", "intent", None, Default::default(), at)
        .expect("the intent");
    let mut intent = Records::get(&store, "intent/rows")
        .expect("a read")
        .expect("the intent");
    intent.config.insert("life".into(), "closed".into());
    intent
        .entered_at
        .insert("life".into(), at + chrono::Duration::seconds(1));
    let base = intent.seq;
    Records::put(&mut store, "intent/rows", &intent, base).expect("the intent closed");

    let post = chat.rendering(&mut store, &defs).expect("a rendering");
    assert!(
        post.lines.iter().all(|l| l.answerable()),
        "a line that is not a decision offers an answer: {:?}",
        post.lines
    );
    assert!(
        post.tail.iter().all(|l| !l.answerable()),
        "a tail line offers an answer: {:?}",
        post.tail
    );
    assert!(
        post.tail.iter().any(|l| l.object == "intent/rows"),
        "the intent that closed is not in the tail: {:?}",
        post.tail
    );
    assert!(
        !post.lines.iter().any(|l| l.object == "signal/s1"),
        "a signal is on the rail"
    );
}

// ---- 8.3 the numbered reply grammar, and "yes all"

/// The grammar reads a number and a word and nothing more; a message outside it
/// is not guessed at (194).
#[test]
fn the_reply_grammar_is_a_grammar_and_not_an_interpreter() {
    use flywheel_surface::chat::{read_grammar, Grammar};
    assert_eq!(
        read_grammar("yes 412"),
        Grammar::Answer { number: 412, answer: "yes".into() }
    );
    assert_eq!(
        read_grammar("412 yes"),
        Grammar::Answer { number: 412, answer: "yes".into() }
    );
    assert_eq!(
        read_grammar("412"),
        Grammar::Answer { number: 412, answer: "yes".into() }
    );
    assert_eq!(
        read_grammar("412: rows lose their numbers on the second page"),
        Grammar::Answer {
            number: 412,
            answer: "rows lose their numbers on the second page".into()
        }
    );
    assert_eq!(read_grammar("yes all"), Grammar::All { answer: "yes".into() });
    for free in [
        "approve everything that looks fine",
        "can you close the bolt",
        "yes to the rows one",
        "",
        "no. 412 is wrong",
    ] {
        assert_eq!(
            read_grammar(free),
            Grammar::Unaccepted,
            "`{free}` was read as something"
        );
    }
}

/// "Yes to all" is one response per decision, each on its own delivery, so any
/// one of them could have been answered alone and each takes effect once
/// (11, 137, 194).
#[test]
fn yes_all_expands_per_decision() {
    let (_sandbox, mut store, mut world, defs, mut chat) = a_chat("yes-all");
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");
    a_decision(&mut store, &defs, "bolt/atlas/drop-the-tail");
    let standing = commands::rail(&mut store, &defs).expect("the rail");
    assert_eq!(standing.len(), 2, "two decisions stand: {standing:?}");

    let message = Message::new("1500", "chuck", "yes all");
    let heard = chat.receive(
&mut store,
&mut world,
&defs, &message).expect("read");
    let given = match heard {
        Heard::Answered(given) => given,
        other => panic!("`yes all` was heard as {other:?}"),
    };
    assert_eq!(given.len(), 2, "one response per decision, not one for all");

    // Distinct delivery identities, so the same message twice is still one
    // response each (137).
    let ids: BTreeSet<String> = given.iter().map(|c| c.id.clone()).collect();
    assert_eq!(ids.len(), 2, "the two responses share one delivery: {ids:?}");

    // Each is a response of its own, naming its own decision and the operator
    // who gave it (153).
    let numbers: BTreeSet<u64> = ids
        .iter()
        .map(|id| {
            let record = Records::get(&store, &format!("response/{id}"))
                .expect("a read")
                .unwrap_or_else(|| panic!("`{id}` wrote no response"));
            assert_eq!(record.record.get("tool").and_then(|v| v.as_str()), Some("answer"));
            assert_eq!(
                record.record.get("given_by").and_then(|v| v.as_str()),
                Some("chuck")
            );
            record
                .record
                .get("decision")
                .and_then(|v| v.as_u64())
                .expect("its own number")
        })
        .collect();
    let expected: BTreeSet<u64> = standing
        .iter()
        .filter_map(|d| d.number.map(u64::from))
        .collect();
    assert_eq!(numbers, expected, "the responses do not name the two decisions");

    // The same message again writes nothing new: each delivery takes effect
    // once (137, I2).
    let before = Records::list_records(&store, &flywheel_atoms::Scope::Machine("response".into()))
        .expect("a listing")
        .len();
    chat.receive(
&mut store,
&mut world,
&defs, &message).expect("read again");
    let after = Records::list_records(&store, &flywheel_atoms::Scope::Machine("response".into()))
        .expect("a listing")
        .len();
    assert_eq!(before, after, "the same message twice wrote a second response");
}

/// Any one of a group can be answered alone, by its number (11).
#[test]
fn one_of_a_group_is_answered_alone() {
    let (_sandbox, mut store, mut world, defs, mut chat) = a_chat("one-alone");
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");
    a_decision(&mut store, &defs, "bolt/atlas/drop-the-tail");
    let standing = commands::rail(&mut store, &defs).expect("the rail");
    let number = standing[0].number.expect("a number");

    let heard = chat
        .receive(
&mut store,
&mut world,
&defs, &Message::new("1501", "chuck", &format!("yes {number}")))
        .expect("read");
    let given = match heard {
        Heard::Answered(given) => given,
        other => panic!("a numbered reply was heard as {other:?}"),
    };
    assert_eq!(given.len(), 1, "one number answered more than one decision");
    let record = Records::get(&store, &format!("response/{}", given[0].id))
        .expect("a read")
        .expect("the response");
    assert_eq!(
        record.record.get("decision").and_then(|v| v.as_u64()),
        Some(u64::from(number))
    );
    // And the operator can tell it landed (154).
    assert!(
        chat.channel
            .replies
            .iter()
            .any(|(to, said)| to == "1501" && said.contains(&format!("#{number}"))),
        "nothing said the response was recorded: {:?}",
        chat.channel.replies
    );
}

// ---- 8.5 anything else gets one reply and writes nothing

/// A message that is neither the reply grammar nor a forward is answered with
/// one reply saying what the sink accepts, and writes nothing: the machinery
/// never parses free text (194).
#[test]
fn free_text_writes_nothing() {
    let (_sandbox, mut store, mut world, defs, mut chat) = a_chat("free-text");
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");

    let before: Vec<String> = Records::list_records(&store, &flywheel_atoms::Scope::All)
        .expect("a listing")
        .into_iter()
        .map(|o| o.id)
        .collect();

    for free in [
        "close the rows bolt when you get a chance",
        "done with the second page",
        "yes to the one about rows",
        "412 and 413 both",
    ] {
        let heard = chat
            .receive(
&mut store,
&mut world,
&defs, &Message::new("1600", "chuck", free))
            .expect("read");
        assert!(
            matches!(heard, Heard::Unaccepted),
            "`{free}` was read as {heard:?}"
        );
    }

    // Nothing was written: no response, no capture, no signal, no object at all
    // (194).
    let after: Vec<String> = Records::list_records(&store, &flywheel_atoms::Scope::All)
        .expect("a listing")
        .into_iter()
        .map(|o| o.id)
        .collect();
    assert_eq!(before, after, "free text wrote something");

    // And each got one reply, naming both shapes and the page (194, D9).
    assert_eq!(chat.channel.replies.len(), 4, "{:?}", chat.channel.replies);
    for (_, said) in &chat.channel.replies {
        assert!(said.contains("yes 412"), "the reply does not say the grammar: {said}");
        assert!(said.contains("forwarded"), "the reply does not say a forward is taken: {said}");
        assert!(said.contains(ADDRESS), "the reply carries no link to the page: {said}");
    }
}

// ---- 8.6 the posted message carries the platform's controls and the link

/// A decision raised reaches the phone through the chat sink's own
/// notification, carrying the answer controls the platform provides and a link
/// to the object on the page; the page pushes nothing of its own, and a
/// numbered reply answers the same decision the control would (309, 155, 308).
#[test]
fn posted_message_carries_controls_and_link() {
    let (_sandbox, mut store, mut world, defs, mut chat) = a_chat("controls-and-link");
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");

    let post = chat
        .deliver(&mut store, &defs)
        .expect("delivered")
        .expect("this host presents the sink");
    let line = post
        .lines
        .iter()
        .find(|l| l.object == "bolt/atlas/plan-rows")
        .expect("the decision was posted");

    // The platform's answer controls: one per answer the decision offers (309).
    let decisions = commands::rail(&mut store, &defs).expect("the rail");
    let decision = decisions
        .iter()
        .find(|d| d.object == "bolt/atlas/plan-rows")
        .expect("the decision stands");
    assert!(!decision.answers.is_empty(), "the decision offers no answer");
    assert_eq!(line.controls, decision.answers, "the controls are not the answers");
    assert!(line.answerable());

    // And the link to that object on the page, at the host's address with the
    // instance in the path (308, 205a).
    assert_eq!(
        line.link,
        format!("{ADDRESS}/bolt/atlas/plan-rows"),
        "the posted line links elsewhere"
    );
    // Both are in what the channel carried, so the grammar always works beside
    // the controls (309, 194).
    let text = post.text();
    for answer in &decision.answers {
        assert!(text.contains(answer.as_str()), "the message omits `{answer}`: {text}");
    }
    assert!(text.contains(&line.link), "the message omits the link: {text}");

    // A numbered reply to the same decision answers it, as the control would
    // (194, 309).
    let number = decision.number.expect("its number");
    let heard = chat
        .receive(
&mut store,
&mut world,
&defs, &Message::new("1700", "chuck", &format!("{number}: yes")))
        .expect("read");
    let given = match heard {
        Heard::Answered(given) => given,
        other => panic!("the numbered reply was heard as {other:?}"),
    };
    let record = Records::get(&store, &format!("response/{}", given[0].id))
        .expect("a read")
        .expect("the response");
    assert_eq!(
        record.record.get("decision").and_then(|v| v.as_u64()),
        Some(u64::from(number)),
        "the reply answered another decision"
    );
    assert_eq!(record.record.get("tool").and_then(|v| v.as_str()), Some("answer"));

    // The page sends no push of its own: the notification is the chat's (309).
    let page = flywheel_surface::page::read(&mut store, &defs, ADDRESS, "chuck").expect("the page");
    let html = flywheel_surface::page::render(&page);
    for pushes in ["serviceWorker", "Notification", "PushManager", "webpush"] {
        assert!(
            !html.contains(pushes),
            "the page pushes with `{pushes}`; the notification is the chat sink's (309)"
        );
    }
}

// ---- 8.7 the mark advances in the same write as the delivery

/// The mark is the one recorded piece of state behind the tail, and it advances
/// in the same write as the delivery, with the delivery's own id beside it (14,
/// `surfaces.yaml` effects.deliver_rail).
#[test]
fn mark_advances_with_delivery() {
    let (_sandbox, mut store, _world, defs, mut chat) = a_chat("mark-advances");
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");

    let before = chat.read(&store).expect("the sink");
    assert_eq!(before.delivered_at, None, "a sink that never delivered has a mark");
    assert_eq!(before.delivery, None);
    let seq_before = Records::get(&store, &before.id)
        .expect("a read")
        .expect("the sink")
        .seq;

    chat.deliver(&mut store, &defs).expect("delivered").expect("presented");

    let after = chat.read(&store).expect("the sink");
    let at = after.delivered_at.expect("the mark advanced");
    let delivery = after.delivery.clone().expect("the delivery's own id");
    // The id the channel gave back is the one the mark records, so the sink
    // says which message it delivered.
    assert_eq!(
        chat.channel.last().map(|p| p.surface.clone()),
        Some("#willdan".to_string())
    );
    assert!(delivery.starts_with("#willdan-"), "{delivery}");

    // One write, not two: the mark and the delivery id landed together, so no
    // reader ever sees a delivery with no mark or a mark with no delivery
    // (135, 14).
    let seq_after = Records::get(&store, &after.id)
        .expect("a read")
        .expect("the sink")
        .seq;
    assert_eq!(
        seq_after,
        seq_before + 1,
        "the delivery and the mark were two writes"
    );
    assert_eq!(at, commands::now(&store).expect("a point"));
}

/// Each sink carries its own mark, so the tail since the last look is that
/// sink's (14, 236).
#[test]
fn each_sinks_tail_is_its_own() {
    let (_sandbox, mut store, _world, defs, mut chat) = a_chat("own-tail");

    // A shared channel is a sink of its own with one mark, and no member (236).
    let mut shared = Spec::chat("chat-shared", "chuck", "#flywheel");
    shared.member = None;
    let shared = sinks::ensure(&mut store, &defs, &shared).expect("the shared sink");
    assert_eq!(shared.member, None);

    // Something reaches a tail state, and both sinks have it to carry.
    let close = |store: &mut GitStore, id: &str| {
        let when = store.now;
        commands::put_new(store, &defs, id, "intent", None, Default::default(), when)
            .expect("the intent");
        let mut intent = Records::get(store, id).expect("a read").expect("the intent");
        intent.config.insert("life".into(), "closed".into());
        intent.entered_at.insert("life".into(), when);
        let base = intent.seq;
        Records::put(store, id, &intent, base).expect("closed");
    };
    close(&mut store, "intent/rows");

    let one = chat.read(&store).expect("the sink");
    assert_eq!(sinks::tail(&store, &defs, &one).expect("a tail").len(), 1);
    assert_eq!(sinks::tail(&store, &defs, &shared).expect("a tail").len(), 1);

    // One of them looks. Its mark advances; the other's does not.
    chat.deliver(&mut store, &defs).expect("delivered").expect("presented");
    let one = chat.read(&store).expect("the sink");
    let shared = sinks::read(&store, &shared.id).expect("a read").expect("the sink");
    assert!(one.delivered_at.is_some());
    assert_eq!(shared.delivered_at, None);

    assert!(
        sinks::tail(&store, &defs, &one).expect("a tail").is_empty(),
        "the sink that looked still carries what it delivered"
    );
    let theirs = sinks::tail(&store, &defs, &shared).expect("a tail");
    assert_eq!(theirs.len(), 1, "the other sink lost a tail it never looked at");
    assert_eq!(theirs[0].object, "intent/rows");
    assert_eq!(theirs[0].kind, "closed");

    // A second thing reaches a tail state: now the tails differ by exactly it.
    store.now += chrono::Duration::minutes(1);
    close(&mut store, "intent/tail-rows");
    assert_eq!(sinks::tail(&store, &defs, &one).expect("a tail").len(), 1);
    assert_eq!(sinks::tail(&store, &defs, &shared).expect("a tail").len(), 2);
}

// ---- 8.8 notifications routed by kind

/// The operator routes a kind to the chat sink; an event of that kind occurs on
/// a host that presents no sink, and the chat carries it while the noticing
/// host shows it nowhere of its own (82).
#[test]
fn routed_kind_reaches_its_sinks() {
    let sandbox = store::Sandbox::new("routed-kind");
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let mut store = sandbox.store();

    // Two sinks, routed differently: the operator sets which kinds go where
    // (82).
    let problems = sinks::ensure(
        &mut store,
        &defs,
        &Spec::chat("chat-chuck", "chuck", "#willdan").routing(&["problem", "refusal"]),
    )
    .expect("the chat sink");
    let boltwork = sinks::ensure(
        &mut store,
        &defs,
        &Spec::chat("chat-build", "chuck", "#build").routing(&["bolt-close"]),
    )
    .expect("the second chat sink");

    assert_eq!(
        sinks::routed(&store, "problem")
            .expect("a read")
            .iter()
            .map(|s| s.id.clone())
            .collect::<Vec<_>>(),
        vec![problems.id.clone()],
        "a kind reached a sink it was not routed to"
    );

    // A host that presents no sink notices something. It writes it on the
    // object; it shows it nowhere of its own (82).
    let at = commands::now(&store).expect("a point");
    commands::put_new(&mut store, &defs, "bolt/atlas/plan-rows", "bolt", None, Default::default(), at)
        .expect("the bolt");
    sinks::notice(
        &mut store,
        "bolt/atlas/plan-rows",
        "problem",
        "the acceptance file would not parse",
        "mac-mini",
        at,
    )
    .expect("noticed");

    let mut noticing = Chat::new(&problems.id, "mac-mini", ADDRESS, Recorded::new());
    assert!(
        noticing
            .deliver(&mut store, &defs)
            .expect("a pass")
            .is_none(),
        "the host that noticed it delivered it itself"
    );
    assert!(
        noticing.channel.posts.is_empty(),
        "the noticing host showed it somewhere of its own"
    );

    // The host that presents the sink carries it.
    let mut presenting = Chat::new(&problems.id, "studio", ADDRESS, Recorded::new());
    assert!(presenting.present(&mut store).expect("the lease"));
    let post = presenting
        .deliver(&mut store, &defs)
        .expect("delivered")
        .expect("presented");
    assert_eq!(post.notices.len(), 1, "{:?}", post.notices);
    assert_eq!(post.notices[0].kind, "problem");
    assert_eq!(post.notices[0].by, "mac-mini");
    assert_eq!(post.notices[0].object, "bolt/atlas/plan-rows");
    assert!(post.text().contains("the acceptance file would not parse"));

    // The sink the kind is not routed to carries nothing of it (82).
    let mut elsewhere = Chat::new(&boltwork.id, "studio", ADDRESS, Recorded::new());
    assert!(elsewhere.present(&mut store).expect("the lease"));
    let other = elsewhere
        .deliver(&mut store, &defs)
        .expect("delivered")
        .expect("presented");
    assert!(
        other.notices.is_empty(),
        "a kind reached a sink it was not routed to: {:?}",
        other.notices
    );

    // And it is carried once: the mark moved past it (14, 82).
    let again = presenting
        .deliver(&mut store, &defs)
        .expect("delivered")
        .expect("presented");
    assert!(again.notices.is_empty(), "the notice was carried twice");
}

// ---- 8.9 a link to an away host says so

/// A link to an object held by a host past its stale window says the host is
/// away and since when, rather than failing silently (308, 150a).
#[test]
fn away_link_says_so() {
    let (_sandbox, mut store, _world, defs, chat) = a_chat("away-link");
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");

    // A laptop takes the object and then goes quiet. Its lease stands (150a).
    let seen = store.now;
    let mut mac = _sandbox.store_as("mac-mini");
    mac.heartbeat(4, true).expect("the laptop's heartbeat");
    flywheel_atoms::StateStore::lease(
        &mut store,
        &flywheel_atoms::LeaseOp::Take {
            object: "bolt/atlas/plan-rows".into(),
            holder: "mac-mini".into(),
        },
    )
    .expect("the lease");
    store.fetch().expect("the host branch");

    // While it is inside its stale window nothing is said: it is simply here.
    let post = chat.rendering(&mut store, &defs).expect("a rendering");
    let line = post
        .lines
        .iter()
        .find(|l| l.object == "bolt/atlas/plan-rows")
        .expect("the decision");
    assert_eq!(line.away, None, "a live host was called away");

    // Six minutes on, past the 5-minute stale window (150a).
    store.now = seen + chrono::Duration::minutes(6);
    let post = chat.rendering(&mut store, &defs).expect("a rendering");
    let line = post
        .lines
        .iter()
        .find(|l| l.object == "bolt/atlas/plan-rows")
        .expect("the decision");
    let said = line.away.clone().expect("the link says nothing about the host");
    assert!(said.contains("mac-mini is away since"), "{said}");
    assert!(said.contains(&seen.to_rfc3339()), "it does not say since when: {said}");
    // The link still opens the object; what is added is what it says (308).
    assert_eq!(line.link, format!("{ADDRESS}/bolt/atlas/plan-rows"));
    assert!(line.text().contains(&said), "the posted line drops it: {}", line.text());

    // The page says it too, so the link that carried it opens on something that
    // does not fail silently (308, 150a).
    let page = flywheel_surface::page::read(&mut store, &defs, ADDRESS, "chuck").expect("the page");
    let html = flywheel_surface::page::render(&page);
    assert!(
        html.contains("data-away-host=\"mac-mini\""),
        "the page says nothing about the away host"
    );
    assert!(html.contains("is away since"), "{html}");
}

// ---- 9.3 the chat forward, as an enumerator

/// A forwarded message is one source event: one capture with a pointer back to
/// it, one signal naming that capture, and nothing else (111, 112, 215).
#[test]
fn forward_writes_capture_and_signal() {
    let (_sandbox, mut store, mut world, defs, mut chat) = a_chat("forward");

    let message = Message::new("1900", "chuck", "").forwarding(
        "message/1421",
        "https://discord.com/channels/willdan/rows/1421",
    );
    let heard = chat
        .receive(&mut store, &mut world, &defs, &message)
        .expect("read");
    assert!(matches!(heard, Heard::Captured(_)), "{heard:?}");

    // One capture, under the machinery's prefix in the blueprints, keyed by the
    // source event and pointing back at it (111, 203).
    let captures = signals::captures(&world).expect("the captures");
    assert_eq!(captures.len(), 1, "{captures:?}");
    assert_eq!(captures[0].key, "message/1421");
    assert_eq!(captures[0].source, flywheel_surface::chat::FORWARD);
    assert_eq!(
        captures[0].raw, "https://discord.com/channels/willdan/rows/1421",
        "the capture does not point back at the message"
    );
    assert_eq!(captures[0].captured_by, "chuck");

    // The enumerator made no judgment: the signal is the capture machine's own
    // `ensure_signal`, which the host's tick performs (S21 runs that whole
    // path; here it is called as the tick calls it).
    assert!(
        signals::signals_of(&world, "message/1421")
            .expect("a read")
            .is_empty(),
        "the enumerator read the message into signals (115)"
    );
    let object = signals::object_of("message/1421");
    let at = commands::now(&store).expect("a point");
    let id = signals::ensure_signal(&mut store, &mut world, &defs, &object, "chuck", at)
        .expect("the effect")
        .expect("one signal");

    // One signal, naming that capture, carrying the message verbatim (113).
    let written = signals::signals_of(&world, "message/1421").expect("a read");
    assert_eq!(written.len(), 1, "{written:?}");
    assert_eq!(written[0].capture, object);
    assert_eq!(written[0].kind, "ask");
    assert_eq!(
        written[0].excerpt, "https://discord.com/channels/willdan/rows/1421",
        "the signal does not carry the message verbatim"
    );
    let held = Records::get(&store, &id).expect("a read").expect("the signal");
    assert_eq!(held.parent.as_deref(), Some(object.as_str()));

    // And nothing else: one response for the forward, no second capture, and no
    // session (111, 115).
    let responses = Records::list_records(&store, &flywheel_atoms::Scope::Machine("response".into()))
        .expect("a listing");
    assert_eq!(responses.len(), 1, "{responses:?}");
    assert!(
        !Records::list_records(&store, &flywheel_atoms::Scope::All)
            .expect("a listing")
            .iter()
            .any(|o| o.id.starts_with("fact/session/")),
        "the forward started a session"
    );

    // The same message twice is one capture and one response (111, 137).
    chat.receive(&mut store, &mut world, &defs, &message)
        .expect("read again");
    assert_eq!(signals::captures(&world).expect("a read").len(), 1);
    assert_eq!(
        Records::list_records(&store, &flywheel_atoms::Scope::Machine("response".into()))
            .expect("a listing")
            .len(),
        1
    );
}
