use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Command line interface for the homecore application.
#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Cli {
    /// Directory containing plugin manifests.
    #[arg(long)]
    pub plugins_dir: Option<PathBuf>,
    /// Start without loading any plugins.
    #[arg(long)]
    pub safe_mode: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Run the core application normally.
    Run,
    /// Operations on plugins.
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum PluginCommand {
    /// List discovered plugins.
    List,
}
