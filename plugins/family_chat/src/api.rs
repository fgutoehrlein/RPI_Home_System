use crate::embed::WEB_DIST;
use anyhow::Result;
use axum::{response::Html, routing::get, Router};

/// Build the HTTP application router.
pub fn app_router() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
}

async fn index() -> Html<&'static str> {
    let file = WEB_DIST
        .get_file("index.html")
        .expect("index.html not found in embedded assets");
    Html(file.contents_utf8().unwrap())
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

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::{Client, Uri};

    #[tokio::test]
    async fn serves_index() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = tokio::spawn(async move {
            axum::Server::from_tcp(listener)
                .unwrap()
                .serve(app_router().into_make_service())
                .await
                .unwrap();
        });

        let client = Client::new();
        let uri: Uri = format!("http://{}/", addr).parse().unwrap();
        let resp = client.get(uri).await.unwrap();
        assert!(resp.status().is_success());
        server.abort();
    }
}
