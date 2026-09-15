//! The chat's wire, spoken to a recorded double of Discord's HTTP API (D9,
//! D16, `surfaces/chat-sink`).
//!
//! The double is `crate::testing::discord`'s: it keeps every request the
//! channel makes and answers each route with the payload Discord documents for
//! it, and what arrives is handed in as the gateway hands it.

use super::chat::{a_decision, ADDRESS};
use crate::chat::{Channel, Chat, Heard, Line, Post, ACCEPTS};
use crate::discord::{self, Discord, Look, Settings, Token};
use crate::testing as world;
use crate::testing::discord::{pressed, typed, Double, CHANNEL, FORWARD, MESSAGE_CREATE};
use flywheel_atoms::testing::FakeStore;
use flywheel_atoms::{Records, Scope};
use flywheel_domain::commands;
use flywheel_domain::signals;
use flywheel_domain::sinks::{self, Spec};
use flywheel_engine::Definitions;
use serde_json::{json, Value};
use std::future::Future;

const SINK: &str = "sink/chat-chuck";
/// The variable the manifest's sink entry names.
const VARIABLE: &str = "FLYWHEEL_DISCORD_TOKEN";
/// What the operator placed there.
const PLACED: &str = "MTAwMDAwMDAwMDAwMDAwMDAwMA.GhYjKl.the-bot-token-the-operator-placed";

// ---------------------------------------------------------------- the helpers

fn placed(name: &str) -> Option<String> {
    (name == VARIABLE).then(|| PLACED.to_string())
}

fn a_channel(double: &Double) -> Discord {
    let token = Token::from_variable(SINK, VARIABLE, placed).expect("the token is placed");
    Discord::open(
        Settings {
            sink: SINK.into(),
            channel: CHANNEL,
            member: Some("chuck".into()),
            api: Some(double.address.clone()),
        },
        token,
    )
    .expect("the channel opens")
}

/// A store holding the chat sink on the channel, presented by this host, and
/// two decisions standing.
fn a_chat(double: &Double) -> (FakeStore, world::Files, Definitions, Chat<Discord>) {
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let mut store = FakeStore::default();
    let sink = sinks::ensure(
        &mut store,
        &defs,
        &Spec::chat("chat-chuck", "chuck", &CHANNEL.to_string()),
    )
    .expect("the chat sink");
    assert_eq!(sink.id, SINK);
    let mut chat = Chat::new(&sink.id, "studio", ADDRESS, a_channel(double));
    assert!(chat.present(&mut store).expect("the presenter lease"));
    a_decision(&mut store, &defs, "bolt/atlas/plan-rows");
    a_decision(&mut store, &defs, "bolt/atlas/drop-the-tail");
    (store, world::Files::new(), defs, chat)
}

fn block_on<F: Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime for the call")
        .block_on(future)
}

// ------------------------------------------------------------------ the tests

