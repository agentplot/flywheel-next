//! The chat sink: the chat the operator already reads, carrying the same
//! decisions and the same numbers as the page (18, `surfaces/chat-sink`).
//!
//! The chat is Discord (D9, section 9). What is Discord about it is the shape
//! of the message — one line per decision with its number, the platform's own
//! answer controls beside them, a link to the object on the page — and the two
//! shapes of message it accepts back: the numbered reply grammar, which is the
//! `answer` tool, and a forwarded message, which is the `capture` tool (194,
//! 112, 215). Anything else is answered with one reply naming those two and
//! writes nothing; the machinery never parses free text (194).
//!
//! Between that and the wire is `Channel`, one seam with one implementation in
//! this release — the same shape `flywheel-workspace-recorded` gives the
//! effects of 42 (D8). The sink's own behaviour is what phase 1 proves: the
//! rendering, the grammar, the mark, the routing and the presenter lease are
//! all exercised against a real state store, and the client that speaks to
//! Discord's servers is a second `Channel` beside the first, not a second sink.

use crate::catalogue::{self, Call};
use crate::links;
use anyhow::{bail, Result};
use flywheel_atoms::{StateStore, World};
use flywheel_domain::commands::{self, Called};
use flywheel_domain::sinks::{self, Sink};
use flywheel_engine::{DecisionInstance, Definitions};
use chrono::Duration;
use serde_json::json;
use std::collections::BTreeMap;

/// One line of a posted rendering (18, 209).
///
/// Each kind keeps the form the status view gives it, in one line: a decision
/// line is answerable and no other line is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// The number the register gave, on a decision line; none on any other
    /// (15).
    pub number: Option<u32>,
    pub kind: String,
    pub object: String,
    /// The link to the object on the page, at the host's address with the
    /// instance in the path (308, 205a).
    pub link: String,
    /// The platform's own answer controls: one per answer the decision offers
    /// (309). Empty on a line that is not a decision.
    pub controls: Vec<String>,
    /// What a link to an away host says instead of failing silently (308,
    /// 150a).
    pub away: Option<String>,
}

impl Line {
    /// Whether this line offers an answer. Only a decision line does (18, 209).
    pub fn answerable(&self) -> bool {
        self.number.is_some() && !self.controls.is_empty()
    }

    /// The line as the channel carries it: `#n kind · object · answers · link`
    /// (`surfaces.yaml` effects.deliver_rail, 18, 308).
    ///
    /// The answers are in the text as well as in `controls`, so the numbered
    /// reply grammar always works beside whatever controls the platform
    /// provides (309, 194).
    pub fn text(&self) -> String {
        let link = match &self.away {
            Some(said) => format!("{} — {said}", self.link),
            None => self.link.clone(),
        };
        match self.number {
            Some(number) => format!(
                "#{number} {} · {} · {} · {link}",
                self.kind,
                self.object,
                self.controls.join(" | "),
            ),
            None => format!("{} · {} · {link}", self.kind, self.object),
        }
    }
}

/// One message posted to a sink: the numbered decisions routed here, the tail
/// since this sink's mark, and a link to the page (18, 14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Post {
    pub sink: String,
    pub surface: String,
    pub lines: Vec<Line>,
    /// The tail since this sink's own mark, outside the decisions (14, 236).
    pub tail: Vec<Line>,
    /// What the machinery noticed and the operator routed here, whichever host
    /// noticed it (82).
    pub notices: Vec<sinks::Notice>,
    /// The link to the page itself, which every rendering carries (18, 308).
    pub link: String,
}

impl Post {
    /// The whole message, as the channel carries it.
    pub fn text(&self) -> String {
        let mut out = String::new();
        for line in &self.lines {
            out.push_str(&line.text());
            out.push('\n');
        }
        for line in &self.tail {
            out.push_str(&line.text());
            out.push('\n');
        }
        for notice in &self.notices {
            out.push_str(&format!("{} · {} · {}\n", notice.kind, notice.object, notice.text));
        }
        out.push_str(&self.link);
        out
    }
}

