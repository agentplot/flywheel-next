//! The chat's wire: Discord, through `serenity` (D9, D16, `surfaces/chat-sink`).
//!
//! `Recorded` is what the sink's behaviour is proven against, and this is the
//! second `Channel` beside it: nothing above the trait changes for it (D8).
//! What is Discord about the chat is here and nowhere else — a rendering carried
//! as the messages the platform accepts, each decision's answers as buttons
//! beside its number (155, 309), a message in the channel or a press of one of
//! those buttons turned into the `Message` the sink reads (194, 112, 215), and
//! the bot's token read from where the operator placed it (204, 207).
//!
//! The host's tick is not async and serenity is, so a channel keeps a thread of
//! its own with a runtime on it: every call to the platform is sent there and
//! waited on, and the gateway's listener runs there beside them. Dropping the
//! channel stops the thread.

use crate::chat::{self, Channel, Line, Post};
use anyhow::{anyhow, bail, Context as _, Result};
use serenity::all::{
    ButtonStyle, ChannelId, ComponentInteraction, Context, CreateActionRow,
    CreateAllowedMentions, CreateButton, CreateMessage, EditInteractionResponse, EventHandler,
    GatewayIntents, GetMessages, Http, HttpBuilder, Interaction, Message as Posted, MessageFlags,
    MessageId, MessageReferenceKind, Ready, User, UserId,
};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Discord's own limits on one message: the characters of its text, its rows
/// of controls, and the controls in one row.
pub const TEXT_LIMIT: usize = 2000;
pub const ROWS_LIMIT: usize = 5;
pub const BUTTONS_LIMIT: usize = 5;

/// A button's label and its id each have a limit of their own.
const LABEL_LIMIT: usize = 80;
const CONTROL_ID_LIMIT: usize = 100;

/// Every control this channel posts has an id beginning here, so a press is
/// known for one of the sink's own and never read out of anything else.
const CONTROL: &str = "fw:";

/// How long a call to the platform is waited on before the tick carries on
/// without it (81, 127).
const PATIENCE: Duration = Duration::from_secs(30);

/// The link Discord gives a message, which a forward's capture points back at
/// (111, 112).
const MESSAGE_LINK: &str = "https://discord.com/channels";

/// The most messages Discord gives in one read of a channel's history.
const HISTORY_PAGE: u8 = 100;

// ------------------------------------------------------------------ the token

/// The bot's token, as read from where the operator placed it (204, 207).
///
/// It has no `Display`, no `Serialize`, and a `Debug` that names only where it
/// was read from, so a channel holding one cannot put it in a log, a record or
/// a rendering by being printed.
pub struct Token {
    placed: String,
    /// The variable it was read from, which is what a refusal names.
    variable: String,
}

impl std::fmt::Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Token(from {})", self.variable)
    }
}

impl Token {
    /// Read the token from the variable the manifest's sink entry names, with
    /// the lookup the caller gives: the process environment on a host.
    ///
    /// An unset or empty variable is refused with what to do about it, naming
    /// the sink and the variable and never a value, so the host can report it
    /// under attention and run on (204, 207, 217f).
    pub fn from_variable(
        sink: &str,
        variable: &str,
        read: impl Fn(&str) -> Option<String>,
    ) -> Result<Token> {
        match read(variable)
            .map(|placed| placed.trim().to_string())
            .filter(|placed| !placed.is_empty())
        {
            Some(placed) => Ok(Token {
                placed,
                variable: variable.to_string(),
            }),
            None => bail!(
                "`{sink}` reads its bot token from `{variable}`, which is not set: place the \
                 token there and restart the host. Until then this host delivers nothing to \
                 the chat and runs on (204, 207, 217f)"
            ),
        }
    }

    /// Read it from the process environment, which is where the operator
    /// places it.
    pub fn from_environment(sink: &str, variable: &str) -> Result<Token> {
        Token::from_variable(sink, variable, |name| std::env::var(name).ok())
    }
}

// ------------------------------------------------------------ the rendering

