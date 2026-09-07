use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tokio::net::{UnixListener, UnixStream};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use super::connection::handle_connection;
use crate::error::{Result, SynchrogitError};
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