/// The channel speaks Discord's wire: a rendering posts as a message with each
/// decision's answers as buttons beside its number and the link to the page,
/// the id the platform gave is the one the mark records, and what arrives — a
/// numbered reply, a press of a button, a forward, anything else — reaches the
/// sink as it does from `Recorded`, each answered the way the platform answers
/// it (155, 309, 194, 112, 215, D9).
#[test]
fn discord_channel_speaks_the_wire() {
    let double = Double::start();
    let (mut store, mut world, defs, mut chat) = a_chat(&double);
    let standing = commands::rail(&mut store, &defs).expect("the rail");
    assert_eq!(standing.len(), 2, "{standing:?}");

    // ---- a delivery is one message, carrying the lines, the buttons and the
    // link.
    let post = chat
        .deliver(&mut store, &defs)
        .expect("delivered")
        .expect("this host presents the sink");
    let seen = double.seen();
    assert_eq!(seen.len(), 1, "two decisions made more than one message: {seen:#?}");
    let request = &seen[0];
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, format!("/api/v10/channels/{CHANNEL}/messages"));
    assert_eq!(
        request.authorization.as_deref(),
        Some(format!("Bot {PLACED}").as_str()),
        "the channel did not speak as the bot"
    );
    let content = request.body["content"].as_str().expect("the message's text");
    for line in &post.lines {
        assert!(content.contains(&line.text()), "the message omits `{}`: {content}", line.text());
    }
    assert!(content.ends_with(ADDRESS), "the message does not end with the page's link: {content}");

    // One row of buttons per decision, each answer beside its number, the
    // affirmative filled (309, S220).
    let rows = request.body["components"].as_array().expect("the rows of controls");
    assert_eq!(rows.len(), post.lines.len(), "{rows:#?}");
    for (row, line) in rows.iter().zip(&post.lines) {
        let number = line.number.expect("a decision line has its number");
        let buttons = row["components"].as_array().expect("a row's buttons");
        let ids: Vec<&str> = buttons.iter().filter_map(|b| b["custom_id"].as_str()).collect();
        let expected: Vec<String> = line
            .controls
            .iter()
            .filter(|answer| !answer.contains('<'))
            .take(discord::BUTTONS_LIMIT)
            .map(|answer| format!("fw:{number}:{answer}"))
            .collect();
        assert_eq!(ids, expected, "the row is not the decision's answers");
        assert_eq!(buttons[0]["style"], json!(1), "the affirmative is not filled");
        for button in buttons {
            assert!(
                button["label"].as_str().is_some_and(|l| l.starts_with(&number.to_string())),
                "a button does not carry its number: {button}"
            );
        }
    }
    // A rendering pings nobody and unfurls no link.
    assert_eq!(request.body["allowed_mentions"]["parse"], json!([]), "{}", request.body);
    assert_eq!(request.body["flags"], json!(4), "{}", request.body);

    // The id the platform gave is the delivery the mark records (14).
    let sink = chat.read(&store).expect("the sink");
    assert_eq!(sink.delivery.as_deref(), Some("1300000000000000001"));

    // ---- a numbered reply reaches the sink as it does from `Recorded`, and
    // the reply to it is a reply in the channel (194, 154).
    let first = standing[0].number.expect("a number");
    assert!(chat.channel.inbox().message(&typed("1310000000000000001", &format!("yes {first}"))));
    let heard = chat.channel.heard().expect("nothing stopped the channel");
    assert_eq!(heard.len(), 1, "{heard:?}");
    assert_eq!(heard[0].id, "1310000000000000001");
    assert_eq!(heard[0].by, "chuck", "the response is not the sink's member's (D10, 153)");
    assert_eq!(heard[0].text, format!("yes {first}"));
    assert!(heard[0].forwarded.is_none());
    match chat.receive(&mut store, &mut world, &defs, &heard[0]).expect("read") {
        Heard::Answered(given) => assert_eq!(given.len(), 1),
        other => panic!("the numbered reply was heard as {other:?}"),
    }
    let reply = double.seen().last().cloned().expect("a reply");
    assert_eq!(reply.path, format!("/api/v10/channels/{CHANNEL}/messages"));
    assert_eq!(reply.body["message_reference"]["message_id"], json!("1310000000000000001"));
    assert!(
        reply.body["content"].as_str().is_some_and(|c| c.starts_with(&format!("#{first} → yes"))),
        "the reply does not say it was recorded: {}",
        reply.body
    );

    // ---- a message a bot wrote, and one in another channel, are not the
    // sink's.
    let mut bots: Value = serde_json::from_str(MESSAGE_CREATE).unwrap();
    bots["author"]["bot"] = json!(true);
    bots["content"] = json!(format!("yes {first}"));
    assert!(!chat.channel.inbox().message(&serde_json::from_value(bots).unwrap()));
    let mut elsewhere: Value = serde_json::from_str(MESSAGE_CREATE).unwrap();
    elsewhere["channel_id"] = json!("1299999999999999999");
    assert!(!chat.channel.inbox().message(&serde_json::from_value(elsewhere).unwrap()));
    assert!(chat.channel.heard().expect("a look").is_empty());

    // ---- a press is acknowledged at once and privately, reaches the sink as
    // the numbered reply, and its answer fills the acknowledgement in (309,
    // 154).
    let second = standing[1].number.expect("a number");
    let answer = standing[1].answers[1].clone();
    let before = double.seen().len();
    assert!(block_on(
        chat.channel
            .inbox()
            .press(&pressed("1400000000000000001", &format!("fw:{second}:{answer}")))
    )
    .expect("the press is taken"));
    let acknowledged = double.seen()[before].clone();
    assert_eq!(acknowledged.method, "POST");
    assert_eq!(
        acknowledged.path,
        "/api/v10/interactions/1400000000000000001/a-press-token/callback"
    );
    assert_eq!(acknowledged.body["type"], json!(5), "the press was not deferred: {}", acknowledged.body);
    assert_eq!(acknowledged.body["data"]["flags"], json!(64), "the acknowledgement is not private");
    let heard = chat.channel.heard().expect("a look");
    assert_eq!(heard.len(), 1, "{heard:?}");
    assert_eq!(heard[0].text, format!("{second}: {answer}"));
    match chat.receive(&mut store, &mut world, &defs, &heard[0]).expect("read") {
        Heard::Answered(given) => {
            let response = Records::get(&store, &format!("response/{}", given[0].id))
                .expect("a read")
                .expect("the response");
            assert_eq!(response.record.get("decision").and_then(|v| v.as_u64()), Some(u64::from(second)));
            assert_eq!(response.record.get("answer").and_then(|v| v.as_str()), Some(answer.as_str()));
        }
        other => panic!("the press was heard as {other:?}"),
    }
    let filled = double.seen().last().cloned().expect("the acknowledgement filled in");
    assert_eq!(filled.method, "PATCH");
    assert_eq!(
        filled.path,
        "/api/v10/webhooks/1000000000000000000/a-press-token/messages/@original"
    );
    assert!(
        filled.body["content"].as_str().is_some_and(|c| c.starts_with(&format!("#{second} → {answer}"))),
        "{}",
        filled.body
    );

    // A control this channel did not post is not read as one.
    assert!(!block_on(chat.channel.inbox().press(&pressed("1400000000000000002", "someone-elses:1"))).unwrap());

    // ---- a forward is what the platform says it is: a capture pointing back
    // at the message it carries (112, 215).
    let forward: serenity::all::Message = serde_json::from_str(FORWARD).expect("a recorded forward reads");
    assert!(chat.channel.inbox().message(&forward));
    let heard = chat.channel.heard().expect("a look");
    let carried = heard[0].forwarded.clone().expect("the forward is heard as one");
    assert_eq!(carried.key, "message/1421");
    assert_eq!(carried.link, "https://discord.com/channels/900000000000000000/1250/1421");
    assert!(matches!(
        chat.receive(&mut store, &mut world, &defs, &heard[0]).expect("read"),
        Heard::Captured(_)
    ));
    assert_eq!(signals::captures(&world).expect("the captures").len(), 1);

    // ---- anything else is answered with what the channel takes, and writes
    // nothing (194).
    assert!(chat.channel.inbox().message(&typed("1310000000000000002", "close the rows bolt when you can")));
    let heard = chat.channel.heard().expect("a look");
    assert!(matches!(
        chat.receive(&mut store, &mut world, &defs, &heard[0]).expect("read"),
        Heard::Unaccepted
    ));
    let said = double.seen().last().cloned().expect("the reply");
    assert_eq!(said.body["message_reference"]["message_id"], json!("1310000000000000002"));
    assert!(said.body["content"].as_str().is_some_and(|c| c.starts_with(ACCEPTS)), "{}", said.body);
}

