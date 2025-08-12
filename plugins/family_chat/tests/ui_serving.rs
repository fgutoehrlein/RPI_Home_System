use family_chat::api::app_router;
use hyper::{header, Client, Uri};

#[tokio::test]
async fn serves_ui_with_correct_content_types() {
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
    // Root index.html
    let uri: Uri = format!("http://{}/", addr).parse().unwrap();
    let resp = client.get(uri).await.unwrap();
    assert!(resp.status().is_success());
    let ct = resp.headers().get(header::CONTENT_TYPE).unwrap();
    assert_eq!(ct, "text/html; charset=utf-8");

    // JS asset
    let uri: Uri = format!("http://{}/assets/app-12345678.js", addr)
        .parse()
        .unwrap();
    let resp = client.get(uri).await.unwrap();
    assert!(resp.status().is_success());
    let ct = resp.headers().get(header::CONTENT_TYPE).unwrap();
    assert_eq!(ct, "application/javascript");

    server.abort();
}
