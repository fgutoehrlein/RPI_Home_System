use crate::{
    auth, config::Config, db, embed::ui_router, files, messages, presence, reads, rooms, typing,
};
use anyhow::Result;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::{
    body::StreamBody,
    extract::{Extension, Multipart, Path, Query, State},
    http::{header, HeaderMap, Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, patch, post},
    Json, Router,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures::{SinkExt, StreamExt};
use parking_lot::Mutex;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    path::PathBuf,
};
use time::{Duration, OffsetDateTime};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_util::io::ReaderStream;
use url::Url;
use uuid::Uuid;

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
    pub ws_members: std::sync::Arc<Mutex<HashMap<Uuid, HashSet<u32>>>>,
    pub presence: std::sync::Arc<presence::Presence>,
    pub typing: std::sync::Arc<typing::TypingTracker>,
}

impl AppState {
    pub async fn new(config: Config) -> Result<Self> {
        let file_dir = config.data_dir.join("files");
        tokio::fs::create_dir_all(&file_dir).await?;
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::new(manager)?;
        {
            let conn = pool.get()?;
            conn.execute_batch(db::SCHEMA)?;
        }
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
            ws_members: std::sync::Arc::new(Mutex::new(HashMap::new())),
            presence: std::sync::Arc::new(presence::Presence::new(std::time::Duration::from_secs(
                1,
            ))),
            typing: std::sync::Arc::new(typing::TypingTracker::new(
                std::time::Duration::from_secs(2),
            )),
        })
    }
}

/// Build the HTTP application router.
pub fn build_router(state: AppState) -> Router {
    let protected = Router::new()
        .route("/api/files", post(upload_file))
        .route("/api/files/:id", get(download_file))
        .route("/api/rooms", get(list_rooms).post(create_room))
        .route("/api/dm/:user_id", get(get_dm))
        .route("/api/messages", post(post_message).get(list_messages))
        .route("/api/read_pointer", post(update_read_pointer))
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
    let admin = Router::new()
        .route("/api/admin/users", get(list_users).post(create_user))
        .route("/api/admin/users/:id", patch(update_user))
        .layer(middleware::from_fn(admin_only))
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
        .merge(admin)
        .merge(ui)
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

async fn auth_middleware<B>(
    State(state): State<AppState>,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    if let Some(value) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(value) = value.to_str() {
            if let Some(token) = value.strip_prefix("Bearer ") {
                let (secret, users) = {
                    let guard = state.auth.lock().await;
                    guard
                        .as_ref()
                        .map(|c| (c.jwt_secret.clone(), c.users.clone()))
                        .unwrap_or_default()
                };
                if !secret.is_empty() {
                    if let Ok(claims) =
                        auth::verify_jwt(&STANDARD.decode(&secret).unwrap_or_default(), token)
                    {
                        if let Some(user) = users
                            .into_iter()
                            .find(|u| u.username.eq_ignore_ascii_case(&claims.sub) && !u.disabled)
                        {
                            req.extensions_mut().insert(claims);
                            req.extensions_mut().insert(user);
                            return Ok(next.run(req).await);
                        }
                    }
                }
            }
        }
    }
    Err(StatusCode::UNAUTHORIZED)
}

async fn admin_only<B>(req: Request<B>, next: Next<B>) -> Result<Response, StatusCode> {
    if req
        .extensions()
        .get::<auth::User>()
        .map(|u| u.admin)
        .unwrap_or(false)
    {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

#[derive(Serialize)]
struct ErrorResp {
    error: String,
}

fn err(status: StatusCode, msg: &str) -> (StatusCode, Json<ErrorResp>) {
    (status, Json(ErrorResp { error: msg.into() }))
}

fn sanitize_avatar(url: Option<String>) -> Result<Option<String>, (StatusCode, Json<ErrorResp>)> {
    if let Some(u) = url {
        let parsed = Url::parse(&u).map_err(|_| err(StatusCode::BAD_REQUEST, "invalid_avatar"))?;
        match parsed.scheme() {
            "http" | "https" => Ok(Some(parsed.to_string())),
            _ => Err(err(StatusCode::BAD_REQUEST, "invalid_avatar")),
        }
    } else {
        Ok(None)
    }
}

async fn save_auth(
    state: &AppState,
    cfg: &auth::AuthConfig,
) -> Result<(), (StatusCode, Json<ErrorResp>)> {
    let bytes =
        serde_json::to_vec(cfg).map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "serialize"))?;
    if let Some(dir) = state.auth_file.parent() {
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "persist"))?;
    }
    tokio::fs::write(&state.auth_file, bytes)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "persist"))?;
    Ok(())
}

