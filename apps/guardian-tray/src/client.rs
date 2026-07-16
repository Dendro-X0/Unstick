use anyhow::{bail, Context, Result};
use guardian_core::{ClientRequest, ServerPush, PIPE_NAME};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ClientOptions;

pub async fn request(req: ClientRequest) -> Result<ServerPush> {
    let client = ClientOptions::new()
        .open(PIPE_NAME)
        .context("open named pipe")?;
    let (reader_half, mut writer) = tokio::io::split(client);
    let mut raw = serde_json::to_string(&req)?;
    raw.push('\n');
    writer.write_all(raw.as_bytes()).await?;
    writer.flush().await?;

    let mut reader = BufReader::new(reader_half);
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        bail!("empty ipc response");
    }
    let push: ServerPush = serde_json::from_str(line.trim())?;
    Ok(push)
}
