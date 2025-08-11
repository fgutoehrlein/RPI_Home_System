use anyhow::Result;
use clap::Parser;
use plugin_api::{Envelope, Kind, Metadata};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use uuid::Uuid;

#[derive(Parser)]
struct Opts {
    #[arg(long)]
    stdio: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let opts = Opts::parse();
    if opts.stdio {
        run_stdio().await?;
    } else {
        println!("sample_plugin --stdio");
    }
    Ok(())
}

async fn run_stdio() -> Result<()> {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut writer = BufWriter::new(stdout);

    // wait for core.hello
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    let hello: Envelope = serde_json::from_str(line.trim())?;
    assert_eq!(hello.kind, Kind::Event);

    // send plugin.init
    let init_id = Uuid::new_v4().to_string();
    let init = Envelope {
        id: Some(init_id.clone()),
        kind: Kind::Request,
        method: Some("plugin.init".into()),
        params: Some(json!({"metadata": Metadata{ id:"sample_plugin".into(), name:"Sample Plugin".into(), version:"0.1.0".into(), needs: vec!["log".into(),"event".into(),"timer".into(),"storage".into()] }})),
        result: None,
        error: None,
        topic: None,
        payload: None,
    };
    send(&mut writer, &init).await?;
    read(&mut reader).await?; // response

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
    read(&mut reader).await?; // response

    // subscribe to timer.tick
    let sub = Envelope {
        id: Some(Uuid::new_v4().to_string()),
        kind: Kind::Request,
        method: Some("event.subscribe".into()),
        params: Some(json!({"topics":["timer.tick"]})),
        result: None,
        error: None,
        topic: None,
        payload: None,
    };
    send(&mut writer, &sub).await?;
    read(&mut reader).await?;

    // set timer
    let timer = Envelope {
        id: Some(Uuid::new_v4().to_string()),
        kind: Kind::Request,
        method: Some("timer.set_interval".into()),
        params: Some(json!({"id":"sample","millis":1000})),
        result: None,
        error: None,
        topic: None,
        payload: None,
    };
    send(&mut writer, &timer).await?;
    read(&mut reader).await?;

    loop {
        let env = read(&mut reader).await?;
        match env.kind {
            Kind::Event => {
                if env.topic.as_deref() == Some("timer.tick") {
                    let req = Envelope {
                        id: Some(Uuid::new_v4().to_string()),
                        kind: Kind::Request,
                        method: Some("log.write".into()),
                        params: Some(json!({"level":"INFO","message":"tick from sample_plugin"})),
                        result: None,
                        error: None,
                        topic: None,
                        payload: None,
                    };
                    send(&mut writer, &req).await?;
                    read(&mut reader).await?; // ignore response
                }
            }
            Kind::Request => {
                if env.method.as_deref() == Some("sample.ping") {
                    let resp = Envelope {
                        id: env.id.clone(),
                        kind: Kind::Response,
                        method: None,
                        params: None,
                        result: env.params.clone(),
                        error: None,
                        topic: None,
                        payload: None,
                    };
                    send(&mut writer, &resp).await?;
                }
            }
            _ => {}
        }
    }
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
    if line.is_empty() { anyhow::bail!("eof") }
    let env = serde_json::from_str(line.trim())?;
    Ok(env)
}
