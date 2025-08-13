mod api;
mod auth;
mod config;
mod core_bridge;
mod db;
mod embed;
mod files;
mod housekeeping;
mod model;
mod plugin;
mod rooms;
mod ws;
mod messages;

use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
struct Opts {
    /// Run with stdio protocol used by the core.
    #[arg(long)]
    stdio: bool,
    /// Address to bind the HTTP server to when running standalone.
    #[arg(long, default_value = "0.0.0.0:8787")]
    bind: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    let opts = Opts::parse();
    plugin::run(opts.stdio, opts.bind).await
}
