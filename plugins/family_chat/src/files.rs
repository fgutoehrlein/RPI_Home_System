use anyhow::Result;
use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::fs;

/// Save file data into a content-addressed store and return its hash id.
pub async fn save_file<P: AsRef<Path>>(base: P, data: Bytes) -> Result<String> {
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let hash = format!("{:x}", hasher.finalize());
    let sub = &hash[..2];
    let dir = base.as_ref().join(sub);
    fs::create_dir_all(&dir).await?;
    let path = dir.join(&hash);
    fs::write(path, data).await?;
    Ok(hash)
}
