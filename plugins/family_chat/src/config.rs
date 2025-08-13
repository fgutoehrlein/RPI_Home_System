#![allow(dead_code)]

use std::path::PathBuf;

/// Runtime configuration for the server.
#[derive(Clone, Debug)]
pub struct Config {
    /// Address to bind the HTTP server to.
    pub bind: String,
    /// Base directory for storing data such as uploaded files.
    pub data_dir: PathBuf,
    /// Maximum upload size in megabytes.
    pub max_upload_mb: u64,
}

impl Config {
    /// Load configuration from environment variables, falling back to
    /// sensible defaults when not present.
    pub fn from_env() -> Self {
        let bind = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:8787".into());
        let data_dir = std::env::var("DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_data_dir());
        let max_upload_mb = std::env::var("MAX_UPLOAD_MB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);
        Self {
            bind,
            data_dir,
            max_upload_mb,
        }
    }

    /// Helper to return the upload limit in bytes.
    pub fn max_upload_bytes(&self) -> u64 {
        self.max_upload_mb * 1024 * 1024
    }
}

/// Determine the default data directory for the plugin.
pub fn default_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("PLUGIN_DATA_DIR") {
        PathBuf::from(dir)
    } else if let Ok(home) = std::env::var("HOME") {
        let mut p = PathBuf::from(home);
        p.push(".local/share/homecore/plugins/family_chat");
        p
    } else {
        PathBuf::from("./family_chat_data")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_env_config() {
        std::env::set_var("BIND", "127.0.0.1:9999");
        std::env::set_var("DATA_DIR", "/tmp/data");
        std::env::set_var("MAX_UPLOAD_MB", "10");
        let cfg = Config::from_env();
        assert_eq!(cfg.bind, "127.0.0.1:9999");
        assert_eq!(cfg.data_dir, PathBuf::from("/tmp/data"));
        assert_eq!(cfg.max_upload_mb, 10);
    }
}
