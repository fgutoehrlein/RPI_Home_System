use family_chat::{
    api::{build_router, AppState},
    config::{Bootstrap, Config},
};
use std::net::{SocketAddr, TcpListener};
use tokio::task::JoinHandle;

async fn spawn(cfg: Config) -> (SocketAddr, JoinHandle<()>) {
    let addr: SocketAddr = cfg.bind.parse().unwrap();
    let state = AppState::new(cfg).await.unwrap();
    let app = build_router(state);
    let server = tokio::spawn(async move {
        axum::Server::bind(&addr)
            .serve(app.into_make_service())
            .await
            .unwrap();
    });
    (addr, server)
}

#[tokio::test]
async fn bootstrap_creates_admin_and_is_idempotent() {
    let tmp = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let cfg = Config {
        bind: format!("127.0.0.1:{}", port),
        data_dir: tmp.path().to_path_buf(),
        max_upload_mb: 5,
        logging_enabled: true,
        bootstrap: Some(Bootstrap {
            username: "admin".into(),
            password: "admin".into(),
        }),
    };
    let (addr, server) = spawn(cfg.clone()).await;
    let client = reqwest::Client::new();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let resp = client
        .post(format!("http://{}/api/login", addr))
        .json(&serde_json::json!({"username":"admin","passphrase":"admin"}))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success());
    let v: serde_json::Value = resp.json().await.unwrap();
    assert!(v["user"]["admin"].as_bool().unwrap());
    assert!(v["user"]["must_change_password"].as_bool().unwrap());
    server.abort();

    // bootstrap should not run again
    let state2 = AppState::new(cfg).await.unwrap();
    let guard = state2.auth.lock().await;
    assert_eq!(guard.as_ref().unwrap().users.len(), 1);
}
