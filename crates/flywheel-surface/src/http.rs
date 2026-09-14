//! The catalogue's one transport in this phase (193, D9).
//!
//! The routes here hold no operation of their own: each one reads or writes
//! through the same catalogue function the in-process caller uses, so a further
//! transport adds a client and not an operation.

use crate::catalogue::{self, Call};
use crate::page;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use flywheel_atoms::{StateStore, World};
use flywheel_engine::Definitions;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// What the transport holds: the store every call writes through, the
/// definitions it ticks against, and the identity every response records as
/// given by — the operators list's single entry while the instance has one
/// (153, 236a, 253a).
pub struct Served<S: StateStore + Send + 'static> {
    pub store: Arc<Mutex<S>>,
    /// The world the capture box writes its material into: the blueprints, under
    /// the machinery's prefix (111, 203). Held beside the store because a
    /// capture is one write to each.
    pub world: Arc<Mutex<Box<dyn World + Send>>>,
    pub defs: Arc<Definitions>,
    /// The instance's operators list (236a). While it holds a single entry a
    /// self-managed host on the operator's private network may serve the page
    /// with no sign-in, and that entry is the identity every response records
    /// as given by (253a, 153).
    pub operators: Vec<String>,
    /// The host's own address, which every link on the page is written at
    /// (205a, 308, D10a).
    pub address: String,
    /// The port the operator at the machine uses, which 245 permits beside the
    /// private-network address (46, 155).
    pub localhost_port: u16,
    /// What a local cause wakes. A page response, a chat message and a
    /// session's report notify in process at once and do not wait for the poll
    /// (130, D6, task 8.10): a write through this transport signals here, and
    /// the loop serving beside it takes its next pass without sleeping out the
    /// 30 seconds. A caller with no loop beside it simply never listens.
    pub woken: Arc<tokio::sync::Notify>,
}

impl<S: StateStore + Send + 'static> Clone for Served<S> {
    fn clone(&self) -> Self {
        Served {
            store: self.store.clone(),
            world: self.world.clone(),
            defs: self.defs.clone(),
            operators: self.operators.clone(),
            address: self.address.clone(),
            localhost_port: self.localhost_port,
            woken: self.woken.clone(),
        }
    }
}