#[derive(Deserialize)]
struct BootstrapUser {
    username: String,
    display_name: String,
    admin: bool,
    #[serde(default)]
    avatar_url: Option<String>,
}

#[derive(Deserialize)]
struct BootstrapReq {
    passphrase: String,
    users: Vec<BootstrapUser>,
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
    let mut users_vec = Vec::new();
    let mut seen = HashSet::new();
    let mut next_id = 1u32;
    for u in req.users {
        if u.display_name.trim().is_empty() || u.username.trim().is_empty() {
            return Err(err(StatusCode::BAD_REQUEST, "invalid_user"));
        }
        let username = u.username.to_lowercase();
        if !seen.insert(username.clone()) {
            return Err(err(StatusCode::BAD_REQUEST, "duplicate_username"));
        }
        let avatar = sanitize_avatar(u.avatar_url)?;
        users_vec.push(auth::User {
            id: next_id,
            username,
            display_name: u.display_name,
            admin: u.admin,
            disabled: false,
            avatar_url: avatar,
        });
        next_id += 1;
    }
    let cfg = auth::AuthConfig {
        passphrase_hash: hash,
        jwt_secret: STANDARD.encode(&secret),
        users: users_vec,
        created_at: OffsetDateTime::now_utc().unix_timestamp(),
    };
    save_auth(&state, &cfg).await?;
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
        .find(|u| u.username.eq_ignore_ascii_case(&req.username))
        .cloned()
        .ok_or(err(StatusCode::UNAUTHORIZED, "invalid_credentials"))?;
    if user.disabled {
        return Err(err(StatusCode::UNAUTHORIZED, "disabled"));
    }
    let secret = STANDARD.decode(&cfg.jwt_secret).unwrap_or_default();
    let token = auth::issue_jwt(&secret, &user.username, Duration::hours(24))
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "token"))?;
    Ok((StatusCode::OK, Json(LoginResp { token, user })))
}

async fn me(Extension(user): Extension<auth::User>) -> Result<impl IntoResponse, StatusCode> {
    Ok(Json(user))
}

async fn refresh_token(
    State(state): State<AppState>,
    Extension(user): Extension<auth::User>,
) -> Result<impl IntoResponse, StatusCode> {
    let guard = state.auth.lock().await;
    if let Some(cfg) = guard.as_ref() {
        let secret = STANDARD.decode(&cfg.jwt_secret).unwrap_or_default();
        let token = auth::issue_jwt(&secret, &user.username, Duration::hours(24))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        return Ok(Json(LoginResp { token, user }));
    }
    Err(StatusCode::UNAUTHORIZED)
}

#[derive(Serialize)]
struct UserResp {
    id: u32,
    username: String,
    display_name: String,
    disabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    avatar_url: Option<String>,
}

impl From<auth::User> for UserResp {
    fn from(u: auth::User) -> Self {
        Self {
            id: u.id,
            username: u.username,
            display_name: u.display_name,
            disabled: u.disabled,
            avatar_url: u.avatar_url,
        }
    }
}