/// The wire between the sink and the platform (D8, D9).
///
/// `post` carries a rendering to the channel and hands back the delivery's own
/// id, which the mark records beside it; `reply` answers one message the sink
/// received. Nothing about the sink's behaviour lives behind this trait: what
/// it posts and what it makes of what arrives are decided above it.
pub trait Channel {
    /// Post one rendering, returning the delivery's own id on the platform.
    fn post(&mut self, post: &Post) -> Result<String>;

    /// Answer one message that arrived, writing nothing to the store (194).
    fn reply(&mut self, to: &str, text: &str) -> Result<()>;
}

/// A host loads its sinks' channels by name, like its workspace and its
/// sessions (D8), so what it holds is a boxed one and this is what lets it be
/// used as any other.
impl Channel for Box<dyn Channel> {
    fn post(&mut self, post: &Post) -> Result<String> {
        (**self).post(post)
    }

    fn reply(&mut self, to: &str, text: &str) -> Result<()> {
        (**self).reply(to, text)
    }
}

/// The `Channel` this release carries: every post recorded, exactly as
/// `flywheel-workspace-recorded` records the effects of 42 (D8).
///
/// A test and the scenario runner read what was posted; a host on the willdan
/// week reads it in the run record. The client that speaks to Discord's servers
/// is a second implementation of this trait and changes nothing above it.
#[derive(Debug, Default)]
pub struct Recorded {
    pub posts: Vec<Post>,
    /// What the sink said back, and to which message (194).
    pub replies: Vec<(String, String)>,
    delivered: u64,
}

impl Recorded {
    pub fn new() -> Recorded {
        Recorded::default()
    }

    /// The last thing posted, for a caller that wants to read it back.
    pub fn last(&self) -> Option<&Post> {
        self.posts.last()
    }
}

impl Channel for Recorded {
    fn post(&mut self, post: &Post) -> Result<String> {
        self.delivered += 1;
        self.posts.push(post.clone());
        Ok(format!("{}-{}", post.surface, self.delivered))
    }

    fn reply(&mut self, to: &str, text: &str) -> Result<()> {
        self.replies.push((to.to_string(), text.to_string()));
        Ok(())
    }
}

/// The chat sink, bound to one channel and presented by one host.
///
/// A host holds the sink's lease before it delivers anything, and one that does
/// not hold it delivers nothing, so the operator sees each delivery once (148).
pub struct Chat<C: Channel> {
    /// The sink object's id.
    pub sink: String,
    /// The host this sink is presented from.
    pub host: String,
    /// The host's address, which every link is written at (205a, 308, D10a).
    pub address: String,
    pub channel: C,
}

impl<C: Channel> Chat<C> {
    pub fn new(sink: &str, host: &str, address: &str, channel: C) -> Chat<C> {
        Chat {
            sink: sink.to_string(),
            host: host.to_string(),
            address: address.to_string(),
            channel,
        }
    }

    /// Become this sink's presenter, or learn that another host is (148).
    pub fn present<S: StateStore>(&mut self, store: &mut S) -> Result<bool> {
        sinks::take_presenter(store, &self.sink, &self.host)?;
        sinks::presents(store, &self.sink, &self.host)
    }

    /// The sink as it stands, or a refusal naming the sink that is not there.
    pub fn read<S: StateStore>(&self, store: &S) -> Result<Sink> {
        sinks::read(store, &self.sink)?
            .ok_or_else(|| anyhow::anyhow!("`{}` is no sink of this instance", self.sink))
    }

    /// What this sink would carry now, without delivering it. The page and the
    /// chat make their renderings the same way, from the register and the
    /// objects, so this is also how a caller compares them (15, 18).
    pub fn rendering<S: StateStore>(&self, store: &mut S, defs: &Definitions) -> Result<Post> {
        let sink = self.read(store)?;
        let decisions = commands::rail(store, defs)?;
        let tail = sinks::tail(store, defs, &sink)?;
        let notices = sinks::notices(store, &sink)?;
        // A link to a host past its stale window says so rather than failing
        // silently (308, 150a).
        let away = sinks::away_by_object(store, commands::now(store)?, Duration::minutes(5))?;
        render(&sink, &self.address, &decisions, &tail, &notices, &away)
    }

