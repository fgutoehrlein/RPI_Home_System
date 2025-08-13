use crate::{auth, config::Config, embed::ui_router, files};
use anyhow::Result;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::{
    body::StreamBody,
    extract::{Extension, Multipart, Path, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures::{SinkExt, StreamExt};
use parking_lot::Mutex;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr, path::PathBuf};
use time::{Duration, OffsetDateTime};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_util::io::ReaderStream;

#[derive(Clone)]
pub struct FileMeta {
    pub mime: String,
    pub name: String,
}

#[derive(Clone)]
pub struct AppState {
    #[allow(dead_code)]
    pub pool: Pool<SqliteConnectionManager>,
    pub file_dir: PathBuf,
    pub files: std::sync::Arc<Mutex<HashMap<String, FileMeta>>>,
    pub event_tx: broadcast::Sender<String>,
    pub config: Config,
    pub auth: std::sync::Arc<tokio::sync::Mutex<Option<auth::AuthConfig>>>,
    pub auth_file: PathBuf,
    pub login_limiter: auth::LoginRateLimiter,
}

impl AppState {
    pub async fn new(config: Config) -> Result<Self> {
        let file_dir = config.data_dir.join("files");
        tokio::fs::create_dir_all(&file_dir).await?;
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::new(manager)?;
        let (tx, _rx) = broadcast::channel(100);
        let auth_file = config.data_dir.join("auth.json");
        let auth = if let Ok(bytes) = tokio::fs::read(&auth_file).await {
            serde_json::from_slice(&bytes).ok()
        } else {
            None
        };
        Ok(Self {
            pool,
            file_dir,
            files: std::sync::Arc::new(Mutex::new(HashMap::new())),
            event_tx: tx,
            config,
            auth: std::sync::Arc::new(tokio::sync::Mutex::new(auth)),
            auth_file,
            login_limiter: auth::LoginRateLimiter::new(5, std::time::Duration::from_secs(60)),
        })
    }
}

/// Build the HTTP application router.
pub fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/api/files", post(upload_file))
        .route("/api/files/:id", get(download_file))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .layer(axum::extract::DefaultBodyLimit::max(
            state.config.max_upload_bytes() as usize,
        ));
    let auth_only = Router::new()
        .route("/api/me", get(me))
        .route("/api/token/refresh", post(refresh_token))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));
    let ws_route =
        Router::new()
            .route("/ws", get(ws_handler))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ));
    let ui: Router<AppState> = ui_router().with_state(());
    Router::new()
        .route("/api/health", get(health))
        .route("/api/bootstrap", post(bootstrap))
        .route("/api/login", post(login))
        .merge(protected)
        .merge(ws_route)
        .merge(auth_only)
        .merge(ui)
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn auth_middleware<B>(
    State(state): State<AppState>,
    mut req: axum::http::Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    if let Some(value) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(value) = value.to_str() {
            if let Some(token) = value.strip_prefix("Bearer ") {
                let secret = {
                    let guard = state.auth.lock().await;
                    guard.as_ref().map(|c| c.jwt_secret.clone())
                };
                if let Some(secret) = secret {
                    if let Ok(claims) =
                        auth::verify_jwt(&STANDARD.decode(&secret).unwrap_or_default(), token)
                    {
                        req.extensions_mut().insert(claims);
                        return Ok(next.run(req).await);
                    }
                }
            }
        }
    }
    Err(StatusCode::UNAUTHORIZED)
}

#[derive(Serialize)]
struct ErrorResp {
    error: String,
}

fn err(status: StatusCode, msg: &str) -> (StatusCode, Json<ErrorResp>) {
    (status, Json(ErrorResp { error: msg.into() }))
}

#[derive(Deserialize)]
struct BootstrapReq {
    passphrase: String,
    users: Vec<auth::User>,
}

