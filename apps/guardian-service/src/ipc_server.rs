//! Named-pipe IPC server for tray / CLI clients.

use std::sync::Arc;

use anyhow::Result;
use guardian_core::{ClientRequest, ServerPush, PIPE_NAME};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::sync::Mutex;

use crate::runtime::ServiceInner;

#[derive(Clone)]
pub struct SharedState {
    pub inner: Arc<Mutex<ServiceInner>>,
}

pub async fn serve(shared: SharedState) -> Result<()> {
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(PIPE_NAME)?;

    loop {
        server.connect().await?;
        let connected = server;
        server = ServerOptions::new().create(PIPE_NAME)?;

        let shared2 = shared.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(connected, shared2).await {
                tracing::debug!(error = %e, "ipc client ended");
            }
        });
    }
}

async fn handle_client(server: NamedPipeServer, shared: SharedState) -> Result<()> {
    let (reader_half, mut writer) = tokio::io::split(server);
    let mut reader = BufReader::new(reader_half);
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        let req: ClientRequest = match serde_json::from_str(line.trim()) {
            Ok(r) => r,
            Err(e) => {
                let err = ServerPush::Error {
                    message: format!("bad request: {e}"),
                };
                write_response(&mut writer, &err).await?;
                continue;
            }
        };
        let response = {
            let mut g = shared.inner.lock().await;
            g.handle_request(req).await
        };
        write_response(&mut writer, &response).await?;
    }
    Ok(())
}

async fn write_response<W: AsyncWriteExt + Unpin>(
    writer: &mut W,
    push: &ServerPush,
) -> Result<()> {
    let mut raw = serde_json::to_string(push)?;
    raw.push('\n');
    writer.write_all(raw.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}
