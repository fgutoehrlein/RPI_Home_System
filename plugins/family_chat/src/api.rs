use crate::embed::ui_router;
use anyhow::Result;
use axum::{routing::get, Router};

/// Build the HTTP application router.
pub fn app_router() -> Router {
    Router::new()
        .route("/api/health", get(health))
        .merge(ui_router())
}

async fn health() -> &'static str {
    "ok"
}

/// Run the HTTP server bound to the provided address.
pub async fn run_http_server(bind: String) -> Result<()> {
    let addr: std::net::SocketAddr = bind.parse()?;
    axum::Server::bind(&addr)
        .serve(app_router().into_make_service())
        .await?;
    Ok(())
}

// Integration tests live in tests/ui_serving.rs