/// What a button looks like: the affirmative filled, drop in red, the rest
/// plain, as the page draws them (S220).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    Affirmative,
    Drop,
    Plain,
}

/// One button: the answer it gives to one numbered decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    /// `fw:<number>:<answer>`, which a press hands back.
    pub id: String,
    /// The number and the answer, so a row says which decision it answers.
    pub label: String,
    pub look: Look,
}

/// One message of a rendering, as Discord takes it: its text, and a row of
/// buttons for each decision the text carries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Piece {
    pub text: String,
    pub rows: Vec<Vec<Control>>,
}

impl Piece {
    fn is_empty(&self) -> bool {
        self.text.is_empty() && self.rows.is_empty()
    }

    /// Whether one more line of this length, with a row of controls or none,
    /// still fits in this message.
    fn fits(&self, line: &str, row: bool) -> bool {
        let text = self.text.chars().count() + line.chars().count() + 1;
        text <= TEXT_LIMIT && (!row || self.rows.len() < ROWS_LIMIT)
    }

    fn push(&mut self, line: &str, row: Vec<Control>) {
        self.text.push_str(line);
        self.text.push('\n');
        if !row.is_empty() {
            self.rows.push(row);
        }
    }

    /// The message as serenity sends it. Nothing in it mentions anyone and
    /// no link unfurls: a rendering is read, it does not ping (18).
    fn message(&self) -> CreateMessage {
        CreateMessage::new()
            .content(self.text.trim_end())
            .components(
                self.rows
                    .iter()
                    .map(|row| {
                        CreateActionRow::Buttons(
                            row.iter()
                                .map(|control| {
                                    CreateButton::new(&control.id)
                                        .label(&control.label)
                                        .style(match control.look {
                                            Look::Affirmative => ButtonStyle::Primary,
                                            Look::Drop => ButtonStyle::Danger,
                                            Look::Plain => ButtonStyle::Secondary,
                                        })
                                })
                                .collect(),
                        )
                    })
                    .collect(),
            )
            .allowed_mentions(CreateAllowedMentions::new())
            .flags(MessageFlags::SUPPRESS_EMBEDS)
    }
}

/// The buttons one line carries: an answer that needs no words of the
/// operator's own, up to a row's worth. An answer that takes text — `redo:
/// <notes>` — is left to the numbered reply the line itself spells out, and so
/// is any beyond the row, because the grammar carries every answer a control
/// does not (155a, 194).
pub fn controls(line: &Line) -> Vec<Control> {
    let Some(number) = line.number else {
        return vec![];
    };
    line.controls
        .iter()
        .filter(|answer| !answer.contains('<'))
        .enumerate()
        .filter_map(|(at, answer)| {
            let id = format!("{CONTROL}{number}:{answer}");
            (id.chars().count() <= CONTROL_ID_LIMIT).then(|| Control {
                id,
                label: clip(&format!("{number} {answer}"), LABEL_LIMIT),
                look: match (at, answer.as_str()) {
                    (_, "drop") => Look::Drop,
                    (0, _) => Look::Affirmative,
                    _ => Look::Plain,
                },
            })
        })
        .take(BUTTONS_LIMIT)
        .collect()
}

