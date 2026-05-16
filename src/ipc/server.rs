use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::error::{Result, SynchrogitError};
use crate::ipc::protocol::{Request, Response};
use crate::runtime::SupervisorControl;

pub struct ServerHandle {
    pub path: PathBuf,
    pub join: JoinHandle<()>,
}

pub async fn spawn(
    path: PathBuf,
    control: SupervisorControl,
    cancel: CancellationToken,
) -> Result<ServerHandle> {
    prepare_socket(&path).await?;
    let listener = UnixListener::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;

    let path_for_task = path.clone();
    let join = tokio::spawn(async move {
        if let Err(e) = serve(listener, control, cancel, &path_for_task).await {
            warn!(error = %e, "ipc server stopped with error");
        }
    });

    Ok(ServerHandle { path, join })
}

async fn prepare_socket(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        tokio::fs::create_dir_all(parent).await?;
    }

    if !path.exists() {
        return Ok(());
    }

    match UnixStream::connect(path).await {
        Ok(_) => Err(SynchrogitError::Other(format!(
            "synchrogit daemon is already listening at {}",
            path.display()
        ))),
        Err(e) if matches!(e.kind(), ErrorKind::ConnectionRefused | ErrorKind::NotFound) => {
            tokio::fs::remove_file(path).await?;
            Ok(())
        }
        Err(e) => Err(SynchrogitError::Other(format!(
            "failed to probe socket {}: {e}",
            path.display()
        ))),
    }
}

async fn serve(
    listener: UnixListener,
    control: SupervisorControl,
    cancel: CancellationToken,
    path: &Path,
) -> Result<()> {
    info!(socket = %path.display(), "ipc server started");

    loop {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            accepted = listener.accept() => {
                let (stream, _) = accepted?;
                let control = control.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, control).await {
                        debug!(error = %e, "ipc connection failed");
                    }
                });
            }
        }
    }

    if let Err(e) = tokio::fs::remove_file(path).await
        && e.kind() != ErrorKind::NotFound
    {
        warn!(error = %e, socket = %path.display(), "failed to remove ipc socket");
    }
    info!("ipc server stopped");
    Ok(())
}

async fn handle_connection(stream: UnixStream, control: SupervisorControl) -> Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        return Ok(());
    }

    let response = match serde_json::from_str::<Request>(line.trim_end()) {
        Ok(request) => handle_request(request, &control).await,
        Err(e) => Response::error(format!("invalid request: {e}")),
    };

    let mut stream = reader.into_inner();
    let payload = serde_json::to_vec(&response)?;
    stream.write_all(&payload).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;
    Ok(())
}

async fn handle_request(request: Request, control: &SupervisorControl) -> Response {
    match request {
        Request::Ping => Response::Pong,
        Request::Status => Response::Status {
            repos: control.status(),
        },
        Request::Sync { repo } => match control.sync(repo.as_deref()).await {
            Ok(queued) => Response::Synced { queued },
            Err(e) => Response::error(e),
        },
        Request::Reload => Response::Reloaded {
            ok: false,
            message: "manual reload lands with the config hot-reload milestone".to_string(),
        },
    }
}
