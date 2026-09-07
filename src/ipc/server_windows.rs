use super::{connection::handle_connection, windows_pipe};
use crate::error::Result;
use crate::runtime::SupervisorControl;
use std::path::PathBuf;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct ServerHandle {
    pub path: PathBuf,
    pub join: JoinHandle<()>,
}

pub async fn spawn(
    path: PathBuf,
    control: SupervisorControl,
    cancel: CancellationToken,
) -> Result<ServerHandle> {
    let mut listener = windows_pipe::create(&path, true)?;
    let endpoint = path.clone();
    let join = tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => break,
                result = listener.connect() => {
                    if let Err(error) = result { tracing::warn!(%error, "pipe accept failed"); break; }
                    // Create the next instance before handing off this one, so
                    // clients never observe a missing pipe between connections.
                    let next = match windows_pipe::create(&endpoint, false) {
                        Ok(next) => next,
                        Err(error) => { tracing::warn!(%error, "pipe creation failed"); break; }
                    };
                    let stream = std::mem::replace(&mut listener, next);
                    let control = control.clone();
                    tokio::spawn(async move {
                        if let Err(error) = handle_connection(stream, control).await { tracing::debug!(%error, "pipe connection failed"); }
                    });
                }
            }
        }
    });
    Ok(ServerHandle { path, join })
}