async fn bootstrap(
    State(state): State<AppState>,
    Json(req): Json<BootstrapReq>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResp>)> {
    if req.passphrase.len() < 8 {
        return Err(err(StatusCode::BAD_REQUEST, "weak_passphrase"));
    }
    if req.users.iter().filter(|u| u.admin).count() == 0
        || req.users.iter().filter(|u| !u.admin).count() == 0
    {
        return Err(err(StatusCode::BAD_REQUEST, "need_admin_and_user"));
    }
    let mut guard = state.auth.lock().await;
    if guard.is_some() {
        return Err(err(StatusCode::CONFLICT, "already_bootstrapped"));
    }
    let hash = auth::hash_passphrase(&req.passphrase)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "hash"))?;
    use rand::RngCore;
    let mut secret = vec![0u8; 32];
    rand::thread_rng().fill_bytes(&mut secret);
    let cfg = auth::AuthConfig {
        passphrase_hash: hash,
        jwt_secret: STANDARD.encode(&secret),
        users: req.users,
        created_at: OffsetDateTime::now_utc().unix_timestamp(),
    };
    let bytes = serde_json::to_vec(&cfg)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "serialize"))?;
    if let Some(dir) = state.auth_file.parent() {
        if tokio::fs::create_dir_all(dir).await.is_err() {
            return Err(err(StatusCode::INTERNAL_SERVER_ERROR, "persist"));
        }
    }
    tokio::fs::write(&state.auth_file, bytes)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "persist"))?;
    *guard = Some(cfg);
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
struct LoginReq {
    username: String,
    passphrase: String,
}

#[derive(Serialize)]
struct LoginResp {
    token: String,
    user: auth::User,
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginReq>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResp>)> {
    if !state.login_limiter.check(&req.username).await {
        return Err(err(StatusCode::TOO_MANY_REQUESTS, "rate_limited"));
    }
    let guard = state.auth.lock().await;
    let cfg = guard
        .as_ref()
        .ok_or(err(StatusCode::UNAUTHORIZED, "not_bootstrapped"))?;
    if !auth::verify_passphrase(&req.passphrase, &cfg.passphrase_hash) {
        return Err(err(StatusCode::UNAUTHORIZED, "invalid_credentials"));
    }
    let user = cfg
        .users
        .iter()
        .find(|u| u.username == req.username)
        .cloned()
        .ok_or(err(StatusCode::UNAUTHORIZED, "invalid_credentials"))?;
    let secret = STANDARD.decode(&cfg.jwt_secret).unwrap_or_default();
    let token = auth::issue_jwt(&secret, &user.username, Duration::hours(24))
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "token"))?;
    Ok((StatusCode::OK, Json(LoginResp { token, user })))
}

async fn me(
    State(state): State<AppState>,
    Extension(claims): Extension<auth::Claims>,
) -> Result<impl IntoResponse, StatusCode> {
    let guard = state.auth.lock().await;
    if let Some(cfg) = guard.as_ref() {
        if let Some(user) = cfg.users.iter().find(|u| u.username == claims.sub) {
            return Ok(Json(user.clone()));
        }
    }
    Err(StatusCode::UNAUTHORIZED)
}

async fn refresh_token(
    State(state): State<AppState>,
    Extension(claims): Extension<auth::Claims>,
) -> Result<impl IntoResponse, StatusCode> {
    let guard = state.auth.lock().await;
    if let Some(cfg) = guard.as_ref() {
        if let Some(user) = cfg.users.iter().find(|u| u.username == claims.sub) {
            let secret = STANDARD.decode(&cfg.jwt_secret).unwrap_or_default();
            let token = auth::issue_jwt(&secret, &user.username, Duration::hours(24))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            return Ok(Json(LoginResp {
                token,
                user: user.clone(),
            }));
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
    if let Some(field) = multipart.next_field().await.unwrap_or(None) {
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
    headers.insert(
        header::CONTENT_DISPOSITION,
        header::HeaderValue::from_str(&format!("attachment; filename=\"{}\"", meta.name)).unwrap(),
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
