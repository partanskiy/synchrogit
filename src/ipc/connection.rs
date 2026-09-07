use crate::error::Result;
use crate::ipc::protocol::{Request, Response};
use crate::runtime::SupervisorControl;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
pub(super) async fn handle_connection<S>(stream: S, control: SupervisorControl) -> Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
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
        Request::Reload => match control.reload().await {
            Ok(report) => Response::Reloaded {
                ok: true,
                message: report.message(),
            },
            Err(e) => Response::Reloaded {
                ok: false,
                message: e,
            },
        },
    }
}
