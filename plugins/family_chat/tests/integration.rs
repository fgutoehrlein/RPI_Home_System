use axum::http::{header, StatusCode};
use base64::Engine;
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

    // bootstrap once
    let body = serde_json::json!({
        "passphrase": "supersecret",
        "users": [
            {"username": "admin", "display_name": "Admin", "admin": true},
            {"username": "user", "display_name": "User", "admin": false}
        ]
    });
    let resp = client
        .post(format!("http://{}/api/bootstrap", addr))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    // second should conflict
    let resp = client
        .post(format!("http://{}/api/bootstrap", addr))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // login success
    let resp = client
        .post(format!("http://{}/api/login", addr))
        .json(&serde_json::json!({"username":"admin","passphrase":"supersecret"}))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let v: serde_json::Value = resp.json().await.unwrap();
    let token = v["token"].as_str().unwrap().to_string();

    // login failure
    let resp = client
        .post(format!("http://{}/api/login", addr))
        .json(&serde_json::json!({"username":"admin","passphrase":"wrong"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let _ = resp.text().await;

    // /api/me
    let resp = client
        .get(format!("http://{}/api/me", addr))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let resp = client
        .get(format!("http://{}/api/me", addr))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let _ = resp.text().await;
    let resp = client
        .get(format!("http://{}/api/me", addr))
        .bearer_auth("bad")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let _ = resp.text().await;

    // token refresh
    let secret = base64::engine::general_purpose::STANDARD
        .decode(&state.auth.lock().await.as_ref().unwrap().jwt_secret)
        .unwrap();
    let short = auth::issue_jwt(&secret, "admin", time::Duration::seconds(1)).unwrap();
    let resp = client
        .post(format!("http://{}/api/token/refresh", addr))
        .bearer_auth(&short)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let refreshed: serde_json::Value = resp.json().await.unwrap();
    let new_token = refreshed["token"].as_str().unwrap();
    let old_claims = auth::verify_jwt(&secret, &short).unwrap();
    let new_claims = auth::verify_jwt(&secret, new_token).unwrap();
    assert!(new_claims.exp > old_claims.exp);

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
    let _ = resp.text().await;

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

#[tokio::test]
async fn admin_user_management() {
    let (addr, server, _state, _tmp) = spawn_server().await;
    let client = reqwest::Client::new();

    // bootstrap
    let body = serde_json::json!({
        "passphrase": "supersecret",
        "users": [
            {"username": "admin", "display_name": "Admin", "admin": true},
            {"username": "user", "display_name": "User", "admin": false}
        ]
    });
    let resp = client
        .post(format!("http://{}/api/bootstrap", addr))
        .json(&body)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    // login tokens
    let admin_token = client
        .post(format!("http://{}/api/login", addr))
        .json(&serde_json::json!({"username":"admin","passphrase":"supersecret"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();
    let user_token = client
        .post(format!("http://{}/api/login", addr))
        .json(&serde_json::json!({"username":"user","passphrase":"supersecret"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();

    // admin list users
    let resp = client
        .get(format!("http://{}/api/admin/users", addr))
        .bearer_auth(&admin_token)
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let users: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(users.as_array().unwrap().len(), 2);

    // normal user forbidden
    let resp = client
        .get(format!("http://{}/api/admin/users", addr))
        .bearer_auth(&user_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // create new user
    let resp = client
        .post(format!("http://{}/api/admin/users", addr))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({"username":"new","display_name":"New"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let created: serde_json::Value = resp.json().await.unwrap();
    let new_id = created["id"].as_u64().unwrap();

    // duplicate username
    let resp = client
        .post(format!("http://{}/api/admin/users", addr))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({"username":"NEW","display_name":"Dup"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CONFLICT);

    // login as new user
    let new_token = client
        .post(format!("http://{}/api/login", addr))
        .json(&serde_json::json!({"username":"new","passphrase":"supersecret"}))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap()["token"]
        .as_str()
        .unwrap()
        .to_string();

    // disable user
    let resp = client
        .patch(format!("http://{}/api/admin/users/{}", addr, new_id))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({"disabled": true}))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());

    // disabled user cannot access
    let resp = client
        .get(format!("http://{}/api/me", addr))
        .bearer_auth(&new_token)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    // disabled user cannot login
    let resp = client
        .post(format!("http://{}/api/login", addr))
        .json(&serde_json::json!({"username":"new","passphrase":"supersecret"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    server.abort();
}
