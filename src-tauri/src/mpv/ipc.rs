//! mpv's JSON IPC client. A single task owns the pipe: it writes the commands, matches
//! the replies by `request_id` and forwards the events.

use std::collections::HashMap;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
#[cfg(unix)]
use tokio::net::UnixStream as Pipe;
#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient as Pipe};
use tokio::sync::{mpsc, oneshot};

use super::events::{self, MpvEvent, OBSERVED};
use super::process::pipe_name;
use super::{MpvError, Result};

const CONNECT_ATTEMPTS: u32 = 50;
const CONNECT_DELAY: Duration = Duration::from_millis(100);

struct Request {
    /// Array of arguments, or object with named arguments (`{"name": "loadfile", …}`).
    command: Value,
    reply: oneshot::Sender<Result<Value>>,
}

#[derive(Clone)]
pub struct MpvHandle {
    tx: mpsc::Sender<Request>,
}

impl MpvHandle {
    /// Sends a command (`["set_property", "pause", true]`…, or an object with named
    /// arguments) and waits for mpv's reply.
    pub async fn command(&self, command: impl Into<Value>) -> Result<Value> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Request {
                command: command.into(),
                reply,
            })
            .await
            .map_err(|_| MpvError::Closed)?;
        rx.await.map_err(|_| MpvError::Closed)?
    }

    /// Frozen mpv: receives the commands without ever replying (timeout tests, #255).
    #[cfg(test)]
    pub fn unresponsive() -> Self {
        let (tx, mut rx) = mpsc::channel::<Request>(32);
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Some(req) = rx.recv().await {
                held.push(req);
            }
        });
        Self { tx }
    }
}

/// Connects to mpv's pipe (which mpv takes a moment to create), then observes the properties.
pub async fn connect(events_tx: mpsc::Sender<MpvEvent>) -> Result<MpvHandle> {
    let pipe = open_pipe().await?;
    let (tx, rx) = mpsc::channel(32);
    tokio::spawn(io_task(pipe, rx, events_tx));
    let handle = MpvHandle { tx };
    for (id, name) in (1u64..).zip(OBSERVED) {
        handle
            .command(vec![json!("observe_property"), json!(id), json!(name)])
            .await?;
    }
    Ok(handle)
}

/// Named pipe on Windows; Unix socket elsewhere (Linux CI).
#[cfg(windows)]
async fn open(name: &str) -> std::io::Result<Pipe> {
    ClientOptions::new().open(name)
}

#[cfg(unix)]
async fn open(name: &str) -> std::io::Result<Pipe> {
    Pipe::connect(name).await
}

async fn open_pipe() -> Result<Pipe> {
    let name = pipe_name();
    let mut last_err = None;
    for _ in 0..CONNECT_ATTEMPTS {
        match open(&name).await {
            Ok(pipe) => return Ok(pipe),
            Err(e) => last_err = Some(e),
        }
        tokio::time::sleep(CONNECT_DELAY).await;
    }
    Err(MpvError::Connect(last_err.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::TimedOut, "timed out")
    })))
}

async fn io_task(
    pipe: Pipe,
    mut requests: mpsc::Receiver<Request>,
    events_tx: mpsc::Sender<MpvEvent>,
) {
    let (reader, mut writer) = tokio::io::split(pipe);
    let mut lines = BufReader::new(reader).lines();
    let mut pending: HashMap<u64, oneshot::Sender<Result<Value>>> = HashMap::new();
    let mut next_id: u64 = 1;

    loop {
        tokio::select! {
            req = requests.recv() => {
                let Some(req) = req else { break };
                let id = next_id;
                next_id += 1;
                let mut line = json!({ "command": req.command, "request_id": id }).to_string();
                line.push('\n');
                if writer.write_all(line.as_bytes()).await.is_err() {
                    let _ = req.reply.send(Err(MpvError::Closed));
                    break;
                }
                pending.insert(id, req.reply);
            }
            line = lines.next_line() => {
                let Ok(Some(line)) = line else { break };
                let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                    tracing::warn!("message mpv illisible");
                    continue;
                };
                if let Some(ev) = events::parse(&msg) {
                    if events_tx.send(ev).await.is_err() {
                        break;
                    }
                } else if let Some(id) = msg.get("request_id").and_then(Value::as_u64)
                    && let Some(reply) = pending.remove(&id)
                {
                    let _ = reply.send(reply_result(msg));
                }
            }
        }
    }
    for (_, reply) in pending.drain() {
        let _ = reply.send(Err(MpvError::Closed));
    }
    let _ = events_tx.send(MpvEvent::Disconnected).await;
}

fn reply_result(mut msg: Value) -> Result<Value> {
    match msg.get("error").and_then(Value::as_str) {
        Some("success") => Ok(msg.get_mut("data").map(Value::take).unwrap_or(Value::Null)),
        Some(err) => Err(MpvError::Command(err.to_owned())),
        None => Err(MpvError::Command("reply without a status".into())),
    }
}
