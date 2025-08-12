use crate::embed::WEB_DIST;
use anyhow::Result;
use axum::{
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::get,
    Router,
};

/// Build the HTTP application router.
pub fn app_router() -> Router {
    Router::new()
        .route("/api/health", get(health))
        .fallback(get(static_handler))
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');
    let file = if path.is_empty() {
        WEB_DIST.get_file("index.html")
    } else {
        WEB_DIST
            .get_file(path)
            .or_else(|| WEB_DIST.get_file("index.html"))
    };
    if let Some(file) = file {
        let mime = mime_guess::from_path(file.path()).first_or_octet_stream();
        (
            [(header::CONTENT_TYPE, mime.as_ref())],
            file.contents().to_vec(),
        )
            .into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
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
    use hyper::{header, Client, Uri};

    #[tokio::test]
    async fn serves_index() {
        // Skip the test if the web UI hasn't been built. This allows running
        // `cargo test` without first generating `webui/dist` assets, which are
        // ignored in version control.
        if WEB_DIST.get_file("index.html").is_none() {
            eprintln!("webui/dist/index.html missing; skipping serves_index test");
            return;
        }

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
        let content_type = resp.headers().get(header::CONTENT_TYPE).unwrap();
        assert_eq!(content_type, "text/html");
        server.abort();
    }
}
