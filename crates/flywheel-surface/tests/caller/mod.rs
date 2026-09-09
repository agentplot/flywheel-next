//! An HTTP caller for the tests: the page's transport, spoken over a socket so
//! a test asserts what a client would actually receive and not what a handler
//! returns to itself.
//!
//! It writes the request bytes and reads the response bytes, so the bundle
//! ships no client of its own and the tests fetch nothing external either.

use serde_json::Value;
use std::io::{Read, Write};
use std::net::TcpStream;

/// Serve the router on a port the operating system picks, run one request
/// against it, and give back the parsed body.
pub fn get(path: &str) -> Value {
    request("GET", path, None)
}

/// Serve the router, send one request with an optional JSON body, and give back
/// the parsed response body.
pub fn request(method: &str, path: &str, body: Option<Value>) -> Value {
    let (method, path) = (method.to_string(), path.to_string());
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a port");
        let address = listener.local_addr().expect("an address");
        let served = tokio::spawn(async move {
            let _ = axum::serve(listener, flywheel_surface::http::router()).await;
        });
        let text = tokio::task::spawn_blocking(move || speak(address, &method, &path, body))
            .await
            .expect("the caller");
        served.abort();
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("the body {text:?}: {e}"))
    })
}

fn speak(
    address: std::net::SocketAddr,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> String {
    let mut socket = TcpStream::connect(address).expect("the served address");
    let payload = body.map(|b| b.to_string()).unwrap_or_default();
    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n"
    );
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