impl<S: StateStore + Send + 'static> Served<S> {
    /// The page, with the world its capture box writes into (111, 203).
    pub fn over(
        store: S,
        world: Box<dyn World + Send>,
        defs: Definitions,
        operators: &[String],
        address: &str,
    ) -> Served<S> {
        Served {
            store: Arc::new(Mutex::new(store)),
            world: Arc::new(Mutex::new(world)),
            defs: Arc::new(defs),
            operators: operators.to_vec(),
            address: address.to_string(),
            localhost_port: 4242,
            woken: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// The instance this host serves, as it stands in the path of every link
    /// the machinery writes (205a, 308).
    pub fn instance(&self) -> &str {
        self.address.trim_end_matches('/').rsplit('/').next().unwrap_or_default()
    }

    /// The identity every response records as given by: the operators list's
    /// single entry (153, 236a, 253a).
    pub fn operator(&self) -> &str {
        self.operators.first().map(String::as_str).unwrap_or("")
    }

    /// The two addresses the host binds, and no others: its private-network
    /// address, and a localhost port for the operator at the machine. Nothing
    /// is published beyond that network (46, 155, 245).
    pub fn bound(&self) -> Vec<String> {
        let mut out = vec![];
        if let Some(host) = private_host(&self.address) {
            out.push(format!("{host}:80"));
        }
        out.push(format!("127.0.0.1:{}", self.localhost_port));
        out
    }

    /// Whether the page may be served unsigned-in to this request, and why not
    /// where it may not (253a).
    pub fn admits(&self, host_header: Option<&str>) -> Result<(), String> {
        if self.operators.len() != 1 {
            return Err(format!(
                "the operators list holds {} entries, so the page is not served unsigned-in; \
                 the exception stands while it holds one (253a)",
                self.operators.len()
            ));
        }
        let asked = host_header.unwrap_or_default();
        let name = asked.split(':').next().unwrap_or_default();
        let port = asked.rsplit_once(':').and_then(|(_, p)| p.parse::<u16>().ok());
        let private = private_host(&self.address).unwrap_or_default();
        let at_the_machine = crate::links::is_localhost(name) && port == Some(self.localhost_port);
        if name == private || at_the_machine {
            return Ok(());
        }
        Err(format!(
            "the page was reached at `{asked}`, which is neither the host's private-network \
             address `{private}` nor the operator's own port; it is not served unsigned-in there \
             (253a, 155)"
        ))
    }
}

/// The host name in an address, without the scheme or the instance path.
pub fn private_host(address: &str) -> Option<String> {
    let rest = address.split_once("//").map(|(_, r)| r).unwrap_or(address);
    let host = rest.split('/').next().unwrap_or_default();
    match host.is_empty() {
        true => None,
        false => Some(host.split(':').next().unwrap_or(host).to_string()),
    }
}

/// Where a control the operator used came from, so answering it returns them
/// there rather than to a body they have to go back from (310, 311).
///
/// The page is served at `/` for the operator at the machine and at
/// `/<instance>` and `/<instance>/<object>` for every link the machinery writes
/// (205a, 308), and a browser sends the address of the page the form was on as
/// the referrer. Only its path is used, and only a path this router serves:
/// nothing a request says decides where the operator is sent but the shape of
/// the page they were on.
fn came_from(headers: &axum::http::HeaderMap) -> String {
    let referrer = headers
        .get(axum::http::header::REFERER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let rest = referrer.split_once("//").map(|(_, r)| r).unwrap_or(referrer);
    let path = match referrer.contains("//") {
        true => rest.find('/').map(|at| &rest[at..]).unwrap_or("/"),
        false => rest,
    };
    let path = path.split(['?', '#']).next().unwrap_or("/");
    match path.starts_with('/') && !path.starts_with("//") && !path.starts_with("/api/") {
        true => path.to_string(),
        false => "/".to_string(),
    }
}

/// The page, with a reason on it. A refusal is not a dead end: the operator is
/// returned to the page they were on and reads there what was refused (81, 310).
fn back_with(path: &str, refused: &str) -> Response {
    to_the_page(&format!("{path}?refused={}", encode(refused)))
}

/// Back to the page, with nothing to say. See Other and not a redirect of the
/// post itself: what follows is a fresh read of the page, so a reload does not
/// answer twice (310).
fn to_the_page(location: &str) -> Response {
    (
        StatusCode::SEE_OTHER,
        [(axum::http::header::LOCATION, location.to_string())],
    )
        .into_response()
}

/// Percent-encode what goes in a query.
fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// What a page request says about the last control the operator used (310).
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct Asked {
    pub refused: Option<String>,
}

/// `GET /api/tools`: the catalogue as the HTTP caller enumerates it.
async fn tools<S: StateStore + Send + 'static>(
    State(served): State<Served<S>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    if let Err(refused) = served.admits(host_of(&headers)) {
        return (StatusCode::FORBIDDEN, Json(json!({"refused": refused})));
    }
    (StatusCode::OK, Json(catalogue::enumerate()))
}

/// The host the request was made to.
fn host_of(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::HOST)
        .and_then(|v| v.to_str().ok())
}

/// What a caller sends to invoke one tool: its arguments by object id, and the
/// delivery's own id where the caller has one (137, 193).
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct Invocation {
    args: BTreeMap<String, Value>,
    delivery_id: Option<String>,
}

