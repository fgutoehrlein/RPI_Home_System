use anyhow::Result;
use plugin_api::Envelope;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

/// Read a single line-delimited JSON envelope from the reader.
pub async fn read_envelope<R: AsyncBufRead + Unpin>(reader: &mut R) -> Result<Envelope> {
    let mut line = String::new();
    let n = reader.read_line(&mut line).await?;
    if n == 0 {
        anyhow::bail!("plugin closed pipe");
    }
    let env = serde_json::from_str(line.trim())?;
    Ok(env)
}

/// Write a single envelope as line-delimited JSON to the writer.
pub async fn write_envelope<W: AsyncWrite + Unpin>(writer: &mut W, env: &Envelope) -> Result<()> {
    let s = serde_json::to_string(env)?;
    writer.write_all(s.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}