/// A rendering that passes Discord's limits goes on in the next message, each
/// decision's row of buttons in the message whose text carries its line; an
/// answer that takes words is left to the numbered reply its line spells out
/// (155a, 194).
#[test]
fn a_rendering_past_discords_limits_goes_on_in_the_next_message() {
    let answers: Vec<String> = ["yes", "drop", "redo: <notes>", "split", "later", "hold", "keep"]
        .iter()
        .map(|a| a.to_string())
        .collect();
    let decision = |number: u32| Line {
        number: Some(number),
        kind: "unit-proposed".into(),
        object: format!("unit/atlas/u{number}"),
        link: format!("{ADDRESS}/unit/atlas/u{number}"),
        controls: answers.clone(),
        away: None,
    };
    let tail = |at: usize| Line {
        number: None,
        kind: "landed".into(),
        object: format!("bolt/atlas/{}", "a-long-name-".repeat(12) + &at.to_string()),
        link: format!("{ADDRESS}/bolt/atlas/{at}"),
        controls: vec![],
        away: None,
    };
    let post = Post {
        sink: SINK.into(),
        surface: CHANNEL.to_string(),
        lines: (401..=407).map(decision).collect(),
        tail: (0..20).map(tail).collect(),
        notices: vec![],
        link: ADDRESS.into(),
    };

    let pieces = discord::pieces(&post);
    assert!(pieces.len() >= 3, "twenty long tail lines fit where they cannot: {}", pieces.len());
    assert_eq!(pieces[0].rows.len(), discord::ROWS_LIMIT, "the first message did not fill its rows");
    assert_eq!(pieces[1].rows.len(), 2, "the last two decisions are not in the next message");
    for piece in &pieces {
        assert!(piece.text.chars().count() <= discord::TEXT_LIMIT, "a message is past the limit");
        assert!(piece.rows.len() <= discord::ROWS_LIMIT);
        // Each row sits in the message carrying its decision's line.
        for row in &piece.rows {
            let (number, _) = discord::control_of(&row[0].id).expect("one of the channel's own");
            assert!(piece.text.contains(&format!("#{number} ")), "row {number} is away from its line");
        }
    }
    // Every line is carried once, and the link comes last.
    let all: String = pieces.iter().map(|p| p.text.as_str()).collect();
    for line in post.lines.iter().chain(&post.tail) {
        assert_eq!(all.matches(&line.text()).count(), 1, "`{}` is not carried once", line.text());
    }
    assert!(pieces.last().unwrap().text.trim_end().ends_with(ADDRESS));

    // A row holds what fits: five answers that need no words, the affirmative
    // filled and drop in red, and the patterned answer left to the grammar,
    // which the line still spells out.
    let row = &pieces[0].rows[0];
    let ids: Vec<&str> = row.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["fw:401:yes", "fw:401:drop", "fw:401:split", "fw:401:later", "fw:401:hold"]);
    assert_eq!(row[0].look, Look::Affirmative);
    assert_eq!(row[1].look, Look::Drop);
    assert_eq!(row[2].look, Look::Plain);
    assert_eq!(row[0].label, "401 yes");
    assert!(pieces[0].text.contains("redo: <notes>"));
    assert_eq!(discord::control_of("fw:401:new bolt"), Some((401, "new bolt".into())));
    assert_eq!(discord::control_of("fw:401:"), None);
    assert_eq!(discord::control_of("401:yes"), None);
}