async fn list_users(State(state): State<AppState>) -> Result<impl IntoResponse, StatusCode> {
    let guard = state.auth.lock().await;
    if let Some(cfg) = guard.as_ref() {
        let users: Vec<UserResp> = cfg.users.clone().into_iter().map(Into::into).collect();
        Ok(Json(users))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[derive(Deserialize)]
struct CreateUserReq {
    username: String,
    display_name: String,
    #[serde(default)]
    avatar_url: Option<String>,
}

async fn create_user(
    State(state): State<AppState>,
    Json(req): Json<CreateUserReq>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResp>)> {
    if req.display_name.trim().is_empty() || req.username.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "invalid_user"));
    }
    let avatar = sanitize_avatar(req.avatar_url)?;
    let mut guard = state.auth.lock().await;
    let cfg = guard
        .as_mut()
        .ok_or(err(StatusCode::UNAUTHORIZED, "not_bootstrapped"))?;
    let username = req.username.to_lowercase();
    let user = auth::User {
        id: cfg.next_id(),
        username,
        display_name: req.display_name,
        admin: false,
        disabled: false,
        avatar_url: avatar,
    };
    cfg.add_user(user.clone())
        .map_err(|_| err(StatusCode::CONFLICT, "username_taken"))?;
    let cfg_clone = cfg.clone();
    drop(guard);
    save_auth(&state, &cfg_clone).await?;
    Ok((StatusCode::CREATED, Json(UserResp::from(user))))
}

#[derive(Deserialize)]
struct UpdateUserReq {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    avatar_url: Option<String>,
    #[serde(default)]
    disabled: Option<bool>,
}

async fn update_user(
    State(state): State<AppState>,
    Path(id): Path<u32>,
    Json(req): Json<UpdateUserReq>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResp>)> {
    if let Some(ref dn) = req.display_name {
        if dn.trim().is_empty() {
            return Err(err(StatusCode::BAD_REQUEST, "invalid_user"));
        }
    }
    let avatar = sanitize_avatar(req.avatar_url)?;
    let mut guard = state.auth.lock().await;
    let cfg = guard
        .as_mut()
        .ok_or(err(StatusCode::UNAUTHORIZED, "not_bootstrapped"))?;
    let user = cfg
        .users
        .iter_mut()
        .find(|u| u.id == id)
        .ok_or(err(StatusCode::NOT_FOUND, "not_found"))?;
    if let Some(dn) = req.display_name {
        user.display_name = dn;
    }
    if let Some(dis) = req.disabled {
        user.disabled = dis;
    }
    if avatar.is_some() {
        user.avatar_url = avatar;
    }
    let updated = user.clone();
    let cfg_clone = cfg.clone();
    drop(guard);
    save_auth(&state, &cfg_clone).await?;
    Ok(Json(UserResp::from(updated)))
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

#[derive(Deserialize)]
struct CreateRoomReq {
    name: String,
    slug: Option<String>,
}

async fn create_room(
    State(state): State<AppState>,
    Extension(_user): Extension<auth::User>,
    Json(req): Json<CreateRoomReq>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResp>)> {
    if req.name.trim().is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "invalid_name"));
    }
    let conn = state
        .pool
        .get()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
    match rooms::create_public_room(&conn, &req.name, req.slug.as_deref()) {
        Ok(room) => Ok((StatusCode::OK, Json(room))),
        Err(e) if e.to_string() == "duplicate_slug" => {
            Err(err(StatusCode::CONFLICT, "duplicate_slug"))
        }
        Err(_) => Err(err(StatusCode::INTERNAL_SERVER_ERROR, "db")),
    }
}

#[derive(Serialize)]
struct RoomWithUnread {
    #[serde(flatten)]
    room: rooms::Room,
    unread_count: u32,
}

