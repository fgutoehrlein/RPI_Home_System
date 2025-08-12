use crate::{auth, config::Config, embed::ui_router, files};
use anyhow::Result;
use axum::{
    body::StreamBody,
    extract::{Multipart, Path, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use futures::{StreamExt, SinkExt};
use parking_lot::Mutex;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use serde::Serialize;
use std::{collections::HashMap, net::SocketAddr, path::PathBuf};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_util::io::ReaderStream;
use axum::extract::ws::{WebSocket, WebSocketUpgrade, Message};

#[derive(Clone)]
pub struct FileMeta {
    pub mime: String,
    pub name: String,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: Pool<SqliteConnectionManager>,
    pub file_dir: PathBuf,
    pub jwt_secret: Vec<u8>,
    pub files: std::sync::Arc<Mutex<HashMap<String, FileMeta>>>,
    pub event_tx: broadcast::Sender<String>,
    pub config: Config,
}

impl AppState {
    pub async fn new(config: Config) -> Result<Self> {
        let file_dir = config.data_dir.join("files");
        tokio::fs::create_dir_all(&file_dir).await?;
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::new(manager)?;
        let (tx, _rx) = broadcast::channel(100);
        // In real deployments this should be a persistent secret.
        let jwt_secret = b"insecure-development-secret".to_vec();
        Ok(Self {
            pool,
            file_dir,
            jwt_secret,
            files: std::sync::Arc::new(Mutex::new(HashMap::new())),
            event_tx: tx,
            config,
        })
    }
}

/// Build the HTTP application router.
pub fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/api/files", post(upload_file))
        .route("/api/files/:id", get(download_file))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .layer(axum::extract::DefaultBodyLimit::max(
            state.config.max_upload_bytes() as usize,
        ));
    let ws_route = Router::new()
        .route("/ws", get(ws_handler))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware));
    let ui: Router<AppState> = ui_router().with_state(());
    Router::new()
        .route("/api/health", get(health))
        .merge(protected)
        .merge(ws_route)
        .merge(ui)
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn auth_middleware<B>(
    State(state): State<AppState>,
    req: axum::http::Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    if let Some(value) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(value) = value.to_str() {
            if let Some(token) = value.strip_prefix("Bearer ") {
                if auth::verify_jwt(&state.jwt_secret, token).is_ok() {
                    return Ok(next.run(req).await);
                }
            }
        }
    }
    Err(StatusCode::UNAUTHORIZED)
}

#[derive(Serialize)]
struct UploadResp {
    file_id: String,
}

async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, StatusCode> {
    let mut id = None;
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "file".into());
        let mime = field
            .content_type()
            .map(|m| m.to_string())
            .or_else(|| mime_guess::from_path(&name).first().map(|m| m.to_string()))
            .unwrap_or_else(|| "application/octet-stream".into());
        let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;
        let file_id = files::save_file(&state.file_dir, data)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        state
            .files
            .lock()
            .insert(file_id.clone(), FileMeta { mime, name });
        id = Some(file_id);
        break;
    }
    if let Some(file_id) = id {
        Ok((StatusCode::OK, axum::Json(UploadResp { file_id })))
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

async fn download_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let meta = state
        .files
        .lock()
        .get(&id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;
    let path = files::file_path(&state.file_dir, &id);
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let stream = ReaderStream::new(file);
    let body = StreamBody::new(stream);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_str(&meta.mime).unwrap(),
    );
    Ok((headers, body))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state)))
}

async fn handle_socket(stream: WebSocket, state: AppState) {
    let (mut sender, mut receiver) = stream.split();
    let mut rx = BroadcastStream::new(state.event_tx.subscribe());
    let _ = sender.send(Message::Text("hello".into())).await;
    loop {
        tokio::select! {
            _ = rx.next() => {},
            Some(Ok(msg)) = receiver.next() => {
                if matches!(msg, Message::Close(_)) { break; }
            },
            else => break,
        }
    }
}

/// Run the HTTP server bound to the provided address.
pub async fn run_http_server(bind: String) -> Result<()> {
    let mut config = Config::from_env();
    config.bind = bind.clone();
    let state = AppState::new(config).await?;
    let addr: SocketAddr = bind.parse()?;
    axum::Server::bind(&addr)
        .serve(build_router(state).into_make_service())
        .await?;
    Ok(())
}

// Integration tests live in tests/ directory