    /// Deliver: post the rendering and advance this sink's mark in the same
    /// write, with the delivery's own id beside it (14,
    /// `surfaces.yaml` effects.deliver_rail).
    ///
    /// A host that is not this sink's presenter delivers nothing and says so by
    /// returning none, so the operator sees each delivery once (148).
    pub fn deliver<S: StateStore>(
        &mut self,
        store: &mut S,
        defs: &Definitions,
    ) -> Result<Option<Post>> {
        if !sinks::presents(store, &self.sink, &self.host)? {
            return Ok(None);
        }
        let post = self.rendering(store, defs)?;
        let at = commands::now(store)?;
        let delivery = self.channel.post(&post)?;
        sinks::mark(store, &self.sink, at, &delivery)?;
        Ok(Some(post))
    }
}

/// One decision as the chat carries it: its number, a link to the object on the
/// page, and the platform's answer controls (15, 18, 309).
pub fn line(
    address: &str,
    decision: &DecisionInstance,
    away: Option<&sinks::Away>,
) -> Result<Line> {
    Ok(Line {
        number: decision.number,
        kind: decision.kind.clone(),
        object: decision.object.clone(),
        link: links::to_object(address, &decision.object)?,
        controls: decision.answers.clone(),
        away: away.map(sinks::Away::said),
    })
}

/// One thing that reached a tail state, as the chat carries it: the same one
/// line, and no answer on it, because it is not a decision (18, 209).
pub fn tail_line(address: &str, item: &sinks::TailItem, away: Option<&sinks::Away>) -> Result<Line> {
    Ok(Line {
        number: None,
        kind: item.kind.clone(),
        object: item.object.clone(),
        link: links::to_object(address, &item.object)?,
        controls: vec![],
        away: away.map(sinks::Away::said),
    })
}

/// The rendering one delivery carries: the numbered decisions routed to this
/// sink, the tail since its mark, and a link to the page (18, 14, 82).
///
/// Nothing about it is stored (15): it is made from the register and the
/// objects each time, like the page's.
pub fn render(
    sink: &Sink,
    address: &str,
    decisions: &[DecisionInstance],
    tail: &[sinks::TailItem],
    notices: &[sinks::Notice],
    away: &BTreeMap<String, sinks::Away>,
) -> Result<Post> {
    check_address(address)?;
    let mut lines = Vec::new();
    for decision in decisions {
        // Routing by kind is the operator's, per sink (82); a sink routing
        // `all` carries every kind, which is what one sink alone carries.
        if !sink.routes_kind(&decision.kind) {
            continue;
        }
        lines.push(line(address, decision, away.get(&decision.object))?);
    }
    let mut carried = Vec::new();
    for item in tail {
        carried.push(tail_line(address, item, away.get(&item.object))?);
    }
    Ok(Post {
        sink: sink.id.clone(),
        surface: sink.surface.clone(),
        lines,
        tail: carried,
        notices: notices.to_vec(),
        link: links::to_page(address)?,
    })
}

// ---------------------------------------------------- what the sink accepts

/// The two shapes of message the sink accepts, and everything else (194, D9).
///
/// The machinery never parses free text: this is a grammar, not an
/// interpreter. It reads a number and a word and nothing more, and a message it
/// does not recognise is not guessed at — the interpreter that would read one is
/// the host's agent, which is phase 4 (`surfaces.yaml` host_agent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grammar {
    /// `yes 412`, `412 yes`, `412: <text>`, or `412` on its own (194).
    Answer { number: u32, answer: String },
    /// `yes all`: one response per standing decision, so "yes to all" means
    /// something and any one could still have been answered alone (11).
    All { answer: String },
    /// Nothing the grammar names.
    Unaccepted,
}

