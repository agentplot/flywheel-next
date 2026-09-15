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

/// A reply's head, lowercased, and its body with any chunking undone.
fn split(reply: &[u8]) -> (String, Vec<u8>) {
    let at = reply.windows(4).position(|w| w == b"\r\n\r\n").expect("a head");
    let head = String::from_utf8_lossy(&reply[..at]).to_ascii_lowercase();
    let mut body = reply[at + 4..].to_vec();
    if head.contains("transfer-encoding: chunked") {
        let mut whole = Vec::new();
        let mut rest = &body[..];
        loop {
            let line = rest.windows(2).position(|w| w == b"\r\n").expect("a chunk's size");
            let size = std::str::from_utf8(&rest[..line]).expect("a size in hex");
            let size = usize::from_str_radix(size.split(';').next().unwrap_or_default().trim(), 16).expect("a size");
            rest = &rest[line + 2..];
            if size == 0 {
                break;
            }
            whole.extend_from_slice(&rest[..size]);
            rest = &rest[size + 2..];
        }
        body = whole;
    }
    (head, body)
}

/// One header's value in a lowercased head.
fn header<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines().find_map(|line| line.strip_prefix(&format!("{name}: ")))
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

/// Every response is compressed with the best encoding the request accepts: br
/// before gzip at the same weight, gzip where it weighs more or is all the
/// request takes, and nothing where nothing is accepted. What comes back
/// decodes to the page itself (310a, S235).
#[test]
fn a_response_is_compressed_with_what_the_request_accepts() {
    use std::io::Read;
    in_a_runtime(async {
        let address = a_served_page().await;
        let (head, plain) = split(&asked(address, "/willdan/", "").await);
        assert_eq!(header(&head, "content-encoding"), None, "{head}");
        assert!(String::from_utf8_lossy(&plain).contains("id=\"rail\""));

        let (head, gzipped) = split(&asked(address, "/willdan/", "Accept-Encoding: gzip\r\n").await);
        assert_eq!(header(&head, "content-encoding"), Some("gzip"), "{head}");
        let mut read = Vec::new();
        flate2::read::GzDecoder::new(&gzipped[..]).read_to_end(&mut read).expect("gzip decodes");
        assert_eq!(read, plain, "gzip decodes to the page");

        let (head, brotli) = split(&asked(address, "/willdan/", "Accept-Encoding: gzip, deflate, br\r\n").await);
        assert_eq!(header(&head, "content-encoding"), Some("br"), "{head}");
        assert!(header(&head, "vary").is_some_and(|v| v.contains("accept-encoding")), "{head}");
        let mut read = Vec::new();
        brotli::Decompressor::new(&brotli[..], 4096).read_to_end(&mut read).expect("br decodes");
        assert_eq!(read, plain, "br decodes to the page");
        assert!(brotli.len() * 3 < plain.len(), "{} against {}", brotli.len(), plain.len());

        let (head, _) = split(&asked(address, "/willdan/", "Accept-Encoding: br;q=0.5, gzip\r\n").await);
        assert_eq!(header(&head, "content-encoding"), Some("gzip"), "the request weighed gzip higher: {head}");

        // Not only the page: what the catalogue answers goes the same way.
        let (head, _) = split(&asked(address, "/api/tools", "Accept-Encoding: br\r\n").await);
        assert_eq!(header(&head, "content-encoding"), Some("br"), "{head}");
    });
}

/// The two faces leave the page and are served once each from the host, under a
/// name carrying the binary's version and cached for a year, as the woff2 they
/// already are; a name of another build's is not served (291, 310a, S235).
#[test]
fn the_fonts_are_cached_under_the_binarys_version() {
    in_a_runtime(async {
        let address = a_served_page().await;
        let (_, page) = split(&asked(address, "/willdan/", "").await);
        let page = String::from_utf8_lossy(&page).to_string();
        assert!(!page.contains("data:font/woff2"), "a face still rides in the page");
        for (name, _, _, bytes) in crate::page::FONTS {
            let own = crate::page::font_address(name);
            assert!(own.contains(crate::page::VERSION), "{own}");
            assert!(page.contains(&format!("url({own})")), "the page does not ask the host for {name} at {own}");
            let (head, body) = split(&asked(address, &own, "Accept-Encoding: br, gzip\r\n").await);
            assert!(head.starts_with("http/1.1 200"), "{head}");
            assert_eq!(header(&head, "content-type"), Some("font/woff2"), "{head}");
            let cache = header(&head, "cache-control").unwrap_or_default();
            assert!(cache.contains("max-age=31536000") && cache.contains("immutable"), "{head}");
            assert_eq!(header(&head, "content-encoding"), None, "woff2 is compressed already: {head}");
            assert_eq!(body, bytes, "{name} is served as the binary carries it");

            let another = own.replace(crate::page::VERSION, "0.0.0-another");
            let (head, _) = split(&asked(address, &another, "").await);
            assert!(head.starts_with("http/1.1 404"), "another build's face is served: {head}");
        }
    });
}
