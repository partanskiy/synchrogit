use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};

use crate::clock::DEFAULT_COMMIT_TEMPLATE;
use crate::worker::{WorkerConfig, WorkerHandle, spawn};

#[derive(Debug, Parser)]
#[command(name = "synchrogit", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the sync daemon for a single repository.
    ///
    /// Multi-repo via a TOML config file lands in a later PR.
    Run(RunArgs),
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Path to the git repository to keep in sync.
    #[arg(long, env = "SYNCHROGIT_REPO")]
    pub repo: PathBuf,

    /// Logical name for the repo (defaults to the directory name).
    #[arg(long)]
    pub name: Option<String>,

    /// How often to run a full sync cycle.
    #[arg(long, default_value = "15s", value_parser = parse_duration)]
    pub interval: Duration,

    /// Quiet window after a filesystem event before triggering a sync.
    #[arg(long, default_value = "2s", value_parser = parse_duration)]
    pub debounce: Duration,

    /// Disable `git push`.
    #[arg(long)]
    pub no_push: bool,

    /// Disable `git fetch` + `git pull`.
    #[arg(long)]
    pub no_pull: bool,
}

fn parse_duration(s: &str) -> std::result::Result<Duration, String> {
    humantime::parse_duration(s).map_err(|e| e.to_string())
}

pub async fn dispatch(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Run(args) => run(args).await,
    }
}

async fn run(args: RunArgs) -> anyhow::Result<()> {
    let name = args.name.unwrap_or_else(|| {
        args.repo
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "repo".to_string())
    });
    let handle = spawn(WorkerConfig {
        name,
        path: args.repo,
        interval: args.interval,
        debounce: args.debounce,
        commit_template: DEFAULT_COMMIT_TEMPLATE.to_string(),
        auto_push: !args.no_push,
        auto_pull: !args.no_pull,
    })?;
    wait_for_shutdown(handle).await
}

async fn wait_for_shutdown(handle: WorkerHandle) -> anyhow::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = sigint.recv() => {},
        _ = sigterm.recv() => {},
    }
    tracing::info!("shutting down");
    handle.cancel.cancel();
    let _ = handle.join.await;
    Ok(())
}
