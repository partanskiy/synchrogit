use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use tokio_util::sync::CancellationToken;

use crate::config::{load, load_from_path, watcher};
use crate::ipc::protocol::{Request, Response};
use crate::ipc::{client, default_socket_path, server};
use crate::runtime::{Supervisor, SupervisorControl};

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
    let config_path = loaded.path.clone();
    let repo_count = loaded.config.repos.len();
    tracing::info!(
        config = %config_path.display(),
        repos = repo_count,
        "config loaded"
    );
    let supervisor = Supervisor::spawn_loaded(loaded)?;
    let socket = args.socket.unwrap_or_else(default_socket_path);
    let cancel = CancellationToken::new();
    let control = supervisor.control();
    let ipc = server::spawn(socket, control.clone(), cancel.clone()).await?;
    let reload_watcher = spawn_reload_watcher(config_path, control, cancel.clone());
    wait_for_shutdown(supervisor, ipc, reload_watcher, cancel).await
}

async fn wait_for_shutdown(
    supervisor: Supervisor,
    ipc: server::ServerHandle,
    reload_watcher: tokio::task::JoinHandle<()>,
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
    let _ = reload_watcher.await;
    supervisor.shutdown().await;
    Ok(())
}

fn spawn_reload_watcher(
    config_path: PathBuf,
    control: SupervisorControl,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut watcher = match watcher::spawn(&config_path) {
            Ok(watcher) => watcher,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    config = %config_path.display(),
                    "config watcher disabled"
                );
                return;
            }
        };

        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break,
                Some(_) = watcher.rx.recv() => {
                    match control.reload().await {
                        Ok(report) => tracing::info!(message = %report.message(), "config reloaded"),
                        Err(e) => tracing::warn!(error = %e, "config reload failed; keeping previous config"),
                    }
                }
            }
        }
    })
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
                println!(
                    "{}\t{}\tbranch={}\tupstream={}\trunning={}\toutcome={}\tfailures={}\terror={}",
                    repo.name,
                    repo.path,
                    repo.current_branch.as_deref().unwrap_or("-"),
                    repo.upstream.as_deref().unwrap_or("-"),
                    repo.running,
                    repo.last_sync.last_outcome,
                    repo.last_sync.consecutive_failures,
                    repo.last_sync.last_error.as_deref().unwrap_or("-"),
                );
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