/// `POST /api/tools/:name`: invoke one tool. The body is the same call the
/// in-process caller makes, and the record written is the same record.
///
/// A client sends it as JSON; the page's own controls are plain forms, which
/// send it form-encoded, because a control has to work with no script running
/// and nothing fetched from anywhere else (310, 311).
async fn invoke<S: StateStore + Send + 'static>(
    State(served): State<Served<S>>,
    Path(name): Path<String>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    // Which caller this is, taken from the request and not from a second route:
    // a form body is a control on the page, which has no script behind it and
    // must land back on a page; anything else is a client, which reads the
    // record it made (310, 311).
    let form = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|t| t.starts_with("application/x-www-form-urlencoded"));
    let back = came_from(&headers);
    let text = String::from_utf8_lossy(&body);
    let input: Invocation = match form {
        true => Invocation {
            args: form_fields(&text),
            delivery_id: None,
        },
        false if text.trim().is_empty() => Invocation::default(),
        false => match serde_json::from_str(&text) {
            Ok(read) => read,
            Err(unreadable) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"refused": unreadable.to_string()})),
                )
                    .into_response()
            }
        },
    };
    if let Err(refused) = served.admits(host_of(&headers)) {
        return match form {
            true => (
                StatusCode::FORBIDDEN,
                Html(format!("<p class=\"refused\">{refused}</p>")),
            )
                .into_response(),
            false => (StatusCode::FORBIDDEN, Json(json!({"refused": refused}))).into_response(),
        };
    }
    let mut call = Call::new(&name, served.operator(), "page");
    call.args = input.args;
    call.delivery_id = input.delivery_id;
    let mut store = served.store.lock().await;
    let mut world = served.world.lock().await;
    let outcome = catalogue::call(&mut *store, &mut **world, &served.defs, &call);
    // A page response is one of the three local causes, and it does not wait
    // for the poll (130, D6).
    if outcome.is_ok() {
        served.woken.notify_one();
    }
    match (outcome, form) {
        // The control the operator used sends them back to the page they were
        // on, which renders the answer they just gave (137, 310).
        (Ok(_), true) => to_the_page(&back),
        (Ok(outcome), false) => (
            StatusCode::OK,
            Json(json!({"id": outcome.id, "recorded": catalogue::recorded(&outcome)})),
        )
            .into_response(),
        (Err(refused), true) => back_with(&back, &refused.to_string()),
        (Err(refused), false) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"refused": refused.to_string()})),
        )
            .into_response(),
    }
}

/// `POST /api/answer-all`: the header's "yes all" control (11, S2, S7).
///
/// It sends one `answer` per approve decision, in number order, each recorded on
/// its own, and never a batch: "yes to all" has to mean something for a simple
/// rail, and any one of the decisions it answers had to be answerable alone
/// (11). It is the same tool the single control and the chat's `yes all` call,
/// so this adds no operation to the catalogue (193, D9).
async fn answer_all<S: StateStore + Send + 'static>(
    State(served): State<Served<S>>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let back = came_from(&headers);
    if let Err(refused) = served.admits(host_of(&headers)) {
        return (
            StatusCode::FORBIDDEN,
            Html(format!("<p class=\"refused\">{refused}</p>")),
        )
            .into_response();
    }
    // The numbers the control named when it was rendered, which travelled with
    // the tap. The page is rendered in one request and the control used in the
    // next: a sweep over a fresh read would answer a decision that arrived
    // between the two, and the operator never saw it (S2, S7, 15).
    let named: Vec<u32> = form_fields(&String::from_utf8_lossy(&body))
        .get("numbers")
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
        .split_whitespace()
        .filter_map(|n| n.parse::<u32>().ok())
        .collect();
    if named.is_empty() {
        return back_with(&back, "the control named no decision to answer");
    }
    let mut store = served.store.lock().await;
    let mut world = served.world.lock().await;
    // The rail as it stands, read and not renumbered: a request is not a tick
    // (15, D12).
    let decisions = match flywheel_domain::commands::rail_read(&*store, &served.defs) {
        Ok(decisions) => decisions,
        Err(e) => return back_with(&back, &e.to_string()),
    };
    // Of the numbers named, the ones that still stand and still take a yes. One
    // answered or retracted since the page was drawn is passed over, not
    // refused: approval given once is never re-asked and a yes is never applied
    // twice (6, 137).
    let answering: Vec<(u32, String)> = page::yes_all_answers(&decisions)
        .into_iter()
        .filter(|(number, _)| named.contains(number))
        .collect();
    if answering.is_empty() {
        return back_with(
            &back,
            "every decision the control named has been answered or retracted since the page \
             was drawn; nothing was answered twice (6)",
        );
    }
    let mut given = 0usize;
    for (number, _) in &answering {
        let mut call = Call::new(catalogue::ANSWER, served.operator(), "page");
        call.args.insert("decision".into(), json!(number));
        call.args.insert("answer".into(), Value::from("yes"));
        match catalogue::call(&mut *store, &mut **world, &served.defs, &call) {
            Ok(_) => given += 1,
            // One that cannot be recorded does not stop the rest: each of these
            // is its own response and the operator is told which did not go
            // (11, 81).
            Err(refused) => {
                return back_with(
                    &back,
                    &format!(
                        "{given} of {} answered; decision {number} was refused: {refused}",
                        answering.len()
                    ),
                )
            }
        }
    }
    served.woken.notify_one();
    to_the_page(&back)
}

