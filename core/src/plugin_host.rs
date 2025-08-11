use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use plugin_api::{Envelope, Kind};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::{
    io::{BufReader, BufWriter},
    process::{Child, Command},
    sync::oneshot,
};
use tracing::error;
use uuid::Uuid;

use crate::ipc::{read_envelope, write_envelope};

/// Manifest information parsed from `plugin.toml`.
#[derive(Debug, Deserialize, Clone)]
pub struct PluginManifest {
    pub name: String,
    pub id: String,
    pub version: String,
    pub api_version: String,
    pub exec: String,
    #[serde(default)]
    pub permissions: Vec<String>,
}

/// Status of a plugin managed by the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginStatus {
    Discovered,
    Running,
}

/// Runtime handle to a plugin process.
pub struct PluginHandle {
    pub manifest: PluginManifest,
    pub dir: PathBuf,
    pub status: PluginStatus,
    pub child: Option<Child>,
    writer: Option<Arc<tokio::sync::Mutex<BufWriter<tokio::process::ChildStdin>>>>,
    pending: ArcPending,
    subscriptions: HashSet<String>,
}

type ArcPending = std::sync::Arc<Mutex<HashMap<String, oneshot::Sender<Envelope>>>>;

impl PluginHandle {
    fn new(manifest: PluginManifest, dir: PathBuf) -> Self {
        Self {
            manifest,
            dir,
            status: PluginStatus::Discovered,
            child: None,
            writer: None,
            pending: std::sync::Arc::new(Mutex::new(HashMap::new())),
            subscriptions: HashSet::new(),
        }
    }

    fn exec_path(&self, workspace_root: &Path) -> PathBuf {
        let exec = &self.manifest.exec;
        let p = Path::new(exec);
        if p.is_absolute() || p.components().count() > 1 {
            self.dir.join(p)
        } else {
            // built binary typically lives in workspace_root/target/debug
            let mut path = workspace_root.join("target").join("debug");
            let exe = if cfg!(windows) {
                format!("{}{}.exe", exec, "")
            } else {
                exec.clone()
            };
            path.push(&exe);
            if path.exists() {
                path
            } else {
                // look for hashed cargo test binary
                let dir = workspace_root.join("target").join("debug");
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                            if name.starts_with(&exe) {
                                return p;
                            }
                        }
                    }
                }
                let deps = dir.join("deps");
                if let Ok(entries) = std::fs::read_dir(&deps) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                            if name.starts_with(&exe) {
                                return p;
                            }
                        }
                    }
                }
                path
            }
        }
    }
}

/// Manager responsible for discovering and running plugins.
pub struct PluginManager {
    workspace_root: PathBuf,
    pub plugins: HashMap<String, PluginHandle>,
}

