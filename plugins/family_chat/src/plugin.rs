use anyhow::Result;

/// Entry point for running the plugin either as a standalone HTTP server or
/// via the homecore stdio protocol.
pub async fn run(stdio: bool, bind: String) -> Result<()> {
    if stdio {
        crate::core_bridge::run_stdio(&bind).await
    } else {
        crate::api::run_http_server(bind).await
    }
}
