use axum::http::StatusCode;
use family_chat::{
    api::{build_router, AppState},
    config::Config,
};
use futures::{SinkExt, StreamExt};
use std::net::{SocketAddr, TcpListener};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

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
async fn message_flow_and_pagination() {
    let (addr, server, state, _tmp) = spawn_server().await;
    let client = reqwest::Client::new();

    // bootstrap users
    let body = serde_json::json!({
        "passphrase": "supersecret",
        "users": [
            {"username": "alice", "display_name": "Alice", "admin": true},
            {"username": "bob", "display_name": "Bob", "admin": false},
            {"username": "charlie", "display_name": "Charlie", "admin": false}
        ]
    });
    client
        .post(format!("http://{}/api/bootstrap", addr))
        .json(&body)
        .send()
        .await
        .unwrap();

    // login tokens
    let alice_token = {
        let resp = client
            .post(format!("http://{}/api/login", addr))
            .json(&serde_json::json!({"username":"alice","passphrase":"supersecret"}))
            .send()
            .await
            .unwrap();
        if !resp.status().is_success() {
            panic!("login failed {}", resp.text().await.unwrap());
        }
        resp.json::<serde_json::Value>().await.unwrap()["token"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let bob_token = {
        let resp = client
            .post(format!("http://{}/api/login", addr))
            .json(&serde_json::json!({"username":"bob","passphrase":"supersecret"}))
            .send()
            .await
            .unwrap();
        if !resp.status().is_success() {
            panic!("login failed {}", resp.text().await.unwrap());
        }
        resp.json::<serde_json::Value>().await.unwrap()["token"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let charlie_token = {
        let resp = client
            .post(format!("http://{}/api/login", addr))
            .json(&serde_json::json!({"username":"charlie","passphrase":"supersecret"}))
            .send()
            .await
            .unwrap();
        if !resp.status().is_success() {
            panic!("login failed {}", resp.text().await.unwrap());
        }
        resp.json::<serde_json::Value>().await.unwrap()["token"]
            .as_str()
            .unwrap()
            .to_string()
    };

    // create DM between alice(id=1) and bob(id=2)
    let dm: serde_json::Value = client
        .get(format!("http://{}/api/dm/2", addr))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let room_id = dm["id"].as_str().unwrap().to_string();

    // bob websocket join
    let url = format!("ws://{}/ws", addr);
    let mut req = url.into_client_request().unwrap();
    req.headers_mut().append(
        "Authorization",
        format!("Bearer {}", bob_token).parse().unwrap(),
    );
    let (mut ws, _) = connect_async(req).await.unwrap();
    ws.next().await.unwrap().unwrap(); // hello
    ws.send(WsMessage::Text(format!(
        "{{\"action\":\"join\",\"room_id\":\"{}\"}}",
        room_id
    )))
    .await
    .unwrap();
    ws.next().await.unwrap().unwrap(); // joined

    // alice posts messages
    let post_msg = |text: &str| {
        let client = client.clone();
        let token = alice_token.clone();
        let room = room_id.clone();
        let t = text.to_string();
        async move {
            client
                .post(format!("http://{}/api/messages", addr))
                .bearer_auth(&token)
                .json(&serde_json::json!({"room_id":room,"text_md":t}))
                .send()
                .await
                .unwrap()
                .json::<serde_json::Value>()
                .await
                .unwrap()
        }
    };
    let m1 = post_msg("one").await;
    let id1 = m1["id"].as_str().unwrap().to_string();
    // consume events until we get message event
    loop {
        if let Some(msg) = ws.next().await {
            let msg = msg.unwrap();
            if let WsMessage::Text(t) = msg {
                let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                if v["t"] == "message" {
                    break;
                }
            }
        }
    }

    // ensure persisted
    let count: i64 = state
        .pool
        .get()
        .unwrap()
        .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);

    let m2 = post_msg("two").await;
    let id2 = m2["id"].as_str().unwrap().to_string();
    // drain to next message event
    loop {
        if let Some(msg) = ws.next().await {
            let msg = msg.unwrap();
            if let WsMessage::Text(t) = msg {
                let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                if v["t"] == "message" {
                    break;
                }
            }
        }
    }
    let _m3 = post_msg("three").await;
    // drain again for message event
    loop {
        if let Some(msg) = ws.next().await {
            let msg = msg.unwrap();
            if let WsMessage::Text(t) = msg {
                let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                if v["t"] == "message" {
                    break;
                }
            }
        }
    }

    // pagination
    let all: Vec<serde_json::Value> = client
        .get(format!(
            "http://{}/api/messages?room_id={}&limit=50",
            addr, room_id
        ))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(all.len(), 3);
    let page1: Vec<serde_json::Value> = client
        .get(format!(
            "http://{}/api/messages?room_id={}&limit=2",
            addr, room_id
        ))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page1.len(), 2);
    let before = page1.last().unwrap()["id"].as_str().unwrap();
    let page2: Vec<serde_json::Value> = client
        .get(format!(
            "http://{}/api/messages?room_id={}&before={}&limit=2",
            addr, room_id, before
        ))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page2.len(), 1);
    let mut combined = page1.clone();
    combined.extend(page2.clone());
    assert_eq!(combined, all);

    // edit first message
    let edited: serde_json::Value = client
        .patch(format!("http://{}/api/messages/{}", addr, id1))
        .bearer_auth(&alice_token)
        .json(&serde_json::json!({"text_md":"one edited"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(edited["text_md"], "one edited");
    // wait for edit event
    loop {
        if let Some(msg) = ws.next().await {
            let msg = msg.unwrap();
            if let WsMessage::Text(t) = msg {
                let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                if v["t"] == "message_edit" {
                    assert_eq!(v["message"]["id"], id1);
                    break;
                }
            }
        }
    }

    // search for edited text
    let search_res: Vec<serde_json::Value> = client
        .get(format!("http://{}/api/search?q=edited", addr))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(search_res.len(), 1);

    // delete second message
    client
        .delete(format!("http://{}/api/messages/{}", addr, id2))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap();
    // wait for delete event
    loop {
        if let Some(msg) = ws.next().await {
            let msg = msg.unwrap();
            if let WsMessage::Text(t) = msg {
                let v: serde_json::Value = serde_json::from_str(&t).unwrap();
                if v["t"] == "message_delete" {
                    assert_eq!(v["message_id"], id2);
                    break;
                }
            }
        }
    }
    let search_res: Vec<serde_json::Value> = client
        .get(format!("http://{}/api/search?q=two", addr))
        .bearer_auth(&alice_token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(search_res.len(), 0);

    // unauthorized post by charlie
    let resp = client
        .post(format!("http://{}/api/messages", addr))
        .bearer_auth(&charlie_token)
        .json(&serde_json::json!({"room_id":room_id,"text_md":"oops"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    server.abort();
}