/// `POST /api/curate`: the curator's surface submitting its moves.
///
/// The form is the page's control and the operation is the catalogue's
/// `curate` tool, which the chat and a client reach as `/api/tools/curate`
/// (193, 311). The route holds nothing of its own: it carries the form's
/// fields to the same function the in-process caller uses, and comes back to
/// the page (310, 311).
async fn curate<S: StateStore + Send + 'static>(
    State(served): State<Served<S>>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let back = came_from(&headers);
    if let Err(refused) = served.admits(host_of(&headers)) {
        return (
            StatusCode::FORBIDDEN,
            Html(format!("<p class=\"refused\">{refused}</p>")),
        )
            .into_response();
    }
    let mut call = Call::new("curate", served.operator(), "page");
    call.args = form_fields(&String::from_utf8_lossy(&body));
    let mut store = served.store.lock().await;
    let mut world = served.world.lock().await;
    match catalogue::call(&mut *store, &mut **world, &served.defs, &call) {
        Ok(_) => {
            // A session's report is a local cause too (130, D6).
            served.woken.notify_one();
            to_the_page(&back)
        }
        Err(refused) => back_with(&back, &refused.to_string()),
    }
}

/// A form body's fields, by object id. Percent-encoding and `+` for a space,
/// which is all a form sends.
fn form_fields(body: &str) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    for pair in body.split('&').filter(|p| !p.is_empty()) {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        out.insert(decode(name), Value::from(decode(value)));
    }
    out
}

fn decode(text: &str) -> String {
    let bytes = text.replace('+', " ").into_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'%' if at + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[at + 1..at + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        at += 3;
                    }
                    Err(_) => {
                        out.push(bytes[at]);
                        at += 1;
                    }
                }
            }
            other => {
                out.push(other);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// The page itself, rendered from the register and the objects on this request
/// and stored nowhere (15, 310).
async fn page<S: StateStore + Send + 'static>(
    State(served): State<Served<S>>,
    Query(asked): Query<Asked>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    rendered(&served, &headers, None, asked.refused).await
}

/// `GET /<instance>` — the page at the address every link the machinery writes
/// is built on, which is the same page the operator's own port serves (205a,
/// 308).
async fn page_of_instance<S: StateStore + Send + 'static>(
    State(served): State<Served<S>>,
    Path(instance): Path<String>,
    Query(asked): Query<Asked>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    if instance != served.instance() {
        return wrong_instance(&served, &instance);
    }
    rendered(&served, &headers, None, asked.refused).await
}

/// `GET /<instance>/<object>` — the link `links::to_object` writes, fetched.
///
/// An object's id carries slashes (`bolt/atlas/plan-rows`), so the rest of the
/// path is the id, and what comes back is the page with that object's surface
/// already open (205a, 209, 308).
async fn page_of_object<S: StateStore + Send + 'static>(
    State(served): State<Served<S>>,
    Path((instance, object)): Path<(String, String)>,
    Query(asked): Query<Asked>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    if instance != served.instance() {
        return wrong_instance(&served, &instance);
    }
    let object = object.trim_matches('/').to_string();
    {
        let store = served.store.lock().await;
        match store.get(&object) {
            Ok(Some(_)) => {}
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Html(format!(
                        "<p class=\"refused\">the instance holds no object `{}`</p>",
                        page::escape(&object)
                    )),
                )
            }
            Err(refused) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Html(format!("<p class=\"refused\">{refused}</p>")),
                )
            }
        }
    }
    rendered(&served, &headers, Some(object), asked.refused).await
}