async fn list_rooms(
    State(state): State<AppState>,
    Extension(user): Extension<auth::User>,
) -> Result<Json<Vec<RoomWithUnread>>, (StatusCode, Json<ErrorResp>)> {
    let conn = state
        .pool
        .get()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
    let rooms = rooms::list_rooms_for_user(&conn, user.id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
    let items = rooms
        .into_iter()
        .map(|room| {
            let unread = reads::unread_count(&conn, user.id, &room.id).unwrap_or(0);
            RoomWithUnread {
                room,
                unread_count: unread,
            }
        })
        .collect();
    Ok(Json(items))
}

async fn get_dm(
    State(state): State<AppState>,
    Extension(user): Extension<auth::User>,
    Path(other_id): Path<u32>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResp>)> {
    if user.id == other_id {
        return Err(err(StatusCode::BAD_REQUEST, "self_dm"));
    }
    {
        let guard = state.auth.lock().await;
        let cfg = guard
            .as_ref()
            .ok_or(err(StatusCode::UNAUTHORIZED, "not_bootstrapped"))?;
        if !cfg.users.iter().any(|u| u.id == other_id) {
            return Err(err(StatusCode::NOT_FOUND, "user_not_found"));
        }
    }
    let conn = state
        .pool
        .get()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
    let room = rooms::get_or_create_dm_room(&conn, user.id, other_id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
    Ok((StatusCode::OK, Json(room)))
}

#[derive(Deserialize)]
struct ReadPointerReq {
    room_id: Uuid,
    message_id: Option<Uuid>,
    timestamp: Option<i64>,
}

async fn update_read_pointer(
    State(state): State<AppState>,
    Extension(user): Extension<auth::User>,
    Json(req): Json<ReadPointerReq>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResp>)> {
    let conn = state
        .pool
        .get()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
    let allowed = rooms::user_can_access_room(&conn, &req.room_id, user.id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
    if !allowed {
        return Err(err(StatusCode::FORBIDDEN, "forbidden"));
    }
    let ts = if let Some(mid) = req.message_id {
        let mut stmt = conn
            .prepare("SELECT created_at FROM messages WHERE id = ?1")
            .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
        stmt.query_row([mid.to_string()], |row| row.get(0))
            .map_err(|_| err(StatusCode::BAD_REQUEST, "message_not_found"))?
    } else if let Some(ts) = req.timestamp {
        ts
    } else {
        OffsetDateTime::now_utc().unix_timestamp()
    };
    reads::set_read_pointer(&conn, user.id, &req.room_id, ts)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
    let _ = state.event_tx.send(
        serde_json::json!({"t":"unread","room_id":req.room_id,"user_id":user.id,"count":0})
            .to_string(),
    );
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct CreateMessageReq {
    room_id: Uuid,
    text_md: String,
    #[serde(default)]
    message_idempotency_key: Option<String>,
}

async fn post_message(
    State(state): State<AppState>,
    Extension(user): Extension<auth::User>,
    Json(req): Json<CreateMessageReq>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResp>)> {
    let conn = state
        .pool
        .get()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
    let allowed = rooms::user_can_access_room(&conn, &req.room_id, user.id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
    if !allowed {
        return Err(err(StatusCode::FORBIDDEN, "forbidden"));
    }
    let msg = messages::create_message(
        &conn,
        &req.room_id,
        user.id,
        &req.text_md,
        req.message_idempotency_key.as_deref(),
    )
    .map_err(|e| match e.to_string().as_str() {
        "empty_message" => err(StatusCode::BAD_REQUEST, "empty_message"),
        _ => err(StatusCode::INTERNAL_SERVER_ERROR, "db"),
    })?;
    reads::set_read_pointer(&conn, user.id, &req.room_id, msg.created_at)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
    let _ = state
        .event_tx
        .send(serde_json::json!({"t":"message","room_id":req.room_id,"message":msg}).to_string());
    let members: Vec<u32> = state
        .ws_members
        .lock()
        .get(&req.room_id)
        .map(|s| s.iter().copied().collect())
        .unwrap_or_default();
    for uid in members {
        if uid == user.id {
            continue;
        }
        if let Ok(unread) = reads::unread_count(&conn, uid, &req.room_id) {
            let _ = state.event_tx.send(
                serde_json::json!({"t":"unread","room_id":req.room_id,"user_id":uid,"count":unread}).to_string(),
            );
        }
    }
    Ok((StatusCode::CREATED, Json(msg)))
}

#[derive(Deserialize)]
struct ListMessagesParams {
    room_id: Uuid,
    before: Option<String>,
    limit: Option<usize>,
}

async fn list_messages(
    State(state): State<AppState>,
    Extension(user): Extension<auth::User>,
    Query(params): Query<ListMessagesParams>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResp>)> {
    let conn = state
        .pool
        .get()
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
    let allowed = rooms::user_can_access_room(&conn, &params.room_id, user.id)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
    if !allowed {
        return Err(err(StatusCode::FORBIDDEN, "forbidden"));
    }
    let limit = params.limit.unwrap_or(50).min(200);
    let before = match params.before {
        Some(ref b) => {
            if let Ok(ts) = b.parse::<i64>() {
                Some(messages::Cursor::Timestamp(ts))
            } else if let Ok(id) = Uuid::parse_str(b) {
                Some(messages::Cursor::Id(id))
            } else {
                None
            }
        }
        None => None,
    };
    let msgs = messages::list_messages(&conn, &params.room_id, before, limit)
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?;
    Ok(Json(msgs))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Extension(user): Extension<auth::User>,
) -> Result<impl IntoResponse, StatusCode> {
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, state, user)))
}

async fn handle_socket(stream: WebSocket, state: AppState, user: auth::User) {
    let (mut sender, mut receiver) = stream.split();
    let mut rx = BroadcastStream::new(state.event_tx.subscribe());
    if state.presence.connect(user.id) {
        let _ = state.event_tx.send(
            serde_json::json!({"t":"presence","user_id":user.id,"state":"online"}).to_string(),
        );
    }
    let _ = sender.send(Message::Text("hello".into())).await;
    loop {
        tokio::select! {
            Some(Ok(ev)) = rx.next() => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&ev) {
                    if let Some(rid_str) = v.get("room_id").and_then(|r| r.as_str()) {
                        if let Ok(rid) = Uuid::parse_str(rid_str) {
                            let allowed = state
                                .ws_members
                                .lock()
                                .get(&rid)
                                .map(|s| s.contains(&user.id))
                                .unwrap_or(false);
                            if allowed {
                                let _ = sender.send(Message::Text(ev)).await;
                            }
                        }
                    } else {
                        let _ = sender.send(Message::Text(ev)).await;
                    }
                }
            },
            Some(Ok(msg)) = receiver.next() => {
                match msg {
                    Message::Text(t) => {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
                            if v.get("action").and_then(|a| a.as_str()) == Some("join") {
                                if let Some(id_str) = v.get("room_id").and_then(|r| r.as_str()) {
                                    if let Ok(room_id) = Uuid::parse_str(id_str) {
                                        let allowed = {
                                            state
                                                .pool
                                                .get()
                                                .ok()
                                                .and_then(|conn| rooms::user_can_access_room(&conn, &room_id, user.id).ok())
                                                .unwrap_or(false)
                                        };
                                        if allowed {
                                            {
                                                let mut guard = state.ws_members.lock();
                                                guard.entry(room_id).or_default().insert(user.id);
                                            }
                                            let presence_map = state.presence.snapshot().into_iter().map(|(k,v)| (k.to_string(), v)).collect::<std::collections::HashMap<_,_>>();
                                            let unread = state
                                                .pool
                                                .get()
                                                .ok()
                                                .and_then(|conn| reads::unread_count(&conn, user.id, &room_id).ok())
                                                .unwrap_or(0);
                                            let snap = serde_json::json!({"t":"snapshot","room_id":room_id,"presence":presence_map,"unread":unread});
                                            let _ = sender.send(Message::Text(snap.to_string())).await;
                                            continue;
                                        }
                                    }
                                }
                            } else if v.get("t").and_then(|a| a.as_str()) == Some("typing") {
                                if let Some(id_str) = v.get("room_id").and_then(|r| r.as_str()) {
                                    if let Ok(room_id) = Uuid::parse_str(id_str) {
                                        let joined = state
                                            .ws_members
                                            .lock()
                                            .get(&room_id)
                                            .map(|s| s.contains(&user.id))
                                            .unwrap_or(false);
                                        if joined && state.typing.typing(user.id, room_id) {
                                            let _ = state.event_tx.send(
                                                serde_json::json!({"t":"typing","room_id":room_id,"user_id":user.id}).to_string(),
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            },
            else => break,
        }
    }
    {
        let mut guard = state.ws_members.lock();
        for members in guard.values_mut() {
            members.remove(&user.id);
        }
        guard.retain(|_, v| !v.is_empty());
    }
    if state.presence.disconnect(user.id).await {
        let _ = state.event_tx.send(
            serde_json::json!({"t":"presence","user_id":user.id,"state":"offline"}).to_string(),
        );
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
