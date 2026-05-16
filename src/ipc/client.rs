use std::path::Path;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::error::Result;
use crate::ipc::protocol::{Request, Response};

pub async fn request(socket: &Path, request: Request) -> Result<Response> {
    let mut stream = UnixStream::connect(socket).await?;
    let payload = serde_json::to_vec(&request)?;
    stream.write_all(&payload).await?;
    stream.write_all(b"\n").await?;
    stream.flush().await?;

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        return Ok(Response::error(
            "daemon closed the connection without a response",
        ));
    }
    Ok(serde_json::from_str(line.trim_end())?)
}
