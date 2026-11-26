//! Discord's local IPC: named pipe `\\.\pipe\discord-ipc-<n>` on Windows, Unix socket elsewhere.
//! Frames are `opcode (u32 LE) + length (u32 LE) + JSON`.

use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, WriteHalf};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;
const OP_PING: u32 = 3;
const OP_PONG: u32 = 4;

/// Discord only sends small replies; beyond this, the connection is considered invalid.
const MAX_FRAME: usize = 64 * 1024;
const READY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    #[error("Discord unreachable: {0}")]
    Io(#[from] std::io::Error),
    #[error("Discord refused the connection: {0}")]
    Refused(String),
    #[error("Discord did not answer")]
    Timeout,
    #[error("connection to Discord closed")]
    Closed,
}

pub struct Frame {
    pub op: u32,
    pub payload: Value,
}

/// Connection established (READY received). Reading runs in a separate task, stopped with the
/// connection: the pipe is only closed once both halves are released, and Discord then clears the
/// presence.
pub struct Connection<S> {
    writer: WriteHalf<S>,
    frames: mpsc::Receiver<Frame>,
    reader: JoinHandle<()>,
    nonce: u64,
}

impl<S> Drop for Connection<S> {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

#[cfg(windows)]
pub type Pipe = tokio::net::windows::named_pipe::NamedPipeClient;
#[cfg(unix)]
pub type Pipe = tokio::net::UnixStream;

/// First Discord pipe that answers (0 to 9: several clients may be running).
pub async fn open(client_id: &str) -> Result<Connection<Pipe>, IpcError> {
    let mut last = IpcError::Closed;
    for i in 0..10 {
        match open_pipe(i).await {
            Ok(pipe) => return Connection::handshake(pipe, client_id).await,
            Err(e) => last = IpcError::Io(e),
        }
    }
    Err(last)
}

#[cfg(windows)]
async fn open_pipe(i: u8) -> std::io::Result<Pipe> {
    tokio::net::windows::named_pipe::ClientOptions::new().open(format!(r"\\.\pipe\discord-ipc-{i}"))
}

#[cfg(unix)]
async fn open_pipe(i: u8) -> std::io::Result<Pipe> {
    let dir = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "/tmp".to_owned());
    Pipe::connect(format!("{dir}/discord-ipc-{i}")).await
}

impl<S: AsyncRead + AsyncWrite + Send + 'static> Connection<S> {
    pub async fn handshake(stream: S, client_id: &str) -> Result<Self, IpcError> {
        let (mut read, writer) = tokio::io::split(stream);
        let (tx, frames) = mpsc::channel(16);
        let reader = tokio::spawn(async move {
            while let Ok(frame) = read_frame(&mut read).await {
                if tx.send(frame).await.is_err() {
                    break;
                }
            }
        });
        let mut conn = Self {
            writer,
            frames,
            reader,
            nonce: 0,
        };
        conn.write(OP_HANDSHAKE, &json!({ "v": 1, "client_id": client_id }))
            .await?;
        let first = tokio::time::timeout(READY_TIMEOUT, conn.frames.recv())
            .await
            .map_err(|_| IpcError::Timeout)?
            .ok_or(IpcError::Closed)?;
        match (first.op, first.payload["evt"].as_str()) {
            (OP_FRAME, Some("READY")) => Ok(conn),
            _ => Err(IpcError::Refused(message(&first.payload))),
        }
    }

    /// Replaces the activity (`None` clears it).
    pub async fn set_activity(&mut self, activity: Option<Value>) -> Result<(), IpcError> {
        self.nonce += 1;
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": std::process::id(), "activity": activity },
            "nonce": self.nonce.to_string(),
        });
        self.write(OP_FRAME, &payload).await
    }

    /// Next frame from Discord (safe in a `select!`); `None`: connection lost.
    pub async fn recv(&mut self) -> Option<Frame> {
        self.frames.recv().await
    }

    /// `PING` gets its `PONG`, an error is logged. `Err`: connection closed by Discord or lost.
    pub async fn handle(&mut self, frame: Frame) -> Result<(), IpcError> {
        match frame.op {
            OP_PING => self.write(OP_PONG, &frame.payload).await,
            OP_CLOSE => Err(IpcError::Refused(message(&frame.payload))),
            _ => {
                if frame.payload["evt"] == "ERROR" {
                    tracing::warn!("Discord : {}", message(&frame.payload));
                }
                Ok(())
            }
        }
    }

    async fn write(&mut self, op: u32, payload: &Value) -> Result<(), IpcError> {
        self.writer.write_all(&encode(op, payload)).await?;
        self.writer.flush().await?;
        Ok(())
    }
}

