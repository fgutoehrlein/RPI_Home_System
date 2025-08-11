use anyhow::Result;
use clap::Parser;
use tracing::{info, warn};

use homecore::{
    cli::{Cli, Command, PluginCommand},
    workspace_root, PluginManager,
};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let cli = Cli::parse();
    let workspace = workspace_root()?;
    let plugins_dir = cli.plugins_dir.clone().unwrap_or(workspace.join("plugins"));

    match cli.command {
        Command::Run => {
            if cli.safe_mode {
                warn!("safe mode enabled - not loading plugins");
                tokio::signal::ctrl_c().await?;
                return Ok(());
            }
            let mut manager = PluginManager::discover(workspace.clone(), plugins_dir)?;
            manager.start_all().await?;
            info!("plugins running - press Ctrl+C to exit");
            tokio::signal::ctrl_c().await?;
        }
        Command::Plugin {
            command: PluginCommand::List,
        } => {
            let manager = PluginManager::discover(workspace.clone(), plugins_dir)?;
            for (manifest, status, path) in manager.list() {
                println!(
                    "{:<15} {:<20} {:<8} {:?} {}",
                    manifest.id,
                    manifest.name,
                    manifest.version,
                    status,
                    path.display()
                );
            }
        }
    }
    Ok(())
}
