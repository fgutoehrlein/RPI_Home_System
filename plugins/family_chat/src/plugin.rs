use anyhow::Result;

use crate::config::Config;

/// Entry point for running the plugin either as a standalone HTTP server or
/// via the homecore stdio protocol.
pub async fn run(stdio: bool, config: Config) -> Result<()> {
    if stdio {
        crate::core_bridge::run_stdio(config).await
    } else {
        crate::api::run_http_server(config).await
    }
}