/// `GET /<instance>/deliverable/<repository>/<path>` — a file a session left.
///
/// A session that finishes leaves real files at real paths, and the page links
/// to them, because an operator who can see that an elaboration is done and
/// cannot read what it produced has been shown nothing (190, 213).
///
/// Two kinds, and the difference is a safety property rather than a
/// preference. A markdown document is rendered into the page, escaped before a
/// tag is written, beside the object whose session delivered it. An HTML
/// deliverable is served **at its own address, as its own document** — it is
/// markup with its own head and its own styles, and putting it inside the page
/// would be letting a file write the page that shows it (310).
///
/// What may be read is what the state says was delivered, and nothing else.
/// The path is matched against the deliverables recorded on the objects, so a
/// request naming a file no session reported gets a 404 whatever it spells —
/// which closes the traversal question by construction rather than by
/// inspecting the path.
async fn deliverable<S: StateStore + Send + 'static>(
    State(served): State<Served<S>>,
    Path((instance, named)): Path<(String, String)>,
    headers: axum::http::HeaderMap,
) -> Response {
    if instance != served.instance() {
        return wrong_instance(&served, &instance).into_response();
    }
    if let Err(refused) = served.admits(host_of(&headers)) {
        return (
            StatusCode::FORBIDDEN,
            Html(format!("<p class=\"refused\">{refused}</p>")),
        )
            .into_response();
    }
    let named = named.trim_matches('/').to_string();
    let mut store = served.store.lock().await;
    let world = served.world.lock().await;
    let mut read = match page::read(
        &mut *store,
        &**world,
        &served.defs,
        &served.address,
        served.operator(),
    ) {
        Ok(read) => read,
        Err(refused) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p class=\"refused\">{refused}</p>")),
            )
                .into_response()
        }
    };
    // The one this names, among the ones a session actually reported.
    let found = read
        .delivered
        .iter()
        .find_map(|(object, held)| {
            held.iter().find(|file| file.named() == named).map(|file| (object.clone(), file.clone()))
        });
    let Some((object, file)) = found else {
        return (
            StatusCode::NOT_FOUND,
            Html(format!(
                "<p class=\"refused\">no session delivered `{}`; the page opens what the \
                 record says was delivered and nothing else (190, 213)</p>",
                page::escape(&named)
            )),
        )
            .into_response();
    };
    let body = match world.read_file(&file.repository, &file.path) {
        Ok(Some(body)) => body,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Html(format!(
                    "<p class=\"refused\">`{}` was delivered and is not in this host's \
                     checkout of `{}`; the session recorded it and the file is not there \
                     (190)</p>",
                    page::escape(&file.path),
                    page::escape(&file.repository)
                )),
            )
                .into_response()
        }
        Err(refused) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("<p class=\"refused\">{refused}</p>")),
            )
                .into_response()
        }
    };
    let text = String::from_utf8_lossy(&body).to_string();
    // Its own document, at its own address, with nothing of ours around it.
    if file.is_its_own_document() {
        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
            text,
        )
            .into_response();
    }
    // Everything else is read on the page, beside the object it belongs to.
    read.reading = Some(page::Reading {
        object,
        named: named.clone(),
        body: match file.path.to_ascii_lowercase().ends_with(".md") {
            true => crate::markdown::render(&text),
            // A record or a plain file is what it says, as it says it.
            false => format!("<pre class=\"plain\">{}</pre>", page::escape(&text)),
        },
    });
    (StatusCode::OK, Html(page::render(&read))).into_response()
}

/// What a link naming another instance gets: this host serves one (205a).
fn wrong_instance<S: StateStore + Send + 'static>(
    served: &Served<S>,
    asked: &str,
) -> (StatusCode, Html<String>) {
    (
        StatusCode::NOT_FOUND,
        Html(format!(
            "<p class=\"refused\">this host serves the instance `{}`, not `{}` (205a)</p>",
            page::escape(served.instance()),
            page::escape(asked)
        )),
    )
}

