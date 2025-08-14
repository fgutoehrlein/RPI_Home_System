#![allow(dead_code)]

use anyhow::Result;
use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Sanitize an incoming filename to avoid path traversal and control characters.
pub fn sanitize_filename(name: &str) -> String {
    let name = name.replace(['/', '\\'], "_");
    name.chars().filter(|c| !c.is_control()).collect()
}

/// Detect MIME type from content or filename.
pub fn detect_mime(name: &str, data: &[u8]) -> String {
    infer::get(data)
        .map(|k| k.mime_type().to_string())
        .or_else(|| mime_guess::from_path(name).first().map(|m| m.to_string()))
        .unwrap_or_else(|| "application/octet-stream".into())
}

/// Simple allowlist for safe MIME types.
pub fn allowed_mime(mime: &str) -> bool {
    const ALLOWED: &[&str] = &["text/plain", "image/png", "image/jpeg", "application/pdf"];
    ALLOWED.iter().any(|m| m.eq_ignore_ascii_case(mime))
}

/// Generate a small thumbnail for image data. Returns PNG bytes and dimensions.
pub fn generate_thumbnail(data: &[u8]) -> Result<Option<(Vec<u8>, u32, u32)>> {
    match image::load_from_memory(data) {
        Ok(img) => {
            let thumb = img.thumbnail(128, 128);
            let (w, h) = (thumb.width(), thumb.height());
            let mut out = Vec::new();
            {
                let mut cursor = std::io::Cursor::new(&mut out);
                thumb.write_to(&mut cursor, image::ImageOutputFormat::Png)?;
            }
            Ok(Some((out, w, h)))
        }
        Err(_) => Ok(None),
    }
}

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

/// Remove files from the content store that are not referenced in the provided set.
pub async fn cleanup_orphans<P: AsRef<Path>>(base: P, keep: &HashSet<String>) -> Result<()> {
    let mut dirs = fs::read_dir(base).await?;
    while let Some(dir) = dirs.next_entry().await? {
        if dir.file_type().await?.is_dir() {
            let mut files = fs::read_dir(dir.path()).await?;
            while let Some(f) = files.next_entry().await? {
                let name = f.file_name().to_string_lossy().to_string();
                if !keep.contains(&name) {
                    let _ = fs::remove_file(f.path()).await;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

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

    #[test]
    fn mime_detection_and_allowlist() {
        let mime = detect_mime("foo.txt", b"hi");
        assert_eq!(mime, "text/plain");
        assert!(allowed_mime(&mime));
        assert!(!allowed_mime("application/x-msdownload"));
    }

    #[test]
    fn sanitizes_filename() {
        let name = sanitize_filename("../evil\\name.txt");
        assert_eq!(name, ".._evil_name.txt");
    }

    #[test]
    fn generates_thumbnail() {
        use image::{ImageOutputFormat, RgbImage};
        let img = RgbImage::from_pixel(1, 1, image::Rgb([0, 0, 0]));
        let mut data = Vec::new();
        {
            let mut cursor = std::io::Cursor::new(&mut data);
            img.write_to(&mut cursor, ImageOutputFormat::Png).unwrap();
        }
        let thumb = generate_thumbnail(&data).unwrap();
        assert!(thumb.is_some());
    }

    #[tokio::test]
    async fn cleans_orphans() {
        let tmp = tempfile::tempdir().unwrap();
        let id = save_file(tmp.path(), Bytes::from_static(b"hello"))
            .await
            .unwrap();
        let path = file_path(tmp.path(), &id);
        let keep = HashSet::new();
        cleanup_orphans(tmp.path(), &keep).await.unwrap();
        assert!(!path.exists());
    }
}
