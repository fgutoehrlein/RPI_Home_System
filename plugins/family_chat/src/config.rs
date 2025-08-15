use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;

/// Command line options for the plugin.
#[derive(Parser, Debug, Default)]
pub struct Cli {
    /// Run with stdio protocol used by the core.
    #[arg(long)]
    pub stdio: bool,
    /// Override bind address (host:port).
    #[arg(long)]
    pub bind: Option<String>,
    /// Override server port.
    #[arg(long)]
    pub port: Option<u16>,
    /// Enable or disable logging (true/false).
    #[arg(long)]
    pub logging: Option<bool>,
    /// Path to configuration file.
    #[arg(long)]
    pub config: Option<PathBuf>,
}

#[derive(Clone)]
pub struct Bootstrap {
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for Bootstrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bootstrap")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

/// Runtime configuration for the server resolved from file, env and CLI.
#[derive(Clone, Debug)]
pub struct Config {
    /// Address to bind the HTTP server to.
    pub bind: String,
    /// Base directory for storing data such as uploaded files.
    pub data_dir: PathBuf,
    /// Maximum upload size in megabytes.
    pub max_upload_mb: u64,
    /// Whether verbose logging is enabled.
    pub logging_enabled: bool,
    /// Bootstrap credentials, consumed on first run.
    pub bootstrap: Option<Bootstrap>,
}

#[derive(Deserialize, Default)]
struct FileConfig {
    #[serde(default)]
    bootstrap: Option<FileBootstrap>,
    #[serde(default)]
    server: FileServer,
    #[serde(default)]
    logging: FileLogging,
}

#[derive(Deserialize)]
struct FileBootstrap {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct FileServer {
    #[serde(default = "default_port")]
    port: u16,
}

#[derive(Deserialize)]
struct FileLogging {
    #[serde(default = "default_logging")]
    enabled: bool,
}

fn default_port() -> u16 {
    8787
}

fn default_logging() -> bool {
    true
}

impl Default for FileServer {
    fn default() -> Self {
        Self {
            port: default_port(),
        }
    }
}

impl Default for FileLogging {
    fn default() -> Self {
        Self {
            enabled: default_logging(),
        }
    }
}

impl Config {
    /// Resolve configuration from CLI, environment variables, config file and defaults.
    pub fn load(cli: &Cli) -> Result<Self> {
        // built-in defaults
        let mut port = default_port();
        let mut logging = default_logging();
        let mut bootstrap: Option<Bootstrap> = None;

        // config file path precedence: CLI -> ENV -> default
        let config_path = cli
            .config
            .clone()
            .or_else(|| std::env::var("FAMILY_CHAT_CONFIG").ok().map(PathBuf::from))
            .unwrap_or_else(|| PathBuf::from("config/family_chat.toml"));

        if let Ok(bytes) = fs::read(&config_path) {
            let contents = String::from_utf8_lossy(&bytes);
            let file_cfg: FileConfig = toml::from_str(&contents).context("invalid config file")?;
            if let Some(b) = file_cfg.bootstrap {
                bootstrap = Some(Bootstrap {
                    username: b.username,
                    password: b.password,
                });
            }
            port = file_cfg.server.port;
            logging = file_cfg.logging.enabled;
        }

        // environment overrides
        if let Ok(p) = std::env::var("FAMILY_CHAT_PORT") {
            if let Ok(p) = p.parse::<u16>() {
                port = p;
            }
        }
        if let Ok(l) = std::env::var("FAMILY_CHAT_LOGGING") {
            if let Ok(l) = l.parse::<bool>() {
                logging = l;
            }
        }

        // CLI overrides
        if let Some(p) = cli.port {
            port = p;
        }
        if let Some(l) = cli.logging {
            logging = l;
        }

        // validate port range
        if !(1024..=65535).contains(&port) {
            anyhow::bail!("invalid_port");
        }

        // bind address precedence for host override
        let bind = if let Some(b) = &cli.bind {
            b.clone()
        } else if let Ok(b) = std::env::var("BIND") {
            b
        } else {
            format!("127.0.0.1:{}", port)
        };

        // existing env variables for data dir and upload limit
        let data_dir = std::env::var("DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_data_dir());
        let max_upload_mb = std::env::var("MAX_UPLOAD_MB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        Ok(Self {
            bind,
            data_dir,
            max_upload_mb,
            logging_enabled: logging,
            bootstrap,
        })
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
    use serial_test::serial;
    use std::fs;

    #[test]
    #[serial]
    fn valid_config_parses() {
        std::env::remove_var("FAMILY_CHAT_PORT");
        std::env::remove_var("FAMILY_CHAT_LOGGING");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cfg.toml");
        fs::write(&path, "[server]\nport=5555\n[logging]\nenabled=false\n").unwrap();
        let cli = Cli {
            config: Some(path),
            ..Default::default()
        };
        let cfg = Config::load(&cli).unwrap();
        assert_eq!(cfg.bind, "127.0.0.1:5555");
        assert!(!cfg.logging_enabled);
    }

    #[test]
    #[serial]
    fn invalid_port_fails() {
        std::env::remove_var("FAMILY_CHAT_PORT");
        std::env::remove_var("FAMILY_CHAT_LOGGING");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cfg.toml");
        fs::write(&path, "[server]\nport=80\n").unwrap();
        let cli = Cli {
            config: Some(path),
            ..Default::default()
        };
        assert!(Config::load(&cli).is_err());
    }

    #[test]
    #[serial]
    fn missing_keys_defaults() {
        std::env::remove_var("FAMILY_CHAT_PORT");
        std::env::remove_var("FAMILY_CHAT_LOGGING");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cfg.toml");
        fs::write(&path, "").unwrap();
        let cli = Cli {
            config: Some(path),
            ..Default::default()
        };
        let cfg = Config::load(&cli).unwrap();
        assert_eq!(cfg.bind, "127.0.0.1:8787");
        assert!(cfg.logging_enabled);
    }

    #[test]
    #[serial]
    fn precedence_cli_env_file() {
        std::env::remove_var("FAMILY_CHAT_LOGGING");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cfg.toml");
        fs::write(&path, "[server]\nport=1111\n").unwrap();
        std::env::set_var("FAMILY_CHAT_PORT", "2222");
        let cli = Cli {
            config: Some(path),
            port: Some(3333),
            ..Default::default()
        };
        let cfg = Config::load(&cli).unwrap();
        assert_eq!(cfg.bind, "127.0.0.1:3333");
        std::env::remove_var("FAMILY_CHAT_PORT");
    }

    #[test]
    #[serial]
    fn file_value_used_when_no_overrides() {
        std::env::remove_var("FAMILY_CHAT_PORT");
        std::env::remove_var("FAMILY_CHAT_LOGGING");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cfg.toml");
        fs::write(&path, "[server]\nport=4444\n").unwrap();
        let cli = Cli {
            config: Some(path),
            ..Default::default()
        };
        let cfg = Config::load(&cli).unwrap();
        assert_eq!(cfg.bind, "127.0.0.1:4444");
    }

    #[test]
    #[serial]
    fn logging_toggle() {
        std::env::remove_var("FAMILY_CHAT_PORT");
        std::env::remove_var("FAMILY_CHAT_LOGGING");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cfg.toml");
        fs::write(&path, "[logging]\nenabled=false\n").unwrap();
        let cli = Cli {
            config: Some(path),
            ..Default::default()
        };
        let cfg = Config::load(&cli).unwrap();
        assert!(!cfg.logging_enabled);
    }
}
