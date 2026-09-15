//! The transport's own behaviour: which address serves what, and how a response
//! goes over the wire (205a, 310a, S235).

use crate::http::Served;
use flywheel_atoms::testing::{FakeStore, FakeWorld};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A page served over the fake store on a loopback port of its own, reached as
/// the operator at the machine reaches it (245, 253a).
async fn a_served_page() -> std::net::SocketAddr {
    let defs = flywheel_domain::set::load().expect("the embedded definitions");
    let served = Served::over(
        FakeStore::default(),
        Box::new(FakeWorld::new()),
        defs,
        &["chuck".to_string()],
        "http://studio.tailnet.ts.net/willdan",
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(crate::http::serve_on(served, listener));
    address
}

/// One request with the headers given, and the whole reply as it came.
async fn asked(address: std::net::SocketAddr, path: &str, headers: &str) -> Vec<u8> {
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost:4242\r\nConnection: close\r\n{headers}\r\n");
    socket.write_all(request.as_bytes()).await.unwrap();
    let mut reply = Vec::new();
    socket.read_to_end(&mut reply).await.unwrap();
    reply
}

fn in_a_runtime(test: impl std::future::Future<Output = ()>) {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(test);
}

/// The instance's address is the page whether it is written as a name or as a
/// directory (205a).
#[test]
fn the_instances_address_as_a_directory_is_the_page() {
    in_a_runtime(async {
        let address = a_served_page().await;
        for path in ["/willdan", "/willdan/"] {
            let reply = String::from_utf8_lossy(&asked(address, path, "").await).to_string();
            assert!(reply.starts_with("HTTP/1.1 200"), "{path}: {}", reply.lines().next().unwrap_or_default());
            assert!(reply.contains("id=\"rail\""), "{path} is not the page");
        }
    });
}

