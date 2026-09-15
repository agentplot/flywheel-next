//! Discord's HTTP API, recorded: a double on a port of this computer that the
//! chat's wire is pointed at in place of Discord's own (D9, D16).
//!
//! It keeps every request made of it — the method, the path, the authorization
//! it carried and the body — and answers each route the channel uses with the
//! payload Discord documents for it, recorded beside this file. What arrives
//! from the platform is handed in as the gateway hands it: a recorded
//! `MESSAGE_CREATE` or `INTERACTION_CREATE` payload read into serenity's own
//! model. Nothing here reaches the network.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// The channel the recorded payloads were posted in.
pub const CHANNEL: u64 = 1_200_000_000_000_000_000;

/// A message the bot posted, as Discord answers a post.
pub const POSTED: &str = include_str!("discord/posted.json");
/// A message the operator typed, as the gateway's `MESSAGE_CREATE` carries it.
pub const MESSAGE_CREATE: &str = include_str!("discord/message_create.json");
/// A message the operator forwarded into the channel.
pub const FORWARD: &str = include_str!("discord/forward.json");
/// A press of a button, as the gateway's `INTERACTION_CREATE` carries it.
pub const INTERACTION_CREATE: &str = include_str!("discord/interaction_create.json");

/// One request made of the double.
#[derive(Debug, Clone)]
pub struct Seen {
    pub method: String,
    pub path: String,
    pub authorization: Option<String>,
    pub body: Value,
}

#[derive(Clone)]
struct Recording {
    seen: Arc<Mutex<Vec<Seen>>>,
    refusing: Arc<AtomicBool>,
    next: Arc<AtomicU64>,
}

/// The double, running until it is dropped.
pub struct Double {
    /// Where it answers, which a channel is opened on as its API.
    pub address: String,
    recording: Recording,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Double {
    pub fn start() -> Double {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port for the double");
        listener
            .set_nonblocking(true)
            .expect("a listener that does not block");
        let address = format!("http://{}", listener.local_addr().expect("its address"));
        let recording = Recording {
            seen: Default::default(),
            refusing: Default::default(),
            next: Arc::new(AtomicU64::new(1_300_000_000_000_000_001)),
        };
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let state = recording.clone();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("a runtime for the double");
            runtime.block_on(async move {
                let listener =
                    tokio::net::TcpListener::from_std(listener).expect("the double's listener");
                let app = axum::Router::new().fallback(answer).with_state(state);
                let _ = axum::serve(listener, app)
                    .with_graceful_shutdown(async {
                        let _ = stopped.await;
                    })
                    .await;
            });
        });
        Double {
            address,
            recording,
            stop: Some(stop),
        }
    }

    /// Every request made of it so far, in order.
    pub fn seen(&self) -> Vec<Seen> {
        self.recording.seen.lock().expect("the double is poisoned").clone()
    }

    /// From here on every request is refused, as Discord refuses a token it
    /// does not know.
    pub fn refuse(&self) {
        self.recording.refusing.store(true, Ordering::SeqCst);
    }
}

impl Drop for Double {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

async fn answer(
    State(recording): State<Recording>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let path = uri.path().to_string();
    let sent: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    recording
        .seen
        .lock()
        .expect("the double is poisoned")
        .push(Seen {
            method: method.to_string(),
            path: path.clone(),
            authorization: headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .map(String::from),
            body: sent.clone(),
        });
    if recording.refusing.load(Ordering::SeqCst) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"message": "401: Unauthorized", "code": 0})),
        )
            .into_response();
    }
    let message = |channel: &str| {
        let mut message: Value = serde_json::from_str(POSTED).expect("the recorded message");
        message["id"] = json!(recording.next.fetch_add(1, Ordering::SeqCst).to_string());
        message["channel_id"] = json!(channel);
        message["content"] = sent["content"].clone();
        Json(message).into_response()
    };
    let api = path.strip_prefix("/api/v10").unwrap_or("");
    let posted_in = api
        .strip_prefix("/channels/")
        .and_then(|rest| rest.strip_suffix("/messages"))
        .filter(|channel| channel.parse::<u64>().is_ok());
    match (method, posted_in) {
        (Method::POST, Some(channel)) => message(channel),
        (Method::POST, None) if api.starts_with("/interactions/") && api.ends_with("/callback") => {
            StatusCode::NO_CONTENT.into_response()
        }
        (Method::PATCH, None)
            if api.starts_with("/webhooks/") && api.ends_with("/messages/@original") =>
        {
            message(&CHANNEL.to_string())
        }
        _ => (
            StatusCode::NOT_FOUND,
            Json(json!({"message": "404: Not Found", "code": 0})),
        )
            .into_response(),
    }
}

/// A message the operator typed in the channel, as the gateway hands it.
pub fn typed(id: &str, text: &str) -> serenity::all::Message {
    let mut message: Value = serde_json::from_str(MESSAGE_CREATE).expect("the recorded message");
    message["id"] = json!(id);
    message["content"] = json!(text);
    serde_json::from_value(message).expect("a recorded message reads")
}

/// A press of one of the channel's buttons, as the gateway hands it.
pub fn pressed(id: &str, control: &str) -> serenity::all::ComponentInteraction {
    let mut press: Value = serde_json::from_str(INTERACTION_CREATE).expect("the recorded press");
    press["id"] = json!(id);
    press["data"]["custom_id"] = json!(control);
    match serde_json::from_value(press).expect("a recorded press reads") {
        serenity::all::Interaction::Component(press) => press,
        other => panic!("the recorded press read as {other:?}"),
    }
}
