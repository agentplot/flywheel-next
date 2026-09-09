//! The catalogue's one transport in this phase (193, D9).
//!
//! The routes here hold no operation of their own: each one reads or writes
//! through the same catalogue function the in-process caller uses, so a further
//! transport adds a client and not an operation.

use crate::catalogue;
use axum::{routing::get, Json, Router};
use serde_json::Value;

/// `GET /api/tools`: the catalogue as the HTTP caller enumerates it.
async fn tools() -> Json<Value> {
    Json(catalogue::enumerate())
}

/// The router the page and the chat are served by.
pub fn router() -> Router {
    Router::new().route("/api/tools", get(tools))
}

/// Serve the router at an address, until the process ends.
pub async fn serve(address: &str) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(address).await?;
    axum::serve(listener, router()).await?;
    Ok(())
}
