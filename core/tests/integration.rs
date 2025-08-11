use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

use homecore::{workspace_root, PluginManager};
use serde_json::json;

struct LogWriter(Arc<Mutex<Vec<u8>>>);
impl io::Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
struct MakeLogWriter(Arc<Mutex<Vec<u8>>>);
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for MakeLogWriter {
    type Writer = LogWriter;
    fn make_writer(&'a self) -> Self::Writer {
        LogWriter(self.0.clone())
    }
}

#[tokio::test]
#[ignore]
async fn sample_plugin_runs() {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let make = MakeLogWriter(buf.clone());
    let subscriber = tracing_subscriber::fmt().with_writer(make).finish();
    let workspace = workspace_root().unwrap();
    // ensure plugin binary is built
    std::process::Command::new("cargo")
        .args(["build", "-p", "sample_plugin"])
        .status()
        .expect("build sample_plugin");
    let plugins_dir = workspace.join("plugins");
    tracing::subscriber::with_default(subscriber, || async move {
        let mut manager = PluginManager::discover(workspace.clone(), plugins_dir).unwrap();
        manager.start_all().await.unwrap();
        let resp = manager
            .call("sample_plugin", "sample.ping", json!({"text":"hi"}))
            .await
            .unwrap();
        assert_eq!(resp.get("text").and_then(|v| v.as_str()), Some("hi"));
        tokio::time::sleep(Duration::from_millis(1100)).await;
        for handle in manager.plugins.values_mut() {
            if let Some(child) = handle.child.as_mut() {
                let _ = child.kill().await;
            }
        }
    })
    .await;
    let logs = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
    assert!(logs.contains("tick from sample_plugin"), "logs: {logs}");
}
