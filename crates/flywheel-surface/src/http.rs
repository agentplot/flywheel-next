//! The catalogue's one transport in this phase (193, D9).
//!
//! The routes here hold no operation of their own: each one reads or writes
//! through the same catalogue function the in-process caller uses, so a further
//! transport adds a client and not an operation.

use crate::catalogue::{self, Call};
use crate::page;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
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
) -> impl IntoResponse {
    let form = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|t| t.starts_with("application/x-www-form-urlencoded"));
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
            }
        },
    };
    if let Err(refused) = served.admits(host_of(&headers)) {
        return (StatusCode::FORBIDDEN, Json(json!({"refused": refused})));
    }
    let mut call = Call::new(&name, served.operator(), "page");
    call.args = input.args;
    call.delivery_id = input.delivery_id;
    let mut store = served.store.lock().await;
    let mut world = served.world.lock().await;
    match catalogue::call(&mut *store, &mut **world, &served.defs, &call) {
        Ok(outcome) => {
            // A page response is one of the three local causes, and it does not
            // wait for the poll (130, D6).
            served.woken.notify_one();
            (
                StatusCode::OK,
                Json(json!({"id": outcome.id, "recorded": catalogue::recorded(&outcome)})),
            )
        }
        Err(refused) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"refused": refused.to_string()})),
        ),
    }
}

/// `POST /api/curate`: the curator's surface submitting its moves.
///
/// This is not a tool and is not the catalogue's (193). It is the operator
/// running the curation session (93b): the submission writes the `move`
/// deliverable the session's exit names, and reports that exit through the same
/// `write_report` that `flywheel exit` calls, so what a person submits here and
/// what a session's command writes are one record (67, D8, D16). `record_moves`
/// on the next tick is what applies them, unchanged (110, `curation.yaml`).
async fn curate<S: StateStore + Send + 'static>(
    State(served): State<Served<S>>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    if let Err(refused) = served.admits(host_of(&headers)) {
        return (StatusCode::FORBIDDEN, Json(json!({"refused": refused})));
    }
    let fields = form_fields(&String::from_utf8_lossy(&body));
    let mut store = served.store.lock().await;
    let mut world = served.world.lock().await;
    let at = match flywheel_domain::commands::now(&mut *store) {
        Ok(at) => at,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"refused": e.to_string()}))),
    };
    // The session the moves are delivered under. The page names it, and it is
    // checked against the store: a move is a session's delivery and never a
    // write of its own (110, 93b).
    let objects = match store.list_records(&flywheel_atoms::Scope::All) {
        Ok(objects) => objects,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"refused": e.to_string()}))),
    };
    let Some(session) = page::curating(&objects) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"refused": "no curation session is charged; the tick charges one when \
                        the unmoved signals cross the threshold or the cadence says so (110)"})),
        );
    };
    let mut moves: Vec<flywheel_domain::signals::Move> = Vec::new();
    for (name, value) in &fields {
        let Some(signal) = name.strip_prefix("move.") else {
            continue;
        };
        let word = value.as_str().unwrap_or_default().trim();
        // A signal the operator left alone stays unmoved, and the next run sees
        // it again: nothing is judged by omission (107, 118).
        if word.is_empty() {
            continue;
        }
        let names = fields
            .get(&format!("target.{signal}"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .trim()
            .to_string();
        moves.push(flywheel_domain::signals::Move {
            signal: signal.to_string(),
            target: match names.is_empty() {
                true => word.to_string(),
                false => format!("{word} {names}"),
            },
            reason: "the operator curated it on the page".into(),
            at: at.to_rfc3339(),
        });
    }
    let mut written = 0usize;
    for moved in &moves {
        match flywheel_domain::signals::write_move(&mut **world, moved) {
            Ok(_) => written += 1,
            Err(refused) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"refused": refused.to_string()})),
                )
            }
        }
    }
    // The exit the session reports, with `move` as what it delivered: the same
    // entry `flywheel exit done --deliverable move` writes (67, 80).
    let reported = flywheel_domain::report::write_report(
        &mut *store,
        &session,
        served.operator(),
        at,
        &flywheel_domain::report::Report::Exit {
            kind: "done".into(),
            deliverables: vec!["move".into()],
            question: None,
            text: None,
        },
    );
    match reported {
        Ok(_) => {
            // A session's report is a local cause too (130, D6).
            served.woken.notify_one();
            (
                StatusCode::OK,
                Json(json!({"recorded": true, "session": session, "moves": written})),
            )
        }
        Err(refused) => (
            StatusCode::BAD_REQUEST,
            Json(json!({"refused": refused.to_string()})),
        ),
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
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    if let Err(refused) = served.admits(host_of(&headers)) {
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
        Ok(read) => (StatusCode::OK, Html(page::render(&read))),
        Err(refused) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html(format!("<p class=\"refused\">{refused}</p>")),
        ),
    }
}

/// The router the page and the chat are served by.
pub fn router<S: StateStore + Send + 'static>(served: Served<S>) -> Router {
    Router::new()
        .route("/", get(page::<S>))
        .route("/api/tools", get(tools::<S>))
        .route("/api/tools/:name", post(invoke::<S>))
        .route("/api/curate", post(curate::<S>))
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