/// The bot's token is read from the variable the manifest's sink entry names,
/// and nothing the channel writes, sends as a body, says in an error or prints
/// carries it (204, 207). With the variable unset the channel is not opened,
/// and the refusal names the sink and the variable and says what to do (217f).
#[test]
fn token_written_nowhere() {
    // Unset, or set to nothing: refused, naming where to place it.
    for read in [|_: &str| None, |_: &str| Some("  ".to_string())] {
        let refused = Token::from_variable(SINK, VARIABLE, read).expect_err("no token, no channel");
        let said = format!("{refused:#}");
        assert!(said.contains(SINK) && said.contains(VARIABLE), "{said}");
        assert!(said.contains("place the token there"), "the refusal does not say what to do: {said}");
    }

    // Placed: the channel speaks with it, and every request carried it — so a
    // sweep that finds it nowhere else is a sweep of something.
    let double = Double::start();
    let (mut store, mut world, defs, mut chat) = a_chat(&double);
    let standing = commands::rail(&mut store, &defs).expect("the rail");
    let post = chat.deliver(&mut store, &defs).expect("delivered").expect("presented");
    let first = standing[0].number.expect("a number");
    chat.channel.inbox().message(&typed("1310000000000000001", &format!("yes {first}")));
    assert!(block_on(chat.channel.inbox().press(&pressed(
        "1400000000000000001",
        &format!("fw:{}:yes", standing[1].number.expect("a number"))
    )))
    .expect("the press"));
    chat.channel.inbox().message(&serde_json::from_str(FORWARD).unwrap());
    chat.channel.inbox().message(&typed("1310000000000000002", "what is waiting?"));
    for message in chat.channel.heard().expect("a look") {
        chat.receive(&mut store, &mut world, &defs, &message).expect("read");
    }

    // A refused request, which is where a token most often leaks: into the
    // error a host puts in its run record.
    double.refuse();
    let refused = chat.deliver(&mut store, &defs).expect_err("a refused token delivers nothing");
    let refusal = format!("{refused:#}");
    assert!(refusal.contains("401"), "the refusal does not say what Discord said: {refusal}");
    assert!(refusal.contains(VARIABLE), "the refusal does not say where to place a token: {refusal}");

    // Nothing makes a refused token good, so the channel asks nothing more of
    // Discord until the operator places another.
    let asked = double.seen().len();
    let again = chat.deliver(&mut store, &defs).expect_err("still refused");
    assert_eq!(double.seen().len(), asked, "a refused token was sent again");
    assert!(format!("{again:#}").contains(VARIABLE), "{again:#}");

    let seen = double.seen();
    assert!(seen.len() >= 6, "{seen:#?}");
    for request in &seen {
        assert_eq!(request.authorization.as_deref(), Some(format!("Bot {PLACED}").as_str()));
    }

    // Nowhere else: not the state the sink wrote through, not the captures,
    // not a body sent or a path asked, not the error, not the rendering, not
    // anything the channel prints.
    let mut swept = vec![
        refusal,
        post.text(),
        format!("{:?}", chat.channel),
        format!("{:?}", signals::captures(&world).expect("the captures")),
    ];
    for object in store.list_records(&Scope::All).expect("a listing") {
        swept.push(format!("{:?}", store.thread(&object.id).expect("its thread")));
        swept.push(format!("{object:?}"));
    }
    for request in &seen {
        swept.push(format!("{} {} {}", request.method, request.path, request.body));
    }
    assert!(swept.len() > 10);
    for text in &swept {
        assert!(!text.contains(PLACED), "the token is written: {text}");
        assert!(!text.contains("the-bot-token"), "part of the token is written: {text}");
    }
}

