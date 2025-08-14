use crate::{api::AppState, files};
use std::collections::HashSet;
use tokio::time::{interval, Duration};

/// Periodically remove orphaned files from the content store.
#[allow(dead_code)]
pub async fn run_housekeeping(state: AppState) {
    let files = state.files.clone();
    let dir = state.file_dir.clone();
    tokio::spawn(async move {
        let mut tick = interval(Duration::from_secs(300));
        loop {
            tick.tick().await;
            let keep: HashSet<String> = files.lock().keys().cloned().collect();
            let _ = files::cleanup_orphans(&dir, &keep).await;
        }
    });
}
