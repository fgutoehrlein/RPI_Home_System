#![allow(dead_code)]

use std::path::PathBuf;

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
