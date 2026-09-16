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
        // The page's stylesheet, at its own address, is what asks for the faces.
        let (_, style) = split(&asked(address, &crate::page::style_address(), "").await);
        let style = String::from_utf8_lossy(&style).to_string();
        assert!(!style.contains("data:font/woff2"), "a face rides in the stylesheet");
        for (name, _, _, bytes) in crate::page::FONTS {
            let own = crate::page::font_address(name);
            assert!(own.contains(crate::page::VERSION), "{own}");
            assert!(style.contains(&format!("url({own})")), "the page does not ask the host for {name} at {own}");
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

/// The page's stylesheet and script leave its HTML and are served from the host
/// under names carrying the binary's version, cached for a year and compressed
/// like any response; a name of another build's is not served (291, 310a, S235).
#[test]
fn the_style_and_script_are_cached_under_the_binarys_version() {
    in_a_runtime(async {
        let address = a_served_page().await;
        let (_, page) = split(&asked(address, "/willdan/", "").await);
        let page = String::from_utf8_lossy(&page).to_string();
        assert!(!page.contains("<style") && !page.contains("<script>"), "the stylesheet or the script rides in the page");
        let style = (crate::page::style_address(), "text/css", crate::page::stylesheet());
        let script = (crate::page::script_address(), "text/javascript", crate::page::script());
        assert!(page.contains(&format!("<link rel=\"stylesheet\" href=\"{}\">", style.0)), "the page links no stylesheet");
        assert!(page.contains(&format!("<script src=\"{}\"></script>", script.0)), "the page loads no script");
        for (own, media, body) in [style, script] {
            assert!(own.contains(crate::page::VERSION), "{own}");
            let (head, served) = split(&asked(address, &own, "").await);
            assert!(head.starts_with("http/1.1 200"), "{head}");
            assert!(header(&head, "content-type").is_some_and(|t| t.starts_with(media)), "{head}");
            let cache = header(&head, "cache-control").unwrap_or_default();
            assert!(cache.contains("max-age=31536000") && cache.contains("immutable"), "{head}");
            assert_eq!(served, body.as_bytes(), "{own} is not what the bundle carries");
            let (head, _) = split(&asked(address, &own, "Accept-Encoding: br, gzip\r\n").await);
            assert_eq!(header(&head, "content-encoding"), Some("br"), "{head}");

            let another = own.replace(crate::page::VERSION, "0.0.0-another");
            let (head, _) = split(&asked(address, &another, "").await);
            assert!(head.starts_with("http/1.1 404"), "another build's {media} is served: {head}");
        }
    });
}

/// The page is the state as of the moment it was asked for, and no cache holds
/// it: everything the host renders from the store — the page, a dock page, what
/// the catalogue answers, the redirect a control answers a press with — says
/// `no-store`, so the browser asks again after a press instead of serving the
/// copy it held before it. What is addressed by build keeps its year (291,
/// 310a, S235).
#[test]
fn nothing_rendered_from_the_store_is_held_by_a_cache() {
    in_a_runtime(async {
        let address = a_served_page().await;
        // The page at either of its addresses, a dock page as the drawer
        // fetches one, and what the catalogue answers.
        for path in ["/", "/willdan", "/willdan/", "/willdan/bolt-1?part=dock", "/api/tools"] {
            let (head, _) = split(&asked(address, path, "").await);
            assert_eq!(header(&head, "cache-control"), Some("no-store"), "{path}: {head}");
        }

        // The answer to a press is a redirect to the page the browser already
        // has, so the copy it holds is exactly what it would show instead.
        let (head, _) = split(&posted(address, "/tour/next", "").await);
        assert!(head.starts_with("http/1.1 303"), "{head}");
        assert_eq!(header(&head, "cache-control"), Some("no-store"), "{head}");

        // And the faces, the stylesheet and the script are untouched by it: a
        // warm visit still carries neither.
        let faces = crate::page::FONTS.iter().map(|(name, ..)| crate::page::font_address(name));
        for own in [crate::page::style_address(), crate::page::script_address()].into_iter().chain(faces) {
            let (head, _) = split(&asked(address, &own, "").await);
            let cache = header(&head, "cache-control").unwrap_or_default();
            assert!(cache.contains("max-age=31536000") && cache.contains("immutable"), "{own}: {head}");
            assert!(!cache.contains("no-store"), "{own}: {head}");
        }
    });
}

/// A form body posted the way the page's script posts one, with what the page
/// holds in the query, and the reply as it came.
async fn posted(address: std::net::SocketAddr, path: &str, body: &str) -> Vec<u8> {
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost:4242\r\nConnection: close\r\n\
         Content-Type: application/x-www-form-urlencoded\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(request.as_bytes()).await.unwrap();
    let mut reply = Vec::new();
    socket.read_to_end(&mut reply).await.unwrap();
    reply
}

/// A JSON body posted the way a client posts one, and the reply as it came.
async fn sent_json(address: std::net::SocketAddr, path: &str, body: &str) -> Vec<u8> {
    let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost:4242\r\nConnection: close\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    socket.write_all(request.as_bytes()).await.unwrap();
    let mut reply = Vec::new();
    socket.read_to_end(&mut reply).await.unwrap();
    reply
}

/// While the loop beside the page has its turn at the store, the page is
/// answered from the latest read and a call is kept and answered at once; the
/// next turn makes each call once, and a kept call made a second time is the
/// same delivery and records nothing twice (310a, 137, D11).
#[test]
fn a_call_sent_while_a_pass_holds_the_store_is_made_once_on_the_next_turn() {
    use flywheel_atoms::{Records, Scope};
    use serde_json::Value;
    in_a_runtime(async {
        let kept = std::env::temp_dir().join(format!("flywheel-kept-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&kept);
        let defs = flywheel_domain::set::load().expect("the embedded definitions");
        let mut served = Served::over(
            FakeStore::default(),
            Box::new(FakeWorld::new()),
            defs,
            &["chuck".to_string()],
            "http://studio.tailnet.ts.net/willdan",
        );
        served.kept = Some(kept.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(crate::http::serve_on(served.clone(), listener));
        let (_, before) = split(&asked(address, "/willdan/", "").await);

        let (turn, taken) = served.take_turn().await;
        assert_eq!(taken.expect("the kept calls"), 0);
        let began = std::time::Instant::now();
        let (head, during) = split(&asked(address, "/willdan/", "").await);
        assert!(head.starts_with("http/1.1 200"), "{head}");
        assert!(during == before, "the page is not the latest read while the pass holds the store");
        let (head, body) = split(
            &sent_json(address, "/api/tools/capture", r#"{"args": {"text": "the rows lose their numbers", "source": "page"}}"#).await,
        );
        assert!(head.starts_with("http/1.1 202"), "a client's call is not kept: {head}");
        let said: Value = serde_json::from_slice(&body).expect("an answer");
        assert_eq!(said["kept"], serde_json::json!(true), "{said}");
        let (head, body) =
            split(&posted(address, "/api/tools/capture?part=update&have=", "text=the+second+page+drops+them&source=page").await);
        assert!(head.starts_with("http/1.1 200"), "{head}");
        let moved: Value = serde_json::from_slice(&body).expect("an update");
        assert_eq!(moved["kept"], serde_json::json!(crate::http::KEPT), "the page's control is not told it was sent: {moved}");
        assert!(began.elapsed() < std::time::Duration::from_millis(1500), "the page waited {:?} on the pass", began.elapsed());
        let mut held: Vec<std::path::PathBuf> = std::fs::read_dir(&kept).unwrap().map(|entry| entry.unwrap().path()).collect();
        held.sort();
        assert_eq!(held.len(), 2, "{held:?}");
        let first = (held[0].clone(), std::fs::read(&held[0]).unwrap());
        drop(turn);

        let (turn, taken) = served.take_turn().await;
        assert_eq!(taken.expect("the kept calls"), 2);
        drop(turn);
        let captures = served.store.lock().await.list_records(&Scope::Machine("capture".into())).unwrap().len();
        assert_eq!(captures, 2, "each kept call is one capture");

        // A turn that made a call and stopped before letting it go makes it
        // again, as the same delivery.
        std::fs::write(&first.0, &first.1).unwrap();
        let (turn, taken) = served.take_turn().await;
        assert_eq!(taken.expect("the kept call"), 1);
        drop(turn);
        let captures = served.store.lock().await.list_records(&Scope::Machine("capture".into())).unwrap().len();
        assert_eq!(captures, 2, "a kept call made twice captured twice");

        let (_, tray) = split(&asked(address, "/willdan/tray?part=dock", "").await);
        let tray = String::from_utf8_lossy(&tray).to_string();
        assert!(tray.contains("the rows lose their numbers"), "the page does not show what was kept once it is made");
        let _ = std::fs::remove_dir_all(&kept);
    });
}

/// What the page holds, as it asks for an update: every region's digest.
fn holding(digests: &std::collections::BTreeMap<String, String>) -> String {
    let pairs: Vec<String> = digests.iter().map(|(id, digest)| format!("{id}:{digest}")).collect();
    pairs.join(",").replace(':', "%3A").replace(',', "%2C")
}

/// An update carries only the regions that moved since what the page holds, and
/// the open drawer's page only when it moved; a form's post answers with the
/// same, and nothing already on the page is sent again (S221, S235, 310a).
#[test]
fn an_update_carries_only_the_regions_changed_since_its_generation() {
    use flywheel_atoms::Records;
    use serde_json::Value;
    const BOLT: &str = "bolt/atlas/plan-rows";
    in_a_runtime(async {
        let defs = flywheel_domain::set::load().expect("the embedded definitions");
        let mut store = FakeStore::default();
        let at = flywheel_domain::commands::now(&store).expect("a point");
        let record = [("repository".to_string(), serde_json::json!("atlas"))].into_iter().collect();
        flywheel_domain::commands::put_new(&mut store, &defs, BOLT, "bolt", None, record, at).expect("the bolt");
        let mut bolt = Records::get(&store, BOLT).expect("a read").expect("the bolt");
        bolt.config.insert("life".into(), "open".into());
        bolt.config.insert("life.open.close".into(), "offered".into());
        let base = bolt.seq;
        Records::put(&mut store, BOLT, &bolt, base).expect("its close offered");
        let number = flywheel_domain::commands::rail(&mut store, &defs)
            .expect("the rail")
            .into_iter()
            .find_map(|d| d.number)
            .expect("a numbered decision");
        let served = Served::over(store, Box::new(FakeWorld::new()), defs, &["chuck".to_string()], "http://studio.tailnet.ts.net/willdan");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(crate::http::serve_on(served, listener));

        // The first view names the digest of every region it draws.
        let (_, page) = split(&asked(address, "/willdan/", "").await);
        let page = String::from_utf8_lossy(&page).to_string();
        let named = page
            .split("<meta name=\"flywheel-digests\" content=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the page names its regions' digests");
        let mut held: std::collections::BTreeMap<String, String> = named
            .split(',')
            .filter_map(|pair| pair.rsplit_once(':'))
            .map(|(id, digest)| (id.to_string(), digest.to_string()))
            .collect();
        for id in ["rail-cards", "rail-since", "lane-construction-in-progress", "count", "loglist"] {
            assert!(held.contains_key(id), "the page holds no digest for `{id}`: {named}");
            assert!(page.contains(&format!("id=\"{id}\"")), "`{id}` is no region of the page");
        }
        assert!(held.contains_key("yes-all"), "the page holds no digest for yes all: {named}");
        let (head, drawer) = split(&asked(address, &format!("/willdan/{BOLT}?part=dock"), "").await);
        let drawer_digest = header(&head, "x-flywheel-digest").expect("the dock page's digest").to_string();
        assert_eq!(drawer_digest, crate::page::digest(&String::from_utf8_lossy(&drawer)));
        let open = format!("{}%20{drawer_digest}", BOLT.replace('/', "%2F"));

        // Nothing moved: nothing comes.
        let (_, body) = split(&asked(address, &format!("/willdan/?part=update&have={}&open={open}", holding(&held)), "").await);
        let nothing: Value = serde_json::from_slice(&body).expect("an update");
        assert_eq!(nothing["regions"], serde_json::json!({}), "{nothing}");
        assert!(nothing.get("dock").is_none(), "the drawer's page did not move: {nothing}");

        // An answer moves the rail's card, what was sent and the log, and the
        // drawer's page, whose answers are under its title; nothing on the board.
        let (head, body) = split(
            &posted(address, &format!("/api/tools/answer?part=update&have={}&open={open}", holding(&held)), &format!("decision={number}&answer=yes")).await,
        );
        assert!(head.starts_with("http/1.1 200"), "a post from the page's script is answered with an update: {head}");
        let moved: Value = serde_json::from_slice(&body).expect("an update");
        let regions: Vec<String> = moved["regions"].as_object().expect("regions").keys().cloned().collect();
        for id in ["rail-cards", "sent", "loglist"] {
            assert!(regions.contains(&id.to_string()), "`{id}` moved and was not sent: {regions:?}");
        }
        for unmoved in ["hosts", "rail-since", "bd-board"] {
            assert!(!regions.contains(&unmoved.to_string()), "`{unmoved}` did not move and was sent: {regions:?}");
        }
        assert!(!regions.iter().any(|id| id.starts_with("lane-")), "the board did not move and was sent: {regions:?}");
        assert_eq!(moved["dock"]["object"], serde_json::json!(BOLT), "the open drawer's page moved and was not sent: {moved}");
        assert!(moved["regions"]["rail-cards"]["html"].as_str().is_some_and(|html| html.contains("chuck")), "{moved}");
        assert!(body.len() < 8_000, "one answer's update is {} bytes", body.len());

        // Holding what it was sent, the page is current.
        for (id, region) in moved["regions"].as_object().expect("regions") {
            held.insert(id.clone(), region["digest"].as_str().expect("a digest").to_string());
        }
        let open = format!("{}%20{}", BOLT.replace('/', "%2F"), moved["dock"]["digest"].as_str().expect("a digest"));
        let (_, body) = split(&asked(address, &format!("/willdan/?part=update&have={}&open={open}", holding(&held)), "").await);
        let current: Value = serde_json::from_slice(&body).expect("an update");
        assert_eq!(current["regions"], serde_json::json!({}), "{current}");
        assert!(current.get("dock").is_none(), "{current}");

        // A post with no script behind it still lands on the page.
        let (head, _) = split(&posted(address, "/api/tools/answer", &format!("decision={number}&answer=yes")).await);
        assert!(head.starts_with("http/1.1 303"), "{head}");
    });
}
