use axum::http::{header, StatusCode};
use family_chat::{
    api::{build_router, AppState},
    auth,
    config::Config,
};
use futures::StreamExt;
use hyper::Client;
use std::net::{SocketAddr, TcpListener};
use tokio::task::JoinHandle;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

async fn spawn_server() -> (SocketAddr, JoinHandle<()>, AppState, tempfile::TempDir) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let config = Config {
        bind: addr.to_string(),
        data_dir: tmp.path().to_path_buf(),
        max_upload_mb: 5,
    };
    let state = AppState::new(config).await.unwrap();
    let app = build_router(state.clone());
    let server = tokio::spawn(async move {
        axum::Server::from_tcp(listener)
            .unwrap()
            .serve(app.into_make_service())
            .await
            .unwrap();
    });
    (addr, server, state, tmp)
}

#[tokio::test]
async fn serves_ui_and_spa_fallback() {
    let (addr, server, _state, _tmp) = spawn_server().await;
    let client = Client::new();

    // index
    let uri = format!("http://{}/", addr).parse().unwrap();
    let resp = client.get(uri).await.unwrap();
    assert!(resp.status().is_success());
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/html; charset=utf-8"
    );

    // asset
    let uri = format!("http://{}/assets/app-12345678.js", addr)
        .parse()
        .unwrap();
    let resp = client.get(uri).await.unwrap();
    assert!(resp.status().is_success());
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/javascript"
    );

    // spa fallback
    let uri = format!("http://{}/deep/link", addr).parse().unwrap();
    let resp = client.get(uri).await.unwrap();
    assert!(resp.status().is_success());
    assert_eq!(
        resp.headers().get(header::CONTENT_TYPE).unwrap(),
        "text/html; charset=utf-8"
    );

    server.abort();
}

#[tokio::test]
async fn upload_download_and_auth() {
    let (addr, server, state, _tmp) = spawn_server().await;
    let client = reqwest::Client::new();
    let token = auth::issue_jwt(&state.jwt_secret, "user", time::Duration::hours(1)).unwrap();

    // upload
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes("hello world".as_bytes().to_vec()).file_name("hello.txt"),
    );
    let resp = client
        .post(format!("http://{}/api/files", addr))
        .bearer_auth(&token)
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let v: serde_json::Value = resp.json().await.unwrap();
    let id = v["file_id"].as_str().unwrap().to_string();

    // download
    let resp = client
        .get(format!("http://{}/api/files/{}", addr, id))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    assert_eq!(resp.headers()["content-type"], "text/plain");
    let body = resp.text().await.unwrap();
    assert_eq!(body, "hello world");

    // unauthorized
    let resp = client
        .post(format!("http://{}/api/files", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // websocket invalid token
    let url = format!("ws://{}/ws", addr);
    let mut req = url.clone().into_client_request().unwrap();
    req.headers_mut()
        .append("Authorization", "Bearer bad".parse().unwrap());
    assert!(connect_async(req).await.is_err());

    // websocket valid token
    let mut req = url.into_client_request().unwrap();
    req.headers_mut().append(
        "Authorization",
        format!("Bearer {}", token).parse().unwrap(),
    );
    let (mut ws, _) = connect_async(req).await.unwrap();
    let msg = ws.next().await.unwrap().unwrap();
    assert_eq!(msg.into_text().unwrap(), "hello");

    server.abort();
}
