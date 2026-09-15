//! `GET /events`: the host telling every open page when its store moved
//! (S221).

use crate::http::Served;
use flywheel_atoms::testing::{FakeStore, FakeWorld};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Read the stream until it has said what is wanted, for at most five seconds.
async fn until(socket: &mut tokio::net::TcpStream, heard: &mut String, wanted: &str) {
    let mut buffer = [0u8; 1024];
    while !heard.contains(wanted) {
        let read = tokio::time::timeout(std::time::Duration::from_secs(5), socket.read(&mut buffer))
            .await
            .unwrap_or_else(|_| panic!("the stream never said `{wanted}`: {heard}"))
            .expect("the stream reads");
        assert!(read > 0, "the stream closed before `{wanted}`: {heard}");
        heard.push_str(&String::from_utf8_lossy(&buffer[..read]));
    }
}

/// An open page hears the generation it may compare with the one it was
/// rendered at, and a line each time the store moves after that (S221).
#[test]
fn an_open_page_hears_the_generation_and_every_change() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
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
        tokio::spawn(crate::http::serve_on(served.clone(), listener));

        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        socket
            .write_all(b"GET /events HTTP/1.1\r\nHost: localhost:4242\r\n\r\n")
            .await
            .unwrap();
        let mut heard = String::new();
        until(&mut socket, &mut heard, "data: 1\n").await;
        assert!(
            heard.to_ascii_lowercase().contains("content-type: text/event-stream"),
            "{heard}"
        );

        served.moved();
        until(&mut socket, &mut heard, "data: 2\n").await;
        served.moved();
        served.moved();
        until(&mut socket, &mut heard, "data: 4\n").await;
    });
}