/// A rendering as the messages Discord accepts, in order (18, 309).
///
/// One post is one message where it fits, and it fits until it carries more
/// than five decisions or two thousand characters; past that it goes on in the
/// next message, each decision's row of buttons in the message whose text
/// carries its line. The tail, the notices and the link to the page follow the
/// decisions. Every line is the line `Recorded` carries, so the numbered reply
/// grammar works beside the buttons (309, 194).
pub fn pieces(post: &Post) -> Vec<Piece> {
    let mut out = Vec::new();
    let mut current = Piece::default();
    let mut add = |line: String, row: Vec<Control>, current: &mut Piece| {
        let line = clip(&line, TEXT_LIMIT - 1);
        if !current.fits(&line, !row.is_empty()) {
            out.push(std::mem::take(current));
        }
        current.push(&line, row);
    };
    for line in &post.lines {
        add(line.text(), controls(line), &mut current);
    }
    for line in &post.tail {
        add(line.text(), vec![], &mut current);
    }
    for notice in &post.notices {
        add(chat::notice_text(notice), vec![], &mut current);
    }
    add(post.link.clone(), vec![], &mut current);
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// The first `limit` characters of a text, marked where it was cut.
fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(limit.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// The number and the answer a press of one of this channel's own buttons
/// gives, or none for an id this channel did not post.
pub fn control_of(id: &str) -> Option<(u32, String)> {
    let (number, answer) = id.strip_prefix(CONTROL)?.split_once(':')?;
    let number = number.parse().ok()?;
    (!answer.is_empty()).then(|| (number, answer.to_string()))
}

// ------------------------------------------------------------- what arrives

/// What arrived from the platform and waits for the host to hand it to the
/// sink, which reads it exactly as it reads what `Recorded` is given (194).
#[derive(Clone)]
pub struct Inbox {
    channel: ChannelId,
    member: Option<String>,
    http: Arc<Http>,
    waiting: Arc<Mutex<Vec<chat::Message>>>,
    /// Presses acknowledged and not yet answered, by the id the sink knows
    /// them by, holding the interaction's token the answer is written under.
    pressed: Arc<Mutex<BTreeMap<String, String>>>,
    /// What the listener could not do, said once to whoever asks next.
    trouble: Arc<Mutex<Option<String>>>,
    /// What to call when something arrives, so the host takes it up now and
    /// not at its next poll (130, D6).
    wake: Arc<Mutex<Option<Arc<dyn Fn() + Send + Sync>>>>,
    /// The newest message of the channel accounted for: at first the delivery
    /// the sink's mark records, then each message seen after it. What waited
    /// is read from after it (217f).
    after: Arc<Mutex<Option<MessageId>>>,
    /// The bot this channel speaks as, whose replies say what it has answered.
    me: Arc<Mutex<Option<UserId>>>,
    /// Every message handed over, so one that comes both from the gateway and
    /// from the history is handed over once (137).
    handed: Arc<Mutex<BTreeSet<MessageId>>>,
}

impl Inbox {
    /// One message posted in the channel (194, 112, 215).
    ///
    /// A message elsewhere, or one a bot wrote — this channel's own renderings
    /// and replies among them — is not the sink's, and is left where it is. A
    /// forward is what the platform says it is, a reference of the forward
    /// kind to the message it carries, and never what its text looks like. A
    /// message already handed over is not handed over again.
    pub fn message(&self, posted: &Posted) -> bool {
        if posted.channel_id != self.channel {
            return false;
        }
        self.seen(posted.id);
        if posted.author.bot {
            return false;
        }
        let mut heard = chat::Message::new(
            &posted.id.to_string(),
            &self.by(&posted.author),
            &posted.content,
        );
        let forward = posted
            .message_reference
            .as_ref()
            .filter(|reference| reference.kind == MessageReferenceKind::Forward);
        if let Some(reference) = forward {
            let Some(original) = reference.message_id else {
                return false;
            };
            let guild = reference
                .guild_id
                .map(|guild| guild.to_string())
                .unwrap_or_else(|| "@me".into());
            heard = heard.forwarding(
                &format!("message/{original}"),
                &format!("{MESSAGE_LINK}/{guild}/{}/{original}", reference.channel_id),
            );
        }
        if !self
            .handed
            .lock()
            .expect("the inbox is poisoned")
            .insert(posted.id)
        {
            return false;
        }
        self.arrive(heard);
        true
    }

    /// Read what waited from after this delivery: the one the sink's mark
    /// records when the presenter starts listening (217f). A delivery that is
    /// no Discord message's id was made through another channel, and nothing
    /// is read after it.
    pub fn after(&self, delivery: Option<&str>) {
        if let Some(id) = delivery.and_then(|d| d.parse::<u64>().ok()).filter(|id| *id != 0) {
            self.seen(MessageId::new(id));
        }
    }

    /// Read the messages written in the channel while nobody listened, and
    /// hand each over as if it had just arrived (217f, 137, 154).
    ///
    /// The gateway replays nothing, so a reply typed while the host slept is
    /// heard only here: every message after the newest accounted for, oldest
    /// first. One this bot has already replied to was answered by an earlier
    /// run and is not handed over again, so a restart acknowledges nothing a
    /// second time. With nothing accounted for — the sink never delivered here
    /// — nothing is read. Returns how many were handed over.
    pub async fn waited(&self) -> Result<usize> {
        let Some(mut after) = *self.after.lock().expect("the inbox is poisoned") else {
            return Ok(0);
        };
        let me = self.me().await?;
        let mut read: Vec<Posted> = Vec::new();
        loop {
            let page = self
                .channel
                .messages(&*self.http, GetMessages::new().after(after).limit(HISTORY_PAGE))
                .await
                .map_err(said)
                .context("reading what was sent while nobody listened")?;
            let full = page.len() == usize::from(HISTORY_PAGE);
            let Some(newest) = page.iter().map(|posted| posted.id).max() else {
                break;
            };
            after = newest;
            read.extend(page);
            if !full {
                break;
            }
        }
        read.sort_by_key(|posted| posted.id);
        let answered: BTreeSet<MessageId> = read
            .iter()
            .filter(|posted| posted.author.id == me)
            .filter_map(|posted| posted.message_reference.as_ref()?.message_id)
            .collect();
        let mut handed = 0;
        for posted in &read {
            if answered.contains(&posted.id) {
                self.seen(posted.id);
            } else if self.message(posted) {
                handed += 1;
            }
        }
        // What the gateway handed over meanwhile keeps its place in the order
        // the messages were written.
        self.waiting
            .lock()
            .expect("the inbox is poisoned")
            .sort_by_key(|message| message.id.parse::<u64>().unwrap_or(u64::MAX));
        Ok(handed)
    }

    /// The bot this channel speaks as, asked of Discord once.
    async fn me(&self) -> Result<UserId> {
        if let Some(me) = *self.me.lock().expect("the inbox is poisoned") {
            return Ok(me);
        }
        let me = self
            .http
            .get_current_user()
            .await
            .map_err(said)
            .context("asking Discord which bot this is")?
            .id;
        *self.me.lock().expect("the inbox is poisoned") = Some(me);
        Ok(me)
    }

    /// A message of the channel accounted for, so what waited is read from
    /// after it.
    fn seen(&self, id: MessageId) {
        let mut after = self.after.lock().expect("the inbox is poisoned");
        if after.map_or(true, |at| at < id) {
            *after = Some(id);
        }
    }

    /// One of this channel's buttons pressed (309, 155).
    ///
    /// Discord wants a press acknowledged within three seconds, and recording
    /// the answer waits on the host, so the press is acknowledged at once and
    /// privately, and the sink's reply fills that acknowledgement in (153,
    /// 154). The press reaches the sink as the numbered reply `412: yes`,
    /// which the grammar reads as that answer to that decision and never as a
    /// command: the words are the model's own, off this channel's control
    /// (194).
    pub async fn press(&self, press: &ComponentInteraction) -> Result<bool> {
        if press.channel_id != self.channel {
            return Ok(false);
        }
        let Some((number, answer)) = control_of(&press.data.custom_id) else {
            return Ok(false);
        };
        self.http.set_application_id(press.application_id);
        press
            .defer_ephemeral(&*self.http)
            .await
            .map_err(said)
            .context("acknowledging the press")?;
        let id = press.id.to_string();
        self.pressed
            .lock()
            .expect("the inbox is poisoned")
            .insert(id.clone(), press.token.clone());
        self.arrive(chat::Message::new(
            &id,
            &self.by(&press.user),
            &format!("{number}: {answer}"),
        ));
        Ok(true)
    }

    /// Who a message is given by (153). In this release the manifest's
    /// operator is who every response records (D10): a sink that belongs to a
    /// member records that member, and a shared channel, which belongs to
    /// nobody, records the platform's own name for who wrote it (236).
    fn by(&self, user: &User) -> String {
        self.member.clone().unwrap_or_else(|| user.name.clone())
    }

    fn arrive(&self, message: chat::Message) {
        self.waiting
            .lock()
            .expect("the inbox is poisoned")
            .push(message);
        let wake = self.wake.lock().expect("the inbox is poisoned").clone();
        if let Some(wake) = wake {
            wake();
        }
    }

    fn troubled(&self, what: String) {
        *self.trouble.lock().expect("the inbox is poisoned") = Some(what);
        let wake = self.wake.lock().expect("the inbox is poisoned").clone();
        if let Some(wake) = wake {
            wake();
        }
    }
}

/// The gateway's events, handed to the inbox.
struct Listener {
    inbox: Inbox,
}

#[serenity::async_trait]
impl EventHandler for Listener {
    /// Connected, at first or again after the connection was lost: what was
    /// sent meanwhile is read, since the gateway replays none of it (217f).
    async fn ready(&self, _: Context, ready: Ready) {
        self.inbox.http.set_application_id(ready.application.id);
        *self.inbox.me.lock().expect("the inbox is poisoned") = Some(ready.user.id);
        if let Err(e) = self.inbox.waited().await {
            self.inbox.troubled(format!("{e:#}"));
        }
    }

    async fn message(&self, _: Context, posted: Posted) {
        self.inbox.message(&posted);
    }

    async fn interaction_create(&self, _: Context, interaction: Interaction) {
        if let Interaction::Component(press) = interaction {
            if let Err(e) = self.inbox.press(&press).await {
                self.inbox.troubled(format!("{e:#}"));
            }
        }
    }
}

// ---------------------------------------------------------------- the channel

/// What one Discord channel is opened on: the sink's own entry in the
/// manifest, read (D8).
#[derive(Debug, Clone)]
pub struct Settings {
    /// The sink this channel delivers for, which a refusal names.
    pub sink: String,
    /// The channel's id on the platform, which is the sink's surface.
    pub channel: u64,
    /// The member the sink belongs to, or none for a shared channel (236).
    pub member: Option<String>,
    /// Where the API is: Discord's own unless a caller says otherwise, which is
    /// how a test puts a recorded double in its place.
    pub api: Option<String>,
}

/// The thread a channel speaks to the platform on.
struct Worker {
    jobs: tokio::sync::mpsc::UnboundedSender<Pin<Box<dyn Future<Output = ()> + Send>>>,
}

impl Worker {
    fn start(sink: &str) -> Result<Worker> {
        let (jobs, mut queue) =
            tokio::sync::mpsc::unbounded_channel::<Pin<Box<dyn Future<Output = ()> + Send>>>();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("a runtime for the chat's wire")?;
        std::thread::Builder::new()
            .name(format!("discord {sink}"))
            .spawn(move || {
                runtime.block_on(async move {
                    while let Some(job) = queue.recv().await {
                        tokio::spawn(job);
                    }
                })
            })
            .context("a thread for the chat's wire")?;
        Ok(Worker { jobs })
    }

    /// Run one call on the thread and wait for what it gives back.
    fn run<T: Send + 'static>(
        &self,
        call: impl Future<Output = Result<T>> + Send + 'static,
    ) -> Result<T> {
        let (given, taken) = std::sync::mpsc::channel();
        self.jobs
            .send(Box::pin(async move {
                let _ = given.send(call.await);
            }))
            .map_err(|_| anyhow!("the chat's wire has stopped"))?;
        taken
            .recv_timeout(PATIENCE)
            .map_err(|_| anyhow!("Discord did not answer within {} seconds", PATIENCE.as_secs()))?
    }

    fn spawn(&self, job: impl Future<Output = ()> + Send + 'static) -> Result<()> {
        self.jobs
            .send(Box::pin(job))
            .map_err(|_| anyhow!("the chat's wire has stopped"))
    }
}

/// The Discord `Channel`: one sink's channel on the platform.
pub struct Discord {
    sink: String,
    channel: ChannelId,
    token: Token,
    api: Option<String>,
    http: Arc<Http>,
    inbox: Inbox,
    worker: Worker,
    /// What Discord said when it refused the token. Nothing the host does
    /// makes a refused token good, so the channel asks nothing more of the
    /// platform until the operator places another and restarts (204, 207).
    refused: Arc<Mutex<Option<String>>>,
}

/// A client of the API at Discord's own address or at the one given.
///
/// serenity's rate limiter builds its requests at Discord's own address
/// whatever the client was given, so a client at another address goes without
/// it; the address given is then the one limiting, as serenity documents.
fn http_at(token: &Token, api: Option<&str>) -> Http {
    let mut http = HttpBuilder::new(&token.placed);
    if let Some(api) = api {
        http = http.proxy(api).ratelimiter_disabled(true);
    }
    http.build()
}

impl std::fmt::Debug for Discord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Discord")
            .field("sink", &self.sink)
            .field("channel", &self.channel.get())
            .field("token", &self.token)
            .finish()
    }
}

