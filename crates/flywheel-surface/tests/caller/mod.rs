//! An HTTP caller for the tests: the page's transport, spoken over a socket so
//! a test asserts what a client would actually receive and not what a handler
//! returns to itself.
//!
//! It writes the request bytes and reads the response bytes, so nothing is
//! fetched from anywhere else here either.
#![allow(dead_code)]

pub use flywheel_surface::testing as world;

use flywheel_atoms::World;
use flywheel_store_git::GitStore;
use flywheel_surface::http::Served;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::TcpStream;

/// A served page, on a port the operating system picked, with the store behind
/// it. It answers requests for as long as it is held.
pub struct Server {
    address: std::net::SocketAddr,
    /// The host name a request is made to, which the page reads to decide
    /// whether it may be served unsigned-in (253a).
    pub host: String,
    runtime: tokio::runtime::Runtime,
    served: Served<GitStore>,
    task: tokio::task::JoinHandle<()>,
}

impl Server {
    pub fn start(store: GitStore, defs: flywheel_engine::Definitions, operator: &str) -> Server {
        Server::start_at(store, defs, operator, "http://host.example/instance")
    }

    pub fn start_at(
        store: GitStore,
        defs: flywheel_engine::Definitions,
        operator: &str,
        address: &str,
    ) -> Server {
        Server::for_operators(store, defs, &[operator.to_string()], address)
    }

    pub fn for_operators(
        store: GitStore,
        defs: flywheel_engine::Definitions,
        operators: &[String],
        address: &str,
    ) -> Server {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("a runtime");
        let served = Served::over(store, a_world(), defs, operators, address);
        let (address, task) = runtime.block_on({
            let served = served.clone();
            async move {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .expect("a port");
                let address = listener.local_addr().expect("an address");
                let task = tokio::spawn(async move {
                    let _ = axum::serve(listener, flywheel_surface::http::router(served)).await;
                });
                (address, task)
            }
        });
        let host = flywheel_surface::http::private_host(address_of(&served))
            .unwrap_or_else(|| "host.example".into());
        Server {
            address,
            host,
            runtime,
            served,
            task,
        }
    }

    pub fn get(&self, path: &str) -> Value {
        self.request("GET", path, None)
    }

    pub fn post(&self, path: &str, body: Value) -> Value {
        self.request("POST", path, Some(body))
    }

    /// The page's own controls: a plain form, sent as a form, with no script
    /// running and nothing fetched from anywhere else (310, 311).
    ///
    /// A browser submitting one is answered with a place to go, never a body:
    /// see `Posted`.
    pub fn form(&self, path: &str, fields: &[(&str, &str)]) -> Posted {
        self.form_from(path, fields, None)
    }

    /// The same, saying which page the control was on, which is what a browser
    /// sends and what decides where the operator lands (310).
    pub fn form_from(&self, path: &str, fields: &[(&str, &str)], referrer: Option<&str>) -> Posted {
        let host = self.host.clone();
        let body = fields
            .iter()
            .map(|(name, value)| format!("{name}={}", encode(value)))
            .collect::<Vec<_>>()
            .join("&");
        Posted::read(&speak_form(self.address, path, &body, &host, referrer))
    }

    /// Follow where a form post sent the operator, as a browser does.
    pub fn follow(&self, posted: &Posted) -> String {
        let to = posted.location.clone().expect("a form post says where to go");
        self.html(&to)
    }

    /// The page itself, as a document.
    pub fn html(&self, path: &str) -> String {
        self.html_at(path, &self.host)
    }

    /// The page as reached at one address, which is what decides whether it is
    /// served unsigned-in (253a).
    pub fn html_at(&self, path: &str, host: &str) -> String {
        speak(self.address, "GET", path, None, host)
    }

    /// The served bindings, for a test that asks what the host listens on.
    pub fn served(&self) -> &Served<GitStore> {
        &self.served
    }

    /// The world behind the page, for a test that reads back the material a
    /// capture wrote under the machinery's prefix (111, 203).
    pub fn with_world<T>(&self, read: impl FnOnce(&mut (dyn World + Send)) -> T) -> T {
        self.runtime.block_on(async {
            let mut world = self.served.world.lock().await;
            read(&mut **world)
        })
    }