impl PluginManager {
    /// Discover plugin manifests under a directory.
    pub fn discover(workspace_root: PathBuf, plugins_dir: PathBuf) -> Result<Self> {
        let mut plugins = HashMap::new();
        if plugins_dir.exists() {
            for entry in std::fs::read_dir(&plugins_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let dir = entry.path();
                    let manifest_path = dir.join("plugin.toml");
                    if manifest_path.exists() {
                        let text = std::fs::read_to_string(&manifest_path)?;
                        let manifest: PluginManifest = toml::from_str(&text)?;
                        let handle = PluginHandle::new(manifest.clone(), dir.clone());
                        plugins.insert(manifest.id.clone(), handle);
                    }
                }
            }
        }
        Ok(Self {
            workspace_root,
            plugins,
        })
    }

    /// List current plugins and their status.
    pub fn list(&self) -> Vec<(&PluginManifest, PluginStatus, &PathBuf)> {
        self.plugins
            .values()
            .map(|p| (&p.manifest, p.status.clone(), &p.dir))
            .collect()
    }

    /// Start all discovered plugins.
    pub async fn start_all(&mut self) -> Result<()> {
        let keys: Vec<String> = self.plugins.keys().cloned().collect();
        for id in keys {
            let handle = self.plugins.get_mut(&id).unwrap();
            PluginManager::start_plugin(&self.workspace_root, handle).await?;
        }
        Ok(())
    }

    async fn start_plugin(workspace_root: &Path, handle: &mut PluginHandle) -> Result<()> {
        let exec = handle.exec_path(workspace_root);
        let mut cmd = Command::new(exec);
        cmd.arg("--stdio").current_dir(&handle.dir);
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped());
        let mut child = cmd.spawn().context("spawning plugin")?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let writer = Arc::new(tokio::sync::Mutex::new(BufWriter::new(stdin)));
        let mut reader = BufReader::new(stdout);

        // send core.hello event
        let env = Envelope {
            id: None,
            kind: Kind::Event,
            method: None,
            params: None,
            result: None,
            error: None,
            topic: Some("core.hello".into()),
            payload: Some(json!({"api_version":"1","services":["log","event","timer","storage"]})),
        };
        {
            let mut w = writer.lock().await;
            write_envelope(&mut *w, &env).await?;
        }

        // wait for plugin.init request
        let env = read_envelope(&mut reader).await?;
        if env.kind == Kind::Request && env.method.as_deref() == Some("plugin.init") {
            // acknowledge
            let resp = Envelope {
                id: env.id.clone(),
                kind: Kind::Response,
                method: None,
                params: None,
                result: Some(json!({"ok":true})),
                error: None,
                topic: None,
                payload: None,
            };
            {
                let mut w = writer.lock().await;
                write_envelope(&mut *w, &resp).await?;
            }
        } else {
            anyhow::bail!("expected plugin.init request");
        }

        // expect plugin.start
        let env = read_envelope(&mut reader).await?;
        if env.kind == Kind::Request && env.method.as_deref() == Some("plugin.start") {
            let resp = Envelope {
                id: env.id.clone(),
                kind: Kind::Response,
                method: None,
                params: None,
                result: Some(json!({"ok":true})),
                error: None,
                topic: None,
                payload: None,
            };
            {
                let mut w = writer.lock().await;
                write_envelope(&mut *w, &resp).await?;
                let ready = Envelope {
                    id: None,
                    kind: Kind::Event,
                    method: None,
                    params: None,
                    result: None,
                    error: None,
                    topic: Some("system.ready".into()),
                    payload: None,
                };
                write_envelope(&mut *w, &ready).await?;
            }
            handle.status = PluginStatus::Running;
        } else {
            anyhow::bail!("expected plugin.start request");
        }

        let pending = handle.pending.clone();
        let subscriptions = std::sync::Arc::new(Mutex::new(HashSet::new()));
        handle.subscriptions = HashSet::new();
        let writer_clone = writer.clone();
        let plugin_id = handle.manifest.id.clone();

        // spawn reader task for further messages
        tokio::spawn(async move {
            let mut reader = reader;
            let writer = writer_clone;
            loop {
                match read_envelope(&mut reader).await {
                    Ok(env) => {
                        match env.kind {
                            Kind::Request => {
                                if let Some(method) = env.method.as_deref() {
                                    if method == "log.write" {
                                        if let Some(params) = env.params {
                                            if let (Some(level), Some(message)) =
                                                (params.get("level"), params.get("message"))
                                            {
                                                if let (Some(level), Some(message)) =
                                                    (level.as_str(), message.as_str())
                                                {
                                                    crate::services::log::write(level, message);
                                                }
                                            }
                                        }
                                        let resp = Envelope {
                                            id: env.id,
                                            kind: Kind::Response,
                                            method: None,
                                            params: None,
                                            result: Some(json!({"ok":true})),
                                            error: None,
                                            topic: None,
                                            payload: None,
                                        };
                                        let mut w = writer.lock().await;
                                        let _ = write_envelope(&mut *w, &resp).await;
                                    } else if method == "event.subscribe" {
                                        if let Some(params) = env.params {
                                            if let Some(arr) =
                                                params.get("topics").and_then(|t| t.as_array())
                                            {
                                                let mut subs = subscriptions.lock();
                                                for topic in arr {
                                                    if let Some(t) = topic.as_str() {
                                                        subs.insert(t.to_string());
                                                    }
                                                }
                                            }
                                        }
                                        let resp = Envelope {
                                            id: env.id,
                                            kind: Kind::Response,
                                            method: None,
                                            params: None,
                                            result: Some(json!({"ok":true})),
                                            error: None,
                                            topic: None,
                                            payload: None,
                                        };
                                        let mut w = writer.lock().await;
                                        let _ = write_envelope(&mut *w, &resp).await;
                                    } else if method == "timer.set_interval" {
                                        if let Some(params) = env.params {
                                            if let (Some(id_val), Some(ms_val)) =
                                                (params.get("id"), params.get("millis"))
                                            {
                                                if let (Some(id), Some(ms)) =
                                                    (id_val.as_str(), ms_val.as_u64())
                                                {
                                                    let writer_inner = writer.clone();
                                                    crate::services::timer::spawn_timer(
                                                        writer_inner,
                                                        crate::services::timer::TimerParams {
                                                            id: id.to_string(),
                                                            millis: ms,
                                                        },
                                                    );
                                                }
                                            }
                                        }
                                        let resp = Envelope {
                                            id: env.id,
                                            kind: Kind::Response,
                                            method: None,
                                            params: None,
                                            result: Some(json!({"ok":true})),
                                            error: None,
                                            topic: None,
                                            payload: None,
                                        };
                                        let mut w = writer.lock().await;
                                        let _ = write_envelope(&mut *w, &resp).await;
                                    } else {
                                        // unknown method
                                        let resp = Envelope {
                                            id: env.id,
                                            kind: Kind::Response,
                                            method: None,
                                            params: None,
                                            result: None,
                                            error: Some(plugin_api::RpcError {
                                                code: -32601,
                                                message: format!("unknown method {}", method),
                                            }),
                                            topic: None,
                                            payload: None,
                                        };
                                        let mut w = writer.lock().await;
                                        let _ = write_envelope(&mut *w, &resp).await;
                                    }
                                }
                            }
                            Kind::Response => {
                                if let Some(id) = env.id.clone() {
                                    if let Some(tx) = pending.lock().remove(&id) {
                                        let _ = tx.send(env);
                                    }
                                }
                            }
                            Kind::Event => {
                                // ignore events from plugin
                            }
                        }
                    }
                    Err(err) => {
                        error!("error reading from plugin {plugin_id}: {err}");
                        break;
                    }
                }
            }
        });

        handle.writer = Some(writer);
        handle.child = Some(child);
        Ok(())
    }

    /// Send a request to a plugin and wait for the response.
    pub async fn call(&self, plugin_id: &str, method: &str, params: Value) -> Result<Value> {
        let handle = self.plugins.get(plugin_id).context("plugin not found")?;
        let writer = handle.writer.as_ref().context("plugin not running")?;
        let id = Uuid::new_v4().to_string();
        let env = Envelope {
            id: Some(id.clone()),
            kind: Kind::Request,
            method: Some(method.to_string()),
            params: Some(params),
            result: None,
            error: None,
            topic: None,
            payload: None,
        };
        let (tx, rx) = oneshot::channel();
        handle.pending.lock().insert(id.clone(), tx);
        {
            let mut w = writer.lock().await;
            write_envelope(&mut *w, &env).await?;
        }
        let resp = rx.await?;
        if let Some(err) = resp.error {
            anyhow::bail!(err.message);
        }
        Ok(resp.result.unwrap_or(Value::Null))
    }
}
