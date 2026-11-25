//! Device tunnels (#430): a user's computer stays reachable by its paired phone outside
//! the local network, for playback handoff and listening history.
//!
//! Unlike a Jam, the relay creates and keeps nothing: the PC draws its own identifier and
//! secret, and the tunnel only exists while it is connected. The relay only forwards
//! end-to-end encrypted packets (key known to both devices only): it reads nothing of them.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{Notify, mpsc, oneshot};

use crate::jams::{ForwardError, Reply};
use crate::limits::same;
use crate::protocol::ToDevice;

/// Longest wait for the PC's answer (a history batch is about 1 MB).
pub const DEVICE_TIMEOUT: Duration = Duration::from_secs(30);
const QUEUE: usize = 16;
const MAX_PENDING: usize = 16;

/// Valid identifier or secret: 32 to 64 hexadecimal characters.
pub fn is_token(s: &str) -> bool {
    (32..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachError {
    /// Identifier already connected with another secret.
    Taken,
    /// The relay already has all its tunnels.
    Full,
    /// This address already has all of its own.
    TooManyForAddress,
}

struct Inner {
    host: mpsc::Sender<ToDevice>,
    generation: u64,
    pending: HashMap<u64, oneshot::Sender<Reply>>,
    next_rid: u64,
}

pub struct Device {
    secret: String,
    ip: IpAddr,
    inner: Mutex<Inner>,
    /// A new connection of the same PC took the place: the old one closes.
    pub replaced: Notify,
}

impl Device {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// True while a request from the phone awaits the PC's answer: the only time the relay
    /// reads the PC's connection (`device::run`). `forward` registers the request before
    /// handing it to the connection, which therefore sees it on its next turn.
    pub fn awaits_answer(&self) -> bool {
        !self.lock().pending.is_empty()
    }

    /// Forwards a packet from the phone to the PC and waits for its answer.
    pub async fn forward(&self, client: IpAddr, body: String) -> Result<Reply, ForwardError> {
        let (host, rid, rx) = {
            let mut inner = self.lock();
            if inner.pending.len() >= MAX_PENDING {
                return Err(ForwardError::Busy);
            }
            inner.next_rid += 1;
            let rid = inner.next_rid;
            let (tx, rx) = oneshot::channel();
            inner.pending.insert(rid, tx);
            (inner.host.clone(), rid, rx)
        };
        let request = ToDevice::Request {
            rid,
            client: client.to_string(),
            body,
        };
        let result = async {
            host.send(request)
                .await
                .map_err(|_| ForwardError::Offline)?;
            match tokio::time::timeout(DEVICE_TIMEOUT, rx).await {
                Ok(Ok(reply)) => Ok(reply),
                Ok(Err(_)) => Err(ForwardError::Offline),
                Err(_) => Err(ForwardError::Busy),
            }
        }
        .await;
        self.lock().pending.remove(&rid);
        result
    }

    pub fn respond(&self, rid: u64, reply: Reply) {
        if let Some(tx) = self.lock().pending.remove(&rid) {
            let _ = tx.send(reply);
        }
    }
}

#[derive(Default)]
pub struct Devices {
    devices: Mutex<HashMap<String, Arc<Device>>>,
    next_generation: Mutex<u64>,
}

impl Devices {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<Device>>> {
        self.devices.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get(&self, id: &str) -> Option<Arc<Device>> {
        self.lock().get(id).cloned()
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// Attaches a PC's connection: returns the queue of its messages and its number. A
    /// connection of the same PC still attached (same secret) is replaced: it often comes
    /// back before the relay has seen the old one die.
    pub fn attach(
        &self,
        id: &str,
        secret: &str,
        ip: IpAddr,
        max: usize,
        max_per_address: usize,
    ) -> Result<(Arc<Device>, mpsc::Receiver<ToDevice>, u64), AttachError> {
        let generation = {
            let mut next = self
                .next_generation
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *next += 1;
            *next
        };
        let mut devices = self.lock();
        let previous = match devices.get(id) {
            Some(device) if !same(&device.secret, secret) => return Err(AttachError::Taken),
            previous => previous.cloned(),
        };
        if previous.is_none() {
            if devices.len() >= max {
                return Err(AttachError::Full);
            }
            if devices.values().filter(|d| d.ip == ip).count() >= max_per_address {
                return Err(AttachError::TooManyForAddress);
            }
        }
        let (tx, rx) = mpsc::channel(QUEUE);
        let device = Arc::new(Device {
            secret: secret.to_owned(),
            ip,
            inner: Mutex::new(Inner {
                host: tx,
                generation,
                pending: HashMap::new(),
                next_rid: 0,
            }),
            replaced: Notify::new(),
        });
        devices.insert(id.to_owned(), Arc::clone(&device));
        if let Some(previous) = previous {
            // Its pending requests fail; its session closes.
            previous.lock().pending.clear();
            previous.replaced.notify_one();
        }
        Ok((device, rx, generation))
    }

    /// Connection `generation` is closed: the tunnel disappears, unless a new connection
    /// has already taken it back.
    pub fn detach(&self, id: &str, generation: u64) {
        let mut devices = self.lock();
        if devices
            .get(id)
            .is_some_and(|d| d.lock().generation == generation)
        {
            devices.remove(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
    const OTHER: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3));
    const ID: &str = "0123456789abcdef0123456789abcdef";
    const SECRET: &str = "fedcba9876543210fedcba9876543210";

    #[test]
    fn tokens_are_long_hex() {
        assert!(is_token(ID));
        assert!(!is_token("0123"));
        assert!(!is_token(&"z".repeat(32)));
        assert!(!is_token(&"a".repeat(65)));
    }

    #[test]
    fn only_the_secret_reattaches() {
        let devices = Devices::default();
        let (_, _rx, first) = devices.attach(ID, SECRET, IP, 10, 3).unwrap();
        assert_eq!(
            devices.attach(ID, &"0".repeat(32), OTHER, 10, 3).err(),
            Some(AttachError::Taken)
        );
        let (_, _rx2, second) = devices.attach(ID, SECRET, OTHER, 10, 3).unwrap();
        // The old connection closing does not detach the new one.
        devices.detach(ID, first);
        assert!(devices.get(ID).is_some());
        devices.detach(ID, second);
        assert!(devices.is_empty());
    }

    #[test]
    fn limits_per_relay_and_per_address() {
        let devices = Devices::default();
        let id = |i: usize| format!("{i:032x}");
        let mut keep = Vec::new();
        for i in 0..2 {
            keep.push(devices.attach(&id(i), SECRET, IP, 3, 2).unwrap());
        }
        assert_eq!(
            devices.attach(&id(2), SECRET, IP, 3, 2).err(),
            Some(AttachError::TooManyForAddress)
        );
        keep.push(devices.attach(&id(3), SECRET, OTHER, 3, 2).unwrap());
        assert_eq!(
            devices.attach(&id(4), SECRET, OTHER, 3, 2).err(),
            Some(AttachError::Full)
        );
        assert_eq!(devices.len(), 3);
    }

    #[tokio::test]
    async fn requests_reach_the_device_and_come_back() {
        let devices = Devices::default();
        let (device, mut rx, _) = devices.attach(ID, SECRET, IP, 10, 3).unwrap();
        let answering = Arc::clone(&device);
        tokio::spawn(async move {
            if let Some(ToDevice::Request { rid, body, .. }) = rx.recv().await {
                // Registered before it reached the connection: no answer without it.
                if !answering.awaits_answer() {
                    return;
                }
                answering.respond(
                    rid,
                    Reply {
                        status: 200,
                        body: format!("re:{body}"),
                    },
                );
            }
        });
        assert!(!device.awaits_answer());
        let reply = device.forward(OTHER, "x".into()).await.unwrap();
        assert_eq!(reply.body, "re:x");
        assert!(!device.awaits_answer());
    }
}
