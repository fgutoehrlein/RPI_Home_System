use axum::{
    extract::Path,
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use mime_guess::mime;
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "webui/dist"]
struct Assets;

pub fn ui_router() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/assets/*path", get(asset))
        .route("/favicon.svg", get(favicon))
        .route("/*path", get(spa_fallback))
}

async fn index() -> impl IntoResponse {
    match Assets::get("index.html") {
        Some(content) => {
            let body = String::from_utf8_lossy(content.data.as_ref()).into_owned();
            let mut res = Html(body).into_response();
            res.headers_mut().insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("no-cache"),
            );
            res
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn asset(Path(path): Path<String>) -> impl IntoResponse {
    serve_file(&format!("assets/{}", path), true)
}

async fn favicon() -> impl IntoResponse {
    serve_file("favicon.svg", true)
}

async fn spa_fallback() -> impl IntoResponse {
    index().await
}

fn serve_file(path: &str, cache: bool) -> Response {
    if let Some(content) = Assets::get(path) {
        let body: Cow<[u8]> = content.data;
        let mut mime = mime_guess::from_path(path).first_or_octet_stream();
        if mime == mime::TEXT_JAVASCRIPT {
            mime = mime::APPLICATION_JAVASCRIPT;
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_str(mime.as_ref()).unwrap(),
        );
        let cache_val = if cache {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        headers.insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static(cache_val),
        );
        (headers, body.into_owned()).into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}