/// What the sink accepts, said in one line, for the reply a message it does not
/// recognise gets back (194).
pub const ACCEPTS: &str =
    "this channel takes a numbered reply — `yes 412`, `412: <text>`, `yes all` — or a forwarded \
     message, which becomes a capture. Anything else is not read.";

/// Read one message against the grammar (194).
pub fn read_grammar(text: &str) -> Grammar {
    let text = text.trim();
    if text.is_empty() {
        return Grammar::Unaccepted;
    }

    // `412: <text>` — the number, then the answer whole. The text after the
    // colon is the answer and is never read for a command (194).
    if let Some((head, body)) = text.split_once(':') {
        if let Ok(number) = head.trim().parse::<u32>() {
            return Grammar::Answer {
                number,
                answer: body.trim().to_string(),
            };
        }
    }

    let words: Vec<&str> = text.split_whitespace().collect();
    match words.as_slice() {
        // `412` on its own is the bare number of the reply grammar, which is a
        // yes (`surfaces.yaml` palette.grammar).
        [one] => match one.parse::<u32>() {
            Ok(number) => Grammar::Answer {
                number,
                answer: "yes".into(),
            },
            Err(_) => Grammar::Unaccepted,
        },
        // `yes all`, and only that word for all: a message saying anything else
        // about all of them is free text (194).
        [answer, "all"] if is_answer(answer) => Grammar::All {
            answer: answer.to_ascii_lowercase(),
        },
        [answer, number] if is_answer(answer) => match number.parse::<u32>() {
            Ok(number) => Grammar::Answer {
                number,
                answer: answer.to_ascii_lowercase(),
            },
            Err(_) => Grammar::Unaccepted,
        },
        [number, answer] if is_answer(answer) => match number.parse::<u32>() {
            Ok(number) => Grammar::Answer {
                number,
                answer: answer.to_ascii_lowercase(),
            },
            Err(_) => Grammar::Unaccepted,
        },
        _ => Grammar::Unaccepted,
    }
}

/// The words the grammar reads as an answer. It is a closed list and not a
/// judgment: a word outside it is free text and is not guessed at (194).
fn is_answer(word: &str) -> bool {
    matches!(
        word.to_ascii_lowercase().as_str(),
        "yes" | "no" | "later" | "drop" | "hold" | "approve" | "keep" | "finish" | "takeover"
    )
}

/// A message the operator forwarded into the channel: one source event, with
/// its own key and a pointer back to it (111, 112, 215).
///
/// The platform gives all three — Discord carries a forwarded message as a
/// snapshot of the one it points at, not as text — so nothing here is read out
/// of what the operator typed (194).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Forwarded {
    /// The source event's own key. Forwarding the same message twice yields one
    /// capture (111).
    pub key: String,
    /// The pointer back to the message, which stays outside every repository
    /// (111, 112, 215).
    pub link: String,
}

/// The source a forwarded message is captured under, which is what the capture
/// machine reads to give it its one signal without a judgment
/// (`capture.yaml` reading.captured, S21).
pub const FORWARD: &str = "forwarded-message";

/// One message that arrived at the sink.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// The platform's own id for it, which is the delivery's identity: the same
    /// message twice is one response (137).
    pub id: String,
    /// The member who wrote it, which every response records as given by (153).
    pub by: String,
    pub text: String,
    /// What it forwards, where it forwards anything (112, 215).
    pub forwarded: Option<Forwarded>,
}

impl Message {
    pub fn new(id: &str, by: &str, text: &str) -> Message {
        Message {
            id: id.to_string(),
            by: by.to_string(),
            text: text.to_string(),
            forwarded: None,
        }
    }

    /// A forwarded message, with the source event it points at.
    pub fn forwarding(mut self, key: &str, link: &str) -> Message {
        self.forwarded = Some(Forwarded {
            key: key.to_string(),
            link: link.to_string(),
        });
        self
    }
}