/// One read, one render, whatever path asked for it (310).
async fn rendered<S: StateStore + Send + 'static>(
    served: &Served<S>,
    headers: &axum::http::HeaderMap,
    opened: Option<String>,
    refused: Option<String>,
) -> (StatusCode, Html<String>) {
    if let Err(refused) = served.admits(host_of(headers)) {
        return (
            StatusCode::FORBIDDEN,
            Html(format!("<p class=\"refused\">{refused}</p>")),
        );
    }
    let mut store = served.store.lock().await;
    let world = served.world.lock().await;
    match page::read(
        &mut *store,
        &**world,
        &served.defs,
        &served.address,
        served.operator(),
    ) {
        Ok(mut read) => {
            read.opened = opened;
            read.refused = refused;
            (StatusCode::OK, Html(page::render(&read)))
        }
        Err(refused) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("<p class=\"refused\">{refused}</p>")),
        ),
    }
}

/// `POST /tour/next` — the operator asked for the scenario's next action
/// (`design/flywheel-next/scenarios/storefront.md`).
///
/// The control writes that the action is owed and nothing else. The host's own
/// loop plays it, the way it performs any other act that has come due, so the
/// request the operator is waiting on never runs a cascade and there is one
/// code path for an action however it was asked for (D11, 125).
///
/// Where the next action is a session's delivery the fact says it is owed two
/// seconds from now, and the page shows the agent working until then: the
/// machinery has already stalled where a real session would be, and the beat
/// is the viewer seeing that before the artifact appears.
async fn tour_next<S: StateStore + Send + 'static>(
    State(served): State<Served<S>>,
    headers: axum::http::HeaderMap,
) -> Response {
    let back = came_from(&headers);
    if let Err(refused) = served.admits(host_of(&headers)) {
        return (
            StatusCode::FORBIDDEN,
            Html(format!("<p class=\"refused\">{refused}</p>")),
        )
            .into_response();
    }
    let mut store = served.store.lock().await;
    // The moment the operator clicked, which is now — and not the point the
    // state was last read as of. A served host sets its clock at the top of a
    // pass, so between passes the as-of is up to a poll behind the wall, and a
    // beat stamped from it was over before it started.
    let at = chrono::Utc::now();
    let instance = served.instance();
    match flywheel_domain::tour::ask(&mut *store, &instance, at) {
        // No scenario here: there is nothing to step and no overlay to have
        // asked. A control that is not there cannot be pressed, so this is a
        // stale page rather than a refusal.
        Ok(None) => to_the_page(&back),
        Ok(Some(_)) => {
            // The loop is waiting on its poll; an action the operator asked
            // for is a local cause like any other (130, D6).
            served.woken.notify_waiters();
            to_the_page(&back)
        }
        Err(e) => back_with(&back, &e.to_string()),
    }
}

/// The router the page and the chat are served by.
pub fn router<S: StateStore + Send + 'static>(served: Served<S>) -> Router {
    Router::new()
        .route("/", get(page::<S>))
        .route("/tour/next", post(tour_next::<S>))
        .route("/api/tools", get(tools::<S>))
        .route("/api/tools/:name", post(invoke::<S>))
        .route("/api/answer-all", post(answer_all::<S>))
        .route("/api/curate", post(curate::<S>))
        // The address every link the machinery writes is built on has the
        // instance in its path, so the link it wrote is a path this router
        // serves (205a, 308).
        .route("/:instance", get(page_of_instance::<S>))
        // A file a session left behind, which the object's surface links to
        // (190, 213). It comes before the catch-all because `deliverable` is a
        // segment of the path and not the head of an object's id.
        .route("/:instance/deliverable/*named", get(deliverable::<S>))
        .route("/:instance/*object", get(page_of_object::<S>))
        .with_state(served)
}

/// Serve the router at an address, until the process ends.
pub async fn serve<S: StateStore + Send + 'static>(
    served: Served<S>,
    address: &str,
) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(address).await?;
    serve_on(served, listener).await
}

/// The same on a listener already bound, so a caller that must know the port
/// before the page is served can learn it — which is what the 390px pass needs
/// to open the rail on a loopback port of its own (314, D15).
pub async fn serve_on<S: StateStore + Send + 'static>(
    served: Served<S>,
    listener: tokio::net::TcpListener,
) -> anyhow::Result<()> {
    axum::serve(listener, router(served)).await?;
    Ok(())
}
