use anyhow::Result;
use plugin_api::{Envelope, Kind, Metadata};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use uuid::Uuid;

/// Abstraction over the communication bridge to the core.
pub trait CoreBridge: Send + Sync {
    fn emit(&self, _event: &str) {}
}

/// A no-op bridge used when running the server standalone or in tests.
#[derive(Clone, Default)]
pub struct NullCoreBridge;

impl CoreBridge for NullCoreBridge {}

/// Run the stdio protocol handshake with the core and then start the HTTP server.
pub async fn run_stdio(bind: &str) -> Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut writer = BufWriter::new(stdout);

    // wait for core.hello
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let _hello: Envelope = serde_json::from_str(line.trim())?;

    // send plugin.init
    let init = Envelope {
        id: Some(Uuid::new_v4().to_string()),
        kind: Kind::Request,
        method: Some("plugin.init".into()),
        params: Some(json!({
            "metadata": Metadata {
                id: "family_chat".into(),
                name: "Family Chat".into(),
                version: "0.1.0".into(),
                needs: vec!["log".into(), "event".into(), "timer".into(), "storage".into()],
            }
        })),
        result: None,
        error: None,
        topic: None,
        payload: None,
    };
    send(&mut writer, &init).await?;
    let _ = read(&mut reader).await?; // response

    // send plugin.start
    let start = Envelope {
        id: Some(Uuid::new_v4().to_string()),
        kind: Kind::Request,
        method: Some("plugin.start".into()),
        params: Some(json!({})),
        result: None,
        error: None,
        topic: None,
        payload: None,
    };
    send(&mut writer, &start).await?;
    let _ = read(&mut reader).await?; // response

    // spawn HTTP server
    let bind = bind.to_string();
    tokio::spawn(async move {
        let _ = crate::api::run_http_server(bind).await;
    });

    // event loop; respond to plugin.stop and then exit
    while let Ok(env) = read(&mut reader).await {
        if env.kind == Kind::Request && env.method.as_deref() == Some("plugin.stop") {
            let resp = Envelope {
                id: env.id.clone(),
                kind: Kind::Response,
                method: None,
                params: None,
                result: Some(json!({})),
                error: None,
                topic: None,
                payload: None,
            };
            let _ = send(&mut writer, &resp).await;
            break;
        }
    }
    Ok(())
}

async fn send<W: AsyncWriteExt + Unpin>(w: &mut W, env: &Envelope) -> Result<()> {
    let s = serde_json::to_string(env)?;
    w.write_all(s.as_bytes()).await?;
    w.write_all(b"\n").await?;
    w.flush().await?;
    Ok(())
}

async fn read<R: AsyncBufReadExt + Unpin>(r: &mut R) -> Result<Envelope> {
    let mut line = String::new();
    r.read_line(&mut line).await?;
    if line.is_empty() {
        anyhow::bail!("eof")
    }
    let env = serde_json::from_str(line.trim())?;
    Ok(env)
}
