use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

use crate::config::{load, load_from_path};
use crate::runtime::Supervisor;

#[derive(Debug, Parser)]
#[command(name = "synchrogit", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the sync daemon for all repositories in the config file.
    Run(RunArgs),
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Config file path. Defaults to XDG precedence.
    #[arg(long, env = "SYNCHROGIT_CONFIG")]
    pub config: Option<PathBuf>,
}

pub async fn dispatch(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Run(args) => run(args).await,
    }
}

async fn run(args: RunArgs) -> anyhow::Result<()> {
    let loaded = match args.config {
        Some(path) => load_from_path(path)?,
        None => load()?,
    };
    let repo_count = loaded.config.repos.len();
    tracing::info!(
        config = %loaded.path.display(),
        repos = repo_count,
        "config loaded"
    );
    let supervisor = Supervisor::spawn(loaded.config)?;
    wait_for_shutdown(supervisor).await
}

async fn wait_for_shutdown(supervisor: Supervisor) -> anyhow::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = sigint.recv() => {},
        _ = sigterm.recv() => {},
    }
    tracing::info!("shutting down");
    supervisor.shutdown().await;
    Ok(())
}