/// The rendering delivered, the host gone, and what the operator wrote in the
/// channel while nobody listened: a numbered reply, and a line the channel does
/// not take. A reply from before the delivery stands behind the mark. Returns
/// the delivery the mark records and the number replied to.
fn replies_waiting(double: &Double) -> (FakeStore, world::Files, Definitions, String, u32) {
    let (mut store, world, defs, mut chat) = a_chat(double);
    let standing = commands::rail(&mut store, &defs).expect("the rail");
    chat.deliver(&mut store, &defs).expect("delivered").expect("presented");
    let delivered = chat
        .read(&store)
        .expect("the sink")
        .delivery
        .expect("the mark records the delivery");
    let first = standing[0].number.expect("a number");
    drop(chat);
    double.written(&typed("1290000000000000001", &format!("drop {first}")));
    double.written(&typed("1310000000000000001", &format!("yes {first}")));
    double.written(&typed("1310000000000000002", "are you still there?"));
    (store, world, defs, delivered, first)
}

/// The presenter back: a channel opened again on the same chat, reading from
/// after the delivery the sink's mark records, as `flywheel host` starts it.
fn the_presenter_returns(double: &Double, store: &mut FakeStore, delivered: &str) -> Chat<Discord> {
    let mut chat = Chat::new(SINK, "studio", ADDRESS, a_channel(double));
    assert!(chat.present(store).expect("the presenter lease"));
    chat.channel.inbox().after(Some(delivered));
    chat
}

/// The responses recorded to one decision.
fn answers_to(store: &FakeStore, number: u32) -> Vec<String> {
    store
        .list_records(&Scope::Machine("response".into()))
        .expect("a listing")
        .into_iter()
        .filter(|r| r.record.get("decision").and_then(|v| v.as_u64()) == Some(u64::from(number)))
        .filter_map(|r| r.record.get("answer").and_then(|v| v.as_str()).map(String::from))
        .collect()
}