impl Discord {
    /// Open the channel. Nothing is sent and nothing is listened to yet.
    pub fn open(settings: Settings, token: Token) -> Result<Discord> {
        let http = Arc::new(http_at(&token, settings.api.as_deref()));
        let channel = ChannelId::new(settings.channel);
        Ok(Discord {
            worker: Worker::start(&settings.sink)?,
            inbox: Inbox {
                channel,
                member: settings.member.clone(),
                http: http.clone(),
                waiting: Default::default(),
                pressed: Default::default(),
                trouble: Default::default(),
                wake: Default::default(),
                after: Default::default(),
                me: Default::default(),
                handed: Default::default(),
            },
            sink: settings.sink,
            channel,
            token,
            api: settings.api,
            http,
            refused: Default::default(),
        })
    }

    /// Refuse at once when Discord has already refused the token.
    fn check_the_token(&self) -> Result<()> {
        match self.refused.lock().expect("the channel is poisoned").as_ref() {
            Some(refusal) => bail!("{refusal}"),
            None => Ok(()),
        }
    }

    /// What a failed call leaves behind: said without its address, and
    /// remembered when it was the token Discord refused.
    fn failed(&self) -> impl Fn(serenity::Error) -> anyhow::Error + Send + 'static {
        let refused = self.refused.clone();
        let sink = self.sink.clone();
        let variable = self.token.variable.clone();
        move |error| {
            use serenity::http::HttpError;
            match &error {
                serenity::Error::Http(HttpError::UnsuccessfulRequest(response))
                    if response.status_code.as_u16() == 401 =>
                {
                    let refusal = format!(
                        "Discord refused `{sink}`'s bot token (401): place a valid one in \
                         `{variable}` and restart the host (204, 207)"
                    );
                    *refused.lock().expect("the channel is poisoned") = Some(refusal.clone());
                    anyhow!(refusal)
                }
                _ => said(error),
            }
        }
    }

    /// What arrives, for a caller handing the platform's events in itself.
    pub fn inbox(&self) -> &Inbox {
        &self.inbox
    }

    /// Call this whenever something arrives, so the host takes it up at once
    /// (130, D6).
    pub fn wake_with(&self, wake: Arc<dyn Fn() + Send + Sync>) {
        *self.inbox.wake.lock().expect("the inbox is poisoned") = Some(wake);
    }

    /// Listen on the gateway: the messages in the channel and the presses of
    /// its buttons (194, 309), and, each time the gateway is ready, what was
    /// written in the channel after `after` — the delivery the sink's mark
    /// records — while nobody listened (217f). The listener reconnects on its
    /// own; what it cannot get past — a token Discord refuses, an intent the
    /// application has not been granted — is said on the next `heard` (81).
    pub fn listen(&self, after: Option<&str>) -> Result<()> {
        // The gateway's client asks the same API where the gateway is, so a
        // channel opened on another address never reaches Discord's own.
        self.check_the_token()?;
        self.inbox.after(after);
        let http = http_at(&self.token, self.api.as_deref());
        let inbox = self.inbox.clone();
        self.worker.spawn(async move {
            let intents = GatewayIntents::GUILD_MESSAGES
                | GatewayIntents::DIRECT_MESSAGES
                | GatewayIntents::MESSAGE_CONTENT;
            let built = serenity::all::ClientBuilder::new_with_http(http, intents)
                .event_handler(Listener {
                    inbox: inbox.clone(),
                })
                .await;
            let outcome = match built {
                Ok(mut client) => client.start().await,
                Err(e) => Err(e),
            };
            if let Err(e) = outcome {
                inbox.troubled(format!("the chat stopped listening: {}", said(e)));
            }
        })
    }
}

