pub mod cli;
pub mod events;
pub mod ipc;
pub mod plugin_host;
pub mod services;

pub use plugin_host::PluginManager;

use anyhow::Result;
use std::path::PathBuf;

/// Determine the workspace root by walking up from the current executable
/// until a `Cargo.toml` file is found.
pub fn workspace_root() -> Result<PathBuf> {
    let mut path = std::env::current_exe()?;
    while path.pop() {
        let candidate = path.join("Cargo.toml");
        if candidate.exists() {
            return Ok(path);
        }
    }
    Err(anyhow::anyhow!("failed to determine workspace root"))
}
