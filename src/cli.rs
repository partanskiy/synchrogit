use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use tokio_util::sync::CancellationToken;

use crate::config::{load, load_from_path};
use crate::ipc::protocol::{Request, Response};
use crate::ipc::{client, default_socket_path, server};
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

    /// Print the repos currently supervised by the daemon.
    Status(ClientArgs),

    /// Ask the daemon to run a sync cycle now.
    Sync(SyncArgs),

    /// Ask the daemon to reload its config.
    Reload(ClientArgs),
}

#[derive(Debug, Args)]
pub struct RunArgs {
    /// Config file path. Defaults to XDG precedence.
    #[arg(long, env = "SYNCHROGIT_CONFIG")]
    pub config: Option<PathBuf>,

    /// Unix socket path for CLI control commands.
    #[arg(long, env = "SYNCHROGIT_SOCKET")]
    pub socket: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ClientArgs {
    /// Unix socket path for daemon control.
    #[arg(long, env = "SYNCHROGIT_SOCKET")]
    pub socket: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SyncArgs {
    /// Optional repo name. If omitted, all repos are queued.
    pub repo: Option<String>,

    /// Unix socket path for daemon control.
    #[arg(long, env = "SYNCHROGIT_SOCKET")]
    pub socket: Option<PathBuf>,
}

pub async fn dispatch(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Run(args) => run(args).await,
        Command::Status(args) => request_and_print(args.socket, Request::Status).await,
        Command::Sync(args) => {
            request_and_print(args.socket, Request::Sync { repo: args.repo }).await
        }
        Command::Reload(args) => request_and_print(args.socket, Request::Reload).await,
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
    let socket = args.socket.unwrap_or_else(default_socket_path);
    let cancel = CancellationToken::new();
    let ipc = server::spawn(socket, supervisor.control(), cancel.clone()).await?;
    wait_for_shutdown(supervisor, ipc, cancel).await
}

async fn wait_for_shutdown(
    supervisor: Supervisor,
    ipc: server::ServerHandle,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    use tokio::signal::unix::{SignalKind, signal};
    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;
    tokio::select! {
        _ = sigint.recv() => {},
        _ = sigterm.recv() => {},
    }
    tracing::info!("shutting down");
    cancel.cancel();
    let _ = ipc.join.await;
    supervisor.shutdown().await;
    Ok(())
}

async fn request_and_print(socket: Option<PathBuf>, request: Request) -> anyhow::Result<()> {
    let socket = socket.unwrap_or_else(default_socket_path);
    let response = client::request(&socket, request).await?;
    print_response(response)
}

fn print_response(response: Response) -> anyhow::Result<()> {
    match response {
        Response::Pong => {
            println!("pong");
        }
        Response::Status { repos } => {
            for repo in repos {
                println!("{}\t{}", repo.name, repo.path);
            }
        }
        Response::Reloaded { ok, message } => {
            if ok {
                println!("{message}");
            } else {
                anyhow::bail!("{message}");
            }
        }
        Response::Synced { queued } => {
            println!("queued: {}", queued.join(", "));
        }
        Response::Error { message } => anyhow::bail!("{message}"),
    }
    Ok(())
}
