mod api;
mod auth;
mod config;
mod core_bridge;
mod db;
mod embed;
mod files;
mod housekeeping;
mod messages;
mod model;
mod plugin;
mod presence;
mod reads;
mod rooms;
mod typing;
mod ws;

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = config::Cli::parse();
    let cfg = config::Config::load(&cli)?;
    let level = if cfg.logging_enabled {
        tracing::Level::INFO
    } else {
        tracing::Level::WARN
    };
    tracing_subscriber::fmt().with_max_level(level).init();
    plugin::run(cli.stdio, cfg).await
}