fn message(payload: &Value) -> String {
    payload["data"]["message"]
        .as_str()
        .or_else(|| payload["message"].as_str())
        .unwrap_or("unexpected reply")
        .to_owned()
}

fn encode(op: u32, payload: &Value) -> Vec<u8> {
    let body = payload.to_string().into_bytes();
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    out
}

async fn read_frame<R: AsyncRead + Unpin>(read: &mut R) -> std::io::Result<Frame> {
    let mut header = [0u8; 8];
    read.read_exact(&mut header).await?;
    let op = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
    if len > MAX_FRAME {
        return Err(std::io::Error::other("trame trop longue"));
    }
    let mut body = vec![0u8; len];
    read.read_exact(&mut body).await?;
    let payload = serde_json::from_slice(&body).map_err(std::io::Error::other)?;
    Ok(Frame { op, payload })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::DuplexStream;

    /// Fake Discord: reads a frame and sends it back, answers with `reply`.
    async fn serve(server: &mut DuplexStream, reply: Option<(u32, Value)>) -> Frame {
        let frame = read_frame(server).await.unwrap();
        if let Some((op, payload)) = reply {
            server.write_all(&encode(op, &payload)).await.unwrap();
        }
        frame
    }

    #[tokio::test]
    async fn handshake_then_activity() {
        let (client, mut server) = tokio::io::duplex(4096);
        let fake = tokio::spawn(async move {
            let hello = serve(
                &mut server,
                Some((OP_FRAME, json!({ "cmd": "DISPATCH", "evt": "READY" }))),
            )
            .await;
            let set = serve(&mut server, Some((OP_PING, json!({ "x": 1 })))).await;
            let pong = serve(&mut server, None).await;
            (hello, set, pong)
        });
        let mut conn = Connection::handshake(client, "123").await.unwrap();
        conn.set_activity(Some(json!({ "type": 2 }))).await.unwrap();
        let ping = conn.recv().await.unwrap();
        conn.handle(ping).await.unwrap();
        let (hello, set, pong) = fake.await.unwrap();
        assert_eq!(hello.op, OP_HANDSHAKE);
        assert_eq!(hello.payload, json!({ "v": 1, "client_id": "123" }));
        assert_eq!(set.op, OP_FRAME);
        assert_eq!(set.payload["cmd"], "SET_ACTIVITY");
        assert_eq!(set.payload["args"]["activity"], json!({ "type": 2 }));
        assert_eq!(set.payload["args"]["pid"], std::process::id());
        assert_eq!(pong.op, OP_PONG);
        assert_eq!(pong.payload, json!({ "x": 1 }));
    }

    #[tokio::test]
    async fn refused_client_id() {
        let (client, mut server) = tokio::io::duplex(4096);
        tokio::spawn(async move {
            serve(
                &mut server,
                Some((
                    OP_CLOSE,
                    json!({ "code": 4000, "message": "Invalid Client ID" }),
                )),
            )
            .await;
        });
        match Connection::handshake(client, "0").await {
            Err(IpcError::Refused(m)) => assert_eq!(m, "Invalid Client ID"),
            _ => panic!("refus attendu"),
        }
    }

    #[tokio::test]
    async fn closed_after_ready() {
        let (client, mut server) = tokio::io::duplex(4096);
        let fake = tokio::spawn(async move {
            serve(&mut server, Some((OP_FRAME, json!({ "evt": "READY" })))).await;
            drop(server);
        });
        let mut conn = Connection::handshake(client, "1").await.unwrap();
        fake.await.unwrap();
        assert!(conn.recv().await.is_none());
    }
}
