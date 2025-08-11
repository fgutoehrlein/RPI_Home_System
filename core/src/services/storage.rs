use anyhow::Result;
use directories::ProjectDirs;
use serde_json::Value;
use std::{collections::HashMap, path::PathBuf};
use tokio::{fs, sync::Mutex};

/// Simple JSON based key-value storage for plugins.
pub struct Storage {
    file: PathBuf,
    data: Mutex<HashMap<String, Value>>,
}

impl Storage {
    /// Create storage for a specific plugin id.
    pub async fn new(plugin_id: &str) -> Result<Self> {
        let proj = ProjectDirs::from("org", "homecore", "homecore").unwrap();
        let dir = proj.data_dir().join("plugins").join(plugin_id);
        fs::create_dir_all(&dir).await?;
        let file = dir.join("data.json");
        let data = if let Ok(bytes) = fs::read(&file).await {
            serde_json::from_slice(&bytes).unwrap_or_default()
        } else {
            HashMap::new()
        };
        Ok(Self { file, data: Mutex::new(data) })
    }

    /// Retrieve a value by key.
    pub async fn get(&self, key: &str) -> Option<Value> {
        self.data.lock().await.get(key).cloned()
    }

    /// Store a value under a key.
    pub async fn put(&self, key: String, value: Value) -> Result<()> {
        let mut data = self.data.lock().await;
        data.insert(key, value);
        let bytes = serde_json::to_vec(&*data)?;
        fs::write(&self.file, bytes).await?;
        Ok(())
    }
}
