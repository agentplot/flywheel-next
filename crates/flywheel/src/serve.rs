//! The page, served by the host that is running (D10a, D11, 307).
//!
//! One process holds one instance: the loop ticks and the page is rendered from
//! the same state store, so what the page shows is what the last tick wrote and
//! an answer given on the page is a commit in the state repository before the
//! next tick reads it (132, 141, 153, 193). Nothing here holds a copy: the
//! handlers reach through to the host's own store and its own world.
//!
//! The host binds two addresses and no others: its private-network name, which
//! is what every link is written at, and a localhost port for the operator
//! sitting at the machine (46, 155, 245, 205a).

use anyhow::{Context, Result};
use flywheel_atoms::{
    EffectWrite, EvidenceRead, HostRecord, LeaseOp, LeaseOutcome, LeaseRecord, Listing, Notice,
    Object, Presentation, PutOutcome, ReadPoint, Received, Records, Scope, StateStore,
    StatusView, ThreadEntry, World, WriteOutcome,
};
use flywheel_engine::runtime::Response;
use flywheel_surface::http::Served;
use std::sync::{Arc, Mutex};

/// The running host, shared between the loop and the page.
pub type Shared = Arc<Mutex<crate::host::Host>>;

/// The host's store, as a handler reaches it. Every operation takes the lock,
/// does the one thing and gives it back, so a request never holds the host
/// while it waits on anything (D11).
pub struct SharedStore(pub Shared);

/// The host's world, reached the same way: the capture box writes its material
/// into the blueprints under the machinery's prefix (111, 203).
pub struct SharedWorld(pub Shared);

impl SharedStore {
    fn with<T>(&self, act: impl FnOnce(&mut crate::host::HostStore) -> T) -> T {
        let mut host = self.0.lock().expect("the running host is poisoned");
        act(&mut host.store)
    }
}

impl Records for SharedStore {
    fn get(&self, id: &str) -> Result<Option<Object>> {
        self.with(|s| s.get(id))
    }
    fn put(&mut self, id: &str, record: &Object, base_seq: u64) -> Result<PutOutcome> {
        self.with(|s| s.put(id, record, base_seq))
    }
    fn append(&mut self, id: &str, entry: &ThreadEntry) -> Result<()> {
        self.with(|s| s.append(id, entry))
    }
    fn list_records(&self, scope: &Scope) -> Result<Vec<Object>> {
        self.with(|s| s.list_records(scope))
    }
    fn thread(&self, id: &str) -> Result<Vec<ThreadEntry>> {
        self.with(|s| s.thread(id))
    }
    fn responses(&self, id: &str) -> Result<Vec<Response>> {
        self.with(|s| s.responses(id))
    }
    fn leases(&self, id: &str) -> Result<Option<LeaseRecord>> {
        self.with(|s| s.leases(id))
    }
    fn hosts(&self) -> Result<Vec<HostRecord>> {
        self.with(|s| s.hosts())
    }
}

impl StateStore for SharedStore {
    fn read(&self, id: &str) -> Result<EvidenceRead> {
        self.with(|s| s.read(id))
    }
    fn list(&self, scope: &Scope) -> Result<Listing> {
        self.with(|s| s.list(scope))
    }
    fn status(&self) -> Result<StatusView> {
        self.with(|s| s.status())
    }
    fn write_effect(&mut self, write: &EffectWrite) -> Result<WriteOutcome> {
        self.with(|s| s.write_effect(write))
    }
    fn lease(&mut self, op: &LeaseOp) -> Result<LeaseOutcome> {
        self.with(|s| s.lease(op))
    }
    fn notify(&self, since: &ReadPoint) -> Result<Notice> {
        self.with(|s| s.notify(since))
    }
    fn present(&mut self, presentation: &Presentation) -> Result<()> {
        self.with(|s| s.present(presentation))
    }
    fn receive(&mut self, response: &Response) -> Result<Received> {
        self.with(|s| s.receive(response))
    }
}

impl SharedWorld {
    fn with<T>(&self, act: impl FnOnce(&mut dyn World) -> T) -> T {
        let mut host = self.0.lock().expect("the running host is poisoned");
        host.store.with_world(|_, world| act(world))
    }
}

impl World for SharedWorld {
    fn manifest(&self) -> Result<serde_json::Value> {
        self.with(|w| w.manifest())
    }
    fn repositories(&self) -> Result<Vec<flywheel_atoms::RepositoryRef>> {
        self.with(|w| w.repositories())
    }
    fn clone_repositories(&mut self) -> Result<()> {
        self.with(|w| w.clone_repositories())
    }
    fn route(&self, name: &str) -> Result<flywheel_atoms::Endpoint> {
        self.with(|w| w.route(name))
    }
    fn app_token(&self) -> Result<String> {
        self.with(|w| w.app_token())
    }
    fn read_file(&self, repository: &str, path: &str) -> Result<Option<Vec<u8>>> {
        self.with(|w| w.read_file(repository, path))
    }
    fn list_files(&self, repository: &str, under: &str) -> Result<Vec<String>> {
        self.with(|w| w.list_files(repository, under))
    }
    fn write_file(
        &mut self,
        repository: &str,
        path: &str,
        body: &[u8],
        by_response: Option<&str>,
    ) -> Result<bool> {
        self.with(|w| w.write_file(repository, path, body, by_response))
    }
}

/// The page this host serves: its own store, its own world, its own address.
pub fn page_of(host: &Shared, port: u16, operators: &[String]) -> Served<SharedStore> {
    let (defs, address) = {
        let held = host.lock().expect("the running host is poisoned");
        (held.defs.clone(), held.sinks.address.clone())
    };
    let mut served = Served::over(
        SharedStore(host.clone()),
        Box::new(SharedWorld(host.clone())),
        defs,
        operators,
        &address,
    );
    served.localhost_port = port;
    served
}

/// The two addresses a host binds, and no others: its private-network name and
/// the operator's own port at the machine (46, 155, 245).
///
/// A name that resolves to nothing on this computer is reported and not bound;
/// the operator at the machine still has their port, and a link written at the
/// name says where it should have answered (205a, 308).
pub async fn listeners(served: &Served<SharedStore>, port: u16) -> Result<Vec<tokio::net::TcpListener>> {
    let mut out = vec![];
    let loopback = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .with_context(|| format!("binding 127.0.0.1:{port} for the operator at this machine"))?;
    out.push(loopback);
    if let Some(name) = flywheel_surface::http::private_host(&served.address) {
        if !flywheel_surface::links::is_localhost(&name) {
            match tokio::net::TcpListener::bind((name.as_str(), port)).await {
                Ok(listener) => out.push(listener),
                Err(e) => eprintln!(
                    "the host's own address `{name}` is not one this computer answers at \
                     ({e}); the page is on localhost:{port} alone (205a, 245)"
                ),
            }
        }
    }
    Ok(out)
}