/// The gateway replays nothing, so what the operator wrote while no host
/// presented the chat is read from the channel's history when the presenter
/// starts listening: every message after the sink's recorded delivery, oldest
/// first, each applied once and acknowledged when it is recorded (217f, 137,
/// 154).
#[test]
fn a_reply_sent_while_nobody_presented_is_applied_when_the_presenter_returns() {
    let double = Double::start();
    let (mut store, mut world, defs, delivered, first) = replies_waiting(&double);
    let mut chat = the_presenter_returns(&double, &mut store, &delivered);

    // What the gateway's ready does: read the channel after the delivery.
    let asked = double.seen().len();
    let handed = block_on(chat.channel.inbox().waited()).expect("the history reads");
    assert_eq!(handed, 2);
    let read = double.seen()[asked..].to_vec();
    assert!(
        read.iter().any(|r| r.method == "GET"
            && r.path == format!("/api/v10/channels/{CHANNEL}/messages")
            && r.query.as_deref().is_some_and(|q| q.contains(&format!("after={delivered}")))),
        "the history was not read after the delivery: {read:#?}"
    );
    assert!(read.iter().all(|r| r.method == "GET"), "reading what waited sent something: {read:#?}");

    // Oldest first; nothing from before the delivery, nothing the bot wrote.
    let heard = chat.channel.heard().expect("nothing stopped the channel");
    let ids: Vec<&str> = heard.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["1310000000000000001", "1310000000000000002"]);
    assert_eq!(heard[0].by, "chuck", "the response is not the sink's member's (153)");
    for message in &heard {
        chat.receive(&mut store, &mut world, &defs, message).expect("read");
    }

    // The reply applied once, and the one from before the delivery not at all.
    assert_eq!(answers_to(&store, first), ["yes"]);

    // Each answered once, on itself: the reply with its record, the line the
    // channel does not take with what it takes.
    let replies: Vec<_> = double
        .seen()
        .into_iter()
        .filter(|r| r.method == "POST" && r.body["message_reference"]["message_id"].is_string())
        .collect();
    assert_eq!(replies.len(), 2, "{replies:#?}");
    assert_eq!(replies[0].body["message_reference"]["message_id"], json!("1310000000000000001"));
    assert!(
        replies[0].body["content"]
            .as_str()
            .is_some_and(|c| c.starts_with(&format!("#{first} → yes, recorded as "))),
        "{}",
        replies[0].body
    );
    assert_eq!(replies[1].body["message_reference"]["message_id"], json!("1310000000000000002"));
    assert!(replies[1].body["content"].as_str().is_some_and(|c| c.starts_with(ACCEPTS)));

    // Read again in the same run, nothing is handed over twice.
    assert_eq!(block_on(chat.channel.inbox().waited()).expect("the history reads"), 0);
    assert!(chat.channel.heard().expect("a look").is_empty());
}

/// A host that restarts before it delivers again reads the same messages
/// after the same delivery. What it answered before is not handed over again,
/// and a message that reaches the sink again all the same is the same
/// delivery: it writes nothing and is not acknowledged a second time (217f,
/// 137, 154).
#[test]
fn a_reply_read_again_after_a_restart_is_not_acknowledged_twice() {
    let double = Double::start();
    let (mut store, mut world, defs, delivered, first) = replies_waiting(&double);
    let mut chat = the_presenter_returns(&double, &mut store, &delivered);
    block_on(chat.channel.inbox().waited()).expect("the history reads");
    for message in chat.channel.heard().expect("a look") {
        chat.receive(&mut store, &mut world, &defs, &message).expect("read");
    }
    assert_eq!(answers_to(&store, first), ["yes"]);
    let records = store.list_records(&Scope::All).expect("a listing").len();
    drop(chat);

    // The restart: the mark still records the same delivery.
    let mut chat = the_presenter_returns(&double, &mut store, &delivered);
    let asked = double.seen().len();
    let handed = block_on(chat.channel.inbox().waited()).expect("the history reads");
    assert_eq!(handed, 0, "a message already answered was handed over again");
    assert!(chat.channel.heard().expect("a look").is_empty());

    // The reply reaching the sink again all the same, as a gateway replaying
    // it would hand it over: the same delivery, recorded before.
    let reply = typed("1310000000000000001", &format!("yes {first}"));
    assert!(chat.channel.inbox().message(&reply));
    assert!(!chat.channel.inbox().message(&reply), "one message was handed over twice");
    let heard = chat.channel.heard().expect("a look");
    assert_eq!(heard.len(), 1, "{heard:?}");
    match chat.receive(&mut store, &mut world, &defs, &heard[0]).expect("read") {
        Heard::Answered(given) => assert!(
            matches!(given[0].outcome, flywheel_atoms::Received::AlreadyApplied { .. }),
            "{given:?}"
        ),
        other => panic!("the reply was heard as {other:?}"),
    }

    // Nothing written, and no second request to the channel: the restart only
    // read.
    assert_eq!(answers_to(&store, first), ["yes"]);
    assert_eq!(store.list_records(&Scope::All).expect("a listing").len(), records);
    let since = double.seen()[asked..].to_vec();
    assert!(
        since.iter().all(|r| r.method == "GET"),
        "the restart acknowledged something again: {since:#?}"
    );
}
