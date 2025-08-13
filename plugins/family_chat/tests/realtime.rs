use family_chat::api::{build_router, AppState};
use family_chat::config::Config;
use futures::{SinkExt, StreamExt};
use tokio::task::JoinHandle;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage, tungstenite::client::IntoClientRequest};
use uuid::Uuid;
use std::net::{SocketAddr, TcpListener};

async fn spawn_server() -> (SocketAddr, JoinHandle<()>, AppState, tempfile::TempDir) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let config = Config { bind: addr.to_string(), data_dir: tmp.path().to_path_buf(), max_upload_mb: 5 };
    let state = AppState::new(config).await.unwrap();
    let app = build_router(state.clone());
    let server = tokio::spawn(async move {
        axum::Server::from_tcp(listener).unwrap().serve(app.into_make_service()).await.unwrap();
    });
    (addr, server, state, tmp)
}

#[tokio::test]
async fn presence_typing_unread_flow() {
    let (addr, server, _state, _tmp) = spawn_server().await;
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "passphrase": "supersecret",
        "users": [
            {"username":"admin","display_name":"Admin","admin":true},
            {"username":"alice","display_name":"Alice","admin":false},
            {"username":"bob","display_name":"Bob","admin":false}
        ]
    });
    client.post(format!("http://{}/api/bootstrap", addr)).json(&body).send().await.unwrap();
    let resp = client.post(format!("http://{}/api/login", addr)).json(&serde_json::json!({"username":"alice","passphrase":"supersecret"})).send().await.unwrap();
    assert!(resp.status().is_success());
    let alice_token = resp.json::<serde_json::Value>().await.unwrap()["token"].as_str().unwrap().to_string();
    let resp = client.post(format!("http://{}/api/login", addr)).json(&serde_json::json!({"username":"bob","passphrase":"supersecret"})).send().await.unwrap();
    assert!(resp.status().is_success());
    let bob_token = resp.json::<serde_json::Value>().await.unwrap()["token"].as_str().unwrap().to_string();
    let resp = client.post(format!("http://{}/api/rooms", addr)).bearer_auth(&alice_token).json(&serde_json::json!({"name":"General","slug":"general"})).send().await.unwrap();
    assert!(resp.status().is_success());
    let room_id = resp.json::<serde_json::Value>().await.unwrap()["id"].as_str().unwrap().parse::<Uuid>().unwrap();

    // Bob connects first
    let mut bob_req = format!("ws://{}/ws", addr).into_client_request().unwrap();
    bob_req.headers_mut().append("Authorization", format!("Bearer {}", bob_token).parse().unwrap());
    let (mut bob_ws, _) = connect_async(bob_req).await.unwrap();
    bob_ws.next().await; // hello
    bob_ws.next().await; // presence for Bob

    // Alice connects
    let mut alice_req = format!("ws://{}/ws", addr).into_client_request().unwrap();
    alice_req.headers_mut().append("Authorization", format!("Bearer {}", alice_token).parse().unwrap());
    let (mut alice_ws, _) = connect_async(alice_req).await.unwrap();
    alice_ws.next().await; // hello

    // Bob should see Alice online
    let ev = bob_ws.next().await.unwrap().unwrap().into_text().unwrap();
    let v: serde_json::Value = serde_json::from_str(&ev).unwrap();
    assert_eq!(v["t"], "presence");
    assert_eq!(v["user_id"], 2);
    assert_eq!(v["state"], "online");

    // Alice disconnects
    alice_ws.close(None).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
    let ev = bob_ws.next().await.unwrap().unwrap().into_text().unwrap();
    let v: serde_json::Value = serde_json::from_str(&ev).unwrap();
    assert_eq!(v["state"], "offline");

    // Typing: reconnect Alice and join room
    let mut alice_req = format!("ws://{}/ws", addr).into_client_request().unwrap();
    alice_req.headers_mut().append("Authorization", format!("Bearer {}", alice_token).parse().unwrap());
    let (mut alice_ws, _) = connect_async(alice_req).await.unwrap();
    alice_ws.next().await; // hello
    alice_ws.send(WsMessage::Text(format!("{{\"action\":\"join\",\"room_id\":\"{}\"}}", room_id))).await.unwrap();
    bob_ws.send(WsMessage::Text(format!("{{\"action\":\"join\",\"room_id\":\"{}\"}}", room_id))).await.unwrap();
    loop {
        if let Some(Ok(WsMessage::Text(txt))) = bob_ws.next().await {
            let val: serde_json::Value = serde_json::from_str(&txt).unwrap();
            if val["t"] == "snapshot" { break; }
        }
    }

    alice_ws.send(WsMessage::Text(format!("{{\"t\":\"typing\",\"room_id\":\"{}\"}}", room_id))).await.unwrap();
    alice_ws.send(WsMessage::Text(format!("{{\"t\":\"typing\",\"room_id\":\"{}\"}}", room_id))).await.unwrap();
    use tokio::time::{timeout, Duration};
    let msg = timeout(Duration::from_millis(500), bob_ws.next()).await.unwrap().unwrap().unwrap();
    if let WsMessage::Text(txt) = msg { let v: serde_json::Value = serde_json::from_str(&txt).unwrap(); assert_eq!(v["t"], "typing"); }
    assert!(timeout(Duration::from_millis(500), bob_ws.next()).await.is_err());

    // Unread test: Bob disconnects
    bob_ws.close(None).await.unwrap();
    for _ in 0..3 {
        client.post(format!("http://{}/api/messages", addr)).bearer_auth(&alice_token).json(&serde_json::json!({"room_id":room_id,"text_md":"hi"})).send().await.unwrap();
    }
    let mut bob_req = format!("ws://{}/ws", addr).into_client_request().unwrap();
    bob_req.headers_mut().append("Authorization", format!("Bearer {}", bob_token).parse().unwrap());
    let (mut bob_ws, _) = connect_async(bob_req).await.unwrap();
    bob_ws.next().await; // hello
    bob_ws.send(WsMessage::Text(format!("{{\"action\":\"join\",\"room_id\":\"{}\"}}", room_id))).await.unwrap();
    let snap = loop {
        if let Some(Ok(WsMessage::Text(txt))) = bob_ws.next().await {
            let val: serde_json::Value = serde_json::from_str(&txt).unwrap();
            if val["t"] == "snapshot" { break val; }
        }
    };
    assert_eq!(snap["unread"], 3);

    client.post(format!("http://{}/api/read_pointer", addr)).bearer_auth(&bob_token).json(&serde_json::json!({"room_id":room_id})).send().await.unwrap();
    let resp = client.get(format!("http://{}/api/rooms", addr)).bearer_auth(&bob_token).send().await.unwrap();
    let rooms: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(rooms[0]["unread_count"], 0);

    client.post(format!("http://{}/api/messages", addr)).bearer_auth(&alice_token).json(&serde_json::json!({"room_id":room_id,"text_md":"again"})).send().await.unwrap();
    let ev = bob_ws.next().await.unwrap().unwrap().into_text().unwrap();
    let v: serde_json::Value = serde_json::from_str(&ev).unwrap();
    assert_eq!(v["t"], "unread");
    server.abort();
}
