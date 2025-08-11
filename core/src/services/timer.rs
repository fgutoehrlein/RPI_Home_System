use serde::Deserialize;
use tokio::time::{self, Duration};
use plugin_api::Envelope;
use crate::ipc::write_envelope;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct TimerParams {
    pub id: String,
    pub millis: u64,
}

/// Spawn a repeating timer that sends `timer.tick` events using the provided writer.
pub fn spawn_timer(writer: Arc<Mutex<tokio::io::BufWriter<tokio::process::ChildStdin>>>, params: TimerParams) {
    let TimerParams { id, millis } = params;
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_millis(millis));
        loop {
            interval.tick().await;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis();
            let env = Envelope {
                id: None,
                kind: plugin_api::Kind::Event,
                method: None,
                params: None,
                result: None,
                error: None,
                topic: Some("timer.tick".into()),
                payload: Some(json!({"id":id,"now_ms":now})),
            };
            let mut w = writer.lock().await;
            let _ = write_envelope(&mut *w, &env).await;
        }
    });
}