    /// The store behind the page, for a test that reads back what a call wrote.
    pub fn with_store<T>(&self, read: impl FnOnce(&mut GitStore) -> T) -> T {
        self.runtime.block_on(async {
            let mut store = self.served.store.lock().await;
            read(&mut store)
        })
    }

    fn request(&self, method: &str, path: &str, body: Option<Value>) -> Value {
        let text = speak(self.address, method, path, body, &self.host);
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("the body {text:?}: {e}"))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// One request over a socket, with no client library between.
fn address_of(served: &Served<GitStore>) -> &str {
    &served.address
}

fn speak(
    address: std::net::SocketAddr,
    method: &str,
    path: &str,
    body: Option<Value>,
    host: &str,
) -> String {
    let mut socket = TcpStream::connect(address).expect("the served address");
    let payload = body.map(|b| b.to_string()).unwrap_or_default();
    let mut head =
        format!("{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n");
    if !payload.is_empty() {
        head.push_str("Content-Type: application/json\r\n");
        head.push_str(&format!("Content-Length: {}\r\n", payload.len()));
    }
    head.push_str("\r\n");
    socket.write_all(head.as_bytes()).expect("the request");
    socket.write_all(payload.as_bytes()).expect("the body");
    let mut answer = String::new();
    socket.read_to_string(&mut answer).expect("the response");
    answer
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or(answer)
}

/// One form post, encoded the way a browser encodes one.
/// What a browser gets back from a control on the page: a status, somewhere to
/// go, and — only where the page refused before it read the form at all — a
/// body (310, 311).
pub struct Posted {
    pub status: u16,
    pub location: Option<String>,
    pub body: String,
}

impl Posted {
    fn read(answer: &str) -> Posted {
        let (head, body) = answer
            .split_once("\r\n\r\n")
            .map(|(h, b)| (h.to_string(), b.to_string()))
            .unwrap_or_else(|| (answer.to_string(), String::new()));
        let status = head
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .and_then(|code| code.parse().ok())
            .unwrap_or(0);
        let location = head
            .lines()
            .find_map(|line| line.strip_prefix("location: ").or_else(|| line.strip_prefix("Location: ")))
            .map(|to| to.trim().to_string());
        Posted { status, location, body }
    }

    /// Whether the control did what it said: the operator is sent back to a
    /// page, with nothing refused on the way (310).
    pub fn recorded(&self) -> bool {
        self.status == 303 && self.refused().is_none()
    }

    /// What the operator is told was refused, read where they will read it: off
    /// the page they are sent back to.
    pub fn refused(&self) -> Option<String> {
        if let Some(query) = self.location.as_deref().and_then(|to| to.split_once("?refused=")) {
            return Some(decode(query.1));
        }
        match self.body.contains("class=\"refused\"") {
            true => Some(self.body.clone()),
            false => None,
        }
    }
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'%' if at + 2 < bytes.len() => {
                match u8::from_str_radix(&text[at + 1..at + 3], 16) {
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
            b'+' => {
                out.push(b' ');
                at += 1;
            }
            other => {
                out.push(other);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn speak_form(
    address: std::net::SocketAddr,
    path: &str,
    body: &str,
    host: &str,
    referrer: Option<&str>,
) -> String {
    let mut socket = TcpStream::connect(address).expect("the served address");
    // A browser says which page the form was on; that is what sends the
    // operator back where they were (310).
    let referrer = format!("Referer: http://{host}{}\r\n", referrer.unwrap_or("/"));
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n{referrer}\
         Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n",
        body.len()
    );
    socket.write_all(head.as_bytes()).expect("the request");
    socket.write_all(body.as_bytes()).expect("the body");
    let mut answer = String::new();
    socket.read_to_string(&mut answer).expect("the response");
    answer
}

fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// The world a served page writes its material into, for a test.
pub fn a_world() -> Box<dyn World + Send> {
    Box::new(world::Files::new())
}