/// What the sink made of a message.
#[derive(Debug, Clone)]
pub enum Heard {
    /// One response per decision answered; "yes all" makes as many as there are
    /// standing decisions, each on its own delivery (11, 137).
    Answered(Vec<Called>),
    /// One capture, from a forwarded message (112, 215).
    Captured(Called),
    /// Neither shape: the sink said what it accepts and wrote nothing (194).
    Unaccepted,
}

/// The `capture` call a forwarded message makes: one keyed capture per source
/// event, its pointer back to the message, and no judgment (112, 215, 111).
pub fn forward_call(by: &str, forwarded: &Forwarded) -> Call {
    Call::new("capture", by, "chat")
        .keyed(&forwarded.key)
        .arg("text", json!(forwarded.link))
        .arg("source", json!(FORWARD))
}

impl<C: Channel> Chat<C> {
    /// Read one message that arrived and do what it says, or say what the sink
    /// accepts and write nothing (194, D9).
    ///
    /// A forward is looked at before the grammar, because a forwarded message
    /// is what the platform says it is and never what its text looks like.
    pub fn receive<S: StateStore, W: World + ?Sized>(
        &mut self,
        store: &mut S,
        world: &mut W,
        defs: &Definitions,
        message: &Message,
    ) -> Result<Heard> {
        if let Some(forwarded) = &message.forwarded {
            let mut call = forward_call(&message.by, forwarded);
            call.delivery_id = Some(format!("chat-{}", message.id));
            let called = crate::catalogue::call(store, world, defs, &call)?;
            return Ok(Heard::Captured(called));
        }
        match read_grammar(&message.text) {
            Grammar::Answer { number, answer } => Ok(Heard::Answered(vec![self.answer(
                store,
                world,
                defs,
                message,
                number,
                &answer,
                &format!("chat-{}", message.id),
            )?])),
            // "Yes to all" means something and any one of them could have been
            // answered alone, so it is one response per decision and not one
            // response about many (11). Each carries its own delivery identity,
            // so the same message twice is still one response each (137).
            Grammar::All { answer } => {
                let standing = commands::rail(store, defs)?;
                let sink = self.read(store)?;
                let mut given = Vec::new();
                for decision in standing {
                    let Some(number) = decision.number else {
                        continue;
                    };
                    if !sink.routes_kind(&decision.kind) {
                        continue;
                    }
                    given.push(self.answer(
                        store,
                        world,
                        defs,
                        message,
                        number,
                        &answer,
                        &format!("chat-{}-{number}", message.id),
                    )?);
                }
                Ok(Heard::Answered(given))
            }
            // Neither shape. One reply saying what the sink accepts, with a
            // link to the page, and nothing written: the machinery does not
            // parse it, guess at it, or record it as a capture (194, D9).
            Grammar::Unaccepted => {
                let link = links::to_page(&self.address)?;
                self.channel
                    .reply(&message.id, &format!("{ACCEPTS} {link}"))?;
                Ok(Heard::Unaccepted)
            }
        }
    }

    /// One numbered answer, through the same `answer` tool the page's control
    /// calls (193, 194).
    fn answer<S: StateStore, W: World + ?Sized>(
        &mut self,
        store: &mut S,
        world: &mut W,
        defs: &Definitions,
        message: &Message,
        number: u32,
        answer: &str,
        delivery: &str,
    ) -> Result<Called> {
        let call = Call::new(catalogue::ANSWER, &message.by, "chat")
            .delivered(delivery)
            .arg("decision", json!(number))
            .arg("answer", json!(answer));
        let called = crate::catalogue::call(store, world, defs, &call)?;
        // The operator can tell it was recorded (154).
        self.channel.reply(
            &message.id,
            &format!("#{number} → {answer}, recorded as {}", called.id),
        )?;
        Ok(called)
    }
}

/// Refuse to write a link that names a localhost port, which opens nothing on a
/// phone (205a, 308, D10a).
pub fn check_address(address: &str) -> Result<()> {
    if links::is_localhost(address) {
        bail!(
            "the chat would carry a link at `{address}`, which opens nothing on a phone; a \
             rendering links at the host's private-network address with the instance in the \
             path (205a, 308, D10a)"
        );
    }
    Ok(())
}