impl Channel for Discord {
    /// Post one rendering, as many messages as Discord's limits make it, and
    /// hand back the last one's id, which the mark records (14).
    fn post(&mut self, post: &Post) -> Result<String> {
        self.check_the_token()?;
        let pieces = pieces(post);
        let http = self.http.clone();
        let channel = self.channel;
        let failed = self.failed();
        let last = self.worker.run(async move {
            let mut last = None;
            for piece in pieces {
                let sent = channel
                    .send_message(&*http, piece.message())
                    .await
                    .map_err(&failed)
                    .context("posting the rendering")?;
                last = Some(sent.id);
            }
            last.ok_or_else(|| anyhow!("the rendering made no message"))
        })?;
        // A sink that had never delivered here reads what waits from after
        // its first delivery (217f).
        self.inbox
            .after
            .lock()
            .expect("the inbox is poisoned")
            .get_or_insert(last);
        Ok(last.to_string())
    }

    /// Answer one message: a press by filling in its acknowledgement, which
    /// only the one who pressed sees, and a message by a reply to it in the
    /// channel (154, 194).
    fn reply(&mut self, to: &str, text: &str) -> Result<()> {
        self.check_the_token()?;
        let text = clip(text, TEXT_LIMIT);
        let http = self.http.clone();
        let failed = self.failed();
        let pressed = self
            .inbox
            .pressed
            .lock()
            .expect("the inbox is poisoned")
            .remove(to);
        match pressed {
            Some(token) => self.worker.run(async move {
                http.edit_original_interaction_response(
                    &token,
                    &EditInteractionResponse::new().content(text),
                    vec![],
                )
                .await
                .map_err(&failed)
                .context("answering the press")?;
                Ok(())
            }),
            None => {
                let message: u64 = to
                    .parse()
                    .with_context(|| format!("`{to}` is no message of this channel"))?;
                let channel = self.channel;
                self.worker.run(async move {
                    channel
                        .send_message(
                            &*http,
                            CreateMessage::new()
                                .content(text)
                                .reference_message((channel, MessageId::new(message)))
                                .allowed_mentions(CreateAllowedMentions::new())
                                .flags(MessageFlags::SUPPRESS_EMBEDS),
                        )
                        .await
                        .map_err(&failed)
                        .context("replying")?;
                    Ok(())
                })
            }
        }
    }

    /// What arrived since the last look, or what stopped the listener (81).
    fn heard(&mut self) -> Result<Vec<chat::Message>> {
        if let Some(trouble) = self
            .inbox
            .trouble
            .lock()
            .expect("the inbox is poisoned")
            .take()
        {
            bail!("{trouble}");
        }
        Ok(std::mem::take(
            &mut *self.inbox.waiting.lock().expect("the inbox is poisoned"),
        ))
    }
}

/// What went wrong, in words that carry no address: a request's own URL can
/// hold a press's token, so it is left out of anything a log or a record could
/// keep (204, 207).
fn said(error: serenity::Error) -> anyhow::Error {
    use serenity::http::HttpError;
    match error {
        serenity::Error::Http(HttpError::UnsuccessfulRequest(refused)) => anyhow!(
            "Discord refused it: {} {}",
            refused.status_code.as_u16(),
            refused.error.message
        ),
        serenity::Error::Http(HttpError::Request(request)) => {
            anyhow!("Discord could not be reached: {}", request.without_url())
        }
        other => anyhow!("{other}"),
    }
}
