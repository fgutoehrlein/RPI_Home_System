#![allow(dead_code)]

use anyhow::Result;
use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
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

/// Determine the on-disk path for a file id within the store.
pub fn file_path<P: AsRef<Path>>(base: P, id: &str) -> PathBuf {
    let sub = &id[..2];
    base.as_ref().join(sub).join(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn saves_and_paths_file() {
        let tmp = tempfile::tempdir().unwrap();
        let id = save_file(tmp.path(), Bytes::from_static(b"hello"))
            .await
            .unwrap();
        let expected = file_path(tmp.path(), &id);
        assert!(expected.exists());
        // ensure path includes first two chars as directory
        let subdir = &id[..2];
        assert!(expected.parent().unwrap().ends_with(subdir));
    }
}
