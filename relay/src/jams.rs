//! Jams open on the relay: host connection, pending requests, latest state.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, oneshot, watch};

use crate::limits::{code_matches, random_token, same};
use crate::protocol::ToHost;

/// Failed code attempts per address over `FAILURE_WINDOW` before it is blocked for
/// this Jam; the block lifts once the oldest ones leave the window (#256: on mobile
/// data, several guests share an address).
pub const MAX_FAILURES: usize = 10;
pub const FAILURE_WINDOW: Duration = Duration::from_secs(15 * 60);
/// Most addresses tracked for a Jam's failed attempts.
const MAX_FAILING_ADDRESSES: usize = 10_000;
/// Host answer awaited for this long at most.
pub const HOST_TIMEOUT: Duration = Duration::from_secs(15);
/// Messages waiting for a host (guest requests).
const HOST_QUEUE: usize = 64;
/// Requests awaiting a host answer, per Jam.
const MAX_PENDING: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub body: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ForwardError {
    /// Host disconnected (mobile network cut): it may come back.
    Offline,
    /// No answer in time, or too many pending requests.
    Busy,
}

/// Why a Jam could not be created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateError {
    /// The relay already has all its Jams open.
    Full,
    /// This address already has all of its own (#257).
    TooManyForAddress,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CodeError {
    Wrong,
    Blocked,
}

struct Inner {
    code: String,
    /// Host address, updated on each resume (Wi-Fi then mobile network): counts towards
    /// the per-address Jam cap (#257).
    host_ip: IpAddr,
    host: Option<mpsc::Sender<ToHost>>,
    /// Number of the attached host connection: an old connection closing after the
    /// resume does not detach the new one.
    host_gen: u64,
    disconnected_at: Option<Instant>,
    pending: HashMap<u64, oneshot::Sender<Reply>>,
    next_rid: u64,
    /// Latest state; `None` once the Jam is closed (open streams end).
    state: Option<watch::Sender<Arc<str>>>,
    failures: HashMap<IpAddr, VecDeque<Instant>>,
}

pub struct Jam {
    pub id: String,
    secret: String,
    created: Instant,
    inner: Mutex<Inner>,
    streams: AtomicUsize,
}

/// Slot of an open page (state stream), given back when the stream closes.
pub struct StreamSlot(Arc<Jam>);

impl Drop for StreamSlot {
    fn drop(&mut self) {
        self.0.streams.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Jam {
    /// Reserves the slot of an open page, unless `max` already are.
    pub fn open_stream(self: &Arc<Self>, max: usize) -> Option<StreamSlot> {
        let mut current = self.streams.load(Ordering::Relaxed);
        loop {
            if current >= max {
                return None;
            }
            match self.streams.compare_exchange_weak(
                current,
                current + 1,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Some(StreamSlot(Arc::clone(self))),
                Err(actual) => current = actual,
            }
        }
    }

    fn new(code: String, host_ip: IpAddr) -> Self {
        let (state, _) = watch::channel(Arc::<str>::from(""));
        Self {
            id: random_token(),
            secret: random_token(),
            created: Instant::now(),
            inner: Mutex::new(Inner {
                code,
                host_ip,
                host: None,
                host_gen: 0,
                disconnected_at: Some(Instant::now()),
                pending: HashMap::new(),
                next_rid: 0,
                state: Some(state),
                failures: HashMap::new(),
            }),
            streams: AtomicUsize::new(0),
        }
    }

    pub fn secret(&self) -> &str {
        &self.secret
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // No code panics under this lock: a poisoned lock stays usable.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Attaches a host connection (new or resumed): returns the queue of its messages
    /// and its number. A connection still attached is replaced (#253): its sender
    /// goes away, which ends it, and its pending requests fail right away
    /// (the guest page retries) instead of waiting for an answer that will not come.
    pub fn attach(&self, code: String, host_ip: IpAddr) -> (mpsc::Receiver<ToHost>, u64) {
        let (tx, rx) = mpsc::channel(HOST_QUEUE);
        let mut inner = self.lock();
        inner.code = code;
        inner.host_ip = host_ip;
        inner.pending.clear();
        inner.host = Some(tx);
        inner.host_gen += 1;
        inner.disconnected_at = None;
        (rx, inner.host_gen)
    }

    /// Connection `generation` is closed: pending requests fail.
    pub fn detach(&self, generation: u64) {
        let mut inner = self.lock();
        if inner.host_gen != generation {
            return;
        }
        inner.host = None;
        inner.disconnected_at = Some(Instant::now());
        inner.pending.clear();
    }

    /// Forwards a guest's call to the host and waits for its answer.
    pub async fn forward(
        &self,
        client: IpAddr,
        path: String,
        code: Option<String>,
        body: String,
    ) -> Result<Reply, ForwardError> {
        let (host, rid, rx) = {
            let mut inner = self.lock();
            let host = inner.host.clone().ok_or(ForwardError::Offline)?;
            if inner.pending.len() >= MAX_PENDING {
                return Err(ForwardError::Busy);
            }
            inner.next_rid += 1;
            let rid = inner.next_rid;
            let (tx, rx) = oneshot::channel();
            inner.pending.insert(rid, tx);
            (host, rid, rx)
        };
        let request = ToHost::Request {
            rid,
            client: client.to_string(),
            path,
            code,
            body,
        };
        let result = async {
            host.send(request)
                .await
                .map_err(|_| ForwardError::Offline)?;
            match tokio::time::timeout(HOST_TIMEOUT, rx).await {
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

    pub fn push_state(&self, data: String) {
        if let Some(state) = &self.lock().state {
            state.send_if_modified(|current| {
                if **current == *data {
                    return false;
                }
                *current = Arc::from(data);
                true
            });
        }
    }

    /// State stream for a page; `None` if the Jam is closed.
    pub fn subscribe(&self) -> Option<watch::Receiver<Arc<str>>> {
        self.lock().state.as_ref().map(watch::Sender::subscribe)
    }

    /// Checks the code of a state stream (API calls are checked by the host).
    pub fn check_code(&self, ip: IpAddr, given: &str) -> Result<(), CodeError> {
        self.check_code_at(ip, given, Instant::now())
    }

    fn check_code_at(&self, ip: IpAddr, given: &str, now: Instant) -> Result<(), CodeError> {
        let mut inner = self.lock();
        let recent = |t: &Instant| now.duration_since(*t) < FAILURE_WINDOW;
        if let Some(times) = inner.failures.get_mut(&ip) {
            times.retain(recent);
            if times.len() >= MAX_FAILURES {
                return Err(CodeError::Blocked);
            }
        }
        if code_matches(&inner.code, given) {
            return Ok(());
        }
        if inner.failures.len() >= MAX_FAILING_ADDRESSES && !inner.failures.contains_key(&ip) {
            inner
                .failures
                .retain(|_, times| times.back().is_some_and(recent));
        }
        if inner.failures.len() < MAX_FAILING_ADDRESSES || inner.failures.contains_key(&ip) {
            inner.failures.entry(ip).or_default().push_back(now);
        }
        Err(CodeError::Wrong)
    }

    fn host_ip(&self) -> IpAddr {
        self.lock().host_ip
    }

    pub fn host_online(&self) -> bool {
        self.lock().host.is_some()
    }

    /// Closes the Jam: the host is notified, open streams and pending requests end.
    fn close(&self, reason: &str) {
        let mut inner = self.lock();
        if let Some(host) = inner.host.take() {
            let _ = host.try_send(ToHost::Error {
                message: reason.to_owned(),
            });
        }
        inner.pending.clear();
        inner.state = None;
    }

    fn expired(&self, now: Instant, lifetime: Duration, grace: Duration) -> bool {
        if now.duration_since(self.created) >= lifetime {
            return true;
        }
        self.lock()
            .disconnected_at
            .is_some_and(|t| now.duration_since(t) >= grace)
    }
}

#[derive(Default)]
pub struct Registry {
    jams: Mutex<HashMap<String, Arc<Jam>>>,
}

impl Registry {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<Jam>>> {
        self.jams.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get(&self, id: &str) -> Option<Arc<Jam>> {
        self.lock().get(id).cloned()
    }

    /// Jam to resume: with the right secret only. A host connection still attached
    /// does not prevent the resume (#253): when the phone changes network, it comes
    /// back well before the relay has seen the old connection die (up to 80 s), and the
    /// secret proves it is the same host.
    pub fn resume(&self, id: &str, secret: &str) -> Option<Arc<Jam>> {
        let jam = self.get(id)?;
        same(&jam.secret, secret).then_some(jam)
    }

    /// New Jam, unless `max` are already open on the relay or `max_per_address`
    /// for address `host_ip` (#257: a single address does not fill the relay).
    pub fn create(
        &self,
        code: String,
        host_ip: IpAddr,
        max: usize,
        max_per_address: usize,
    ) -> Result<Arc<Jam>, CreateError> {
        let mut jams = self.lock();
        if jams.len() >= max {
            return Err(CreateError::Full);
        }
        let same_address = jams.values().filter(|j| j.host_ip() == host_ip).count();
        if same_address >= max_per_address {
            return Err(CreateError::TooManyForAddress);
        }
        let jam = Arc::new(Jam::new(code, host_ip));
        jams.insert(jam.id.clone(), Arc::clone(&jam));
        Ok(jam)
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// Closes Jams that are too old or whose host did not come back in time.
    pub fn sweep(&self, now: Instant, lifetime: Duration, grace: Duration) {
        let expired: Vec<Arc<Jam>> = {
            let mut jams = self.lock();
            let ids: Vec<String> = jams
                .iter()
                .filter(|(_, jam)| jam.expired(now, lifetime, grace))
                .map(|(id, _)| id.clone())
                .collect();
            ids.iter().filter_map(|id| jams.remove(id)).collect()
        };
        for jam in expired {
            tracing::info!("Jam closed ({} still open)", self.len());
            jam.close("Jam expirée");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
    const OTHER: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3));

    #[test]
    fn registry_limits_and_resume() {
        let registry = Registry::default();
        let jam = registry.create("AB23CD".into(), IP, 1, 3).unwrap();
        assert_eq!(
            registry.create("X".into(), IP, 1, 3).err(),
            Some(CreateError::Full)
        );
        // Resume with the right secret only, whether or not a host is attached (#253).
        assert!(registry.resume(&jam.id, "faux").is_none());
        assert!(registry.resume(&jam.id, jam.secret()).is_some());
        let (_rx, generation) = jam.attach("AB23CD".into(), IP);
        assert!(registry.resume(&jam.id, "faux").is_none());
        assert!(registry.resume(&jam.id, jam.secret()).is_some());
        jam.detach(generation + 1);
        assert!(jam.host_online());
        jam.detach(generation);
        assert!(!jam.host_online());
    }

    #[tokio::test]
    async fn a_new_host_connection_replaces_the_old_one() {
        let registry = Registry::default();
        let jam = registry.create("AB23CD".into(), IP, 5, 3).unwrap();
        let (mut old, old_gen) = jam.attach("AB23CD".into(), IP);
        // A request is waiting on the old connection, which will no longer answer.
        let waiting = {
            let jam = Arc::clone(&jam);
            tokio::spawn(async move {
                jam.forward(IP, "/api/join".into(), None, String::new())
                    .await
            })
        };
        assert!(matches!(old.recv().await, Some(ToHost::Request { .. })));

        let (mut new, new_gen) = jam.attach("AB23CD".into(), OTHER);
        // It fails right away, and the old connection sees its queue close.
        assert_eq!(waiting.await.unwrap(), Err(ForwardError::Offline));
        assert!(old.recv().await.is_none());
        // The old connection closing does not detach the new one.
        jam.detach(old_gen);
        assert!(jam.host_online());
        let host = Arc::clone(&jam);
        tokio::spawn(async move {
            if let Some(ToHost::Request { rid, .. }) = new.recv().await {
                let body = "ok".into();
                host.respond(rid, Reply { status: 200, body });
            }
        });
        let reply = jam
            .forward(IP, "/api/join".into(), None, String::new())
            .await
            .unwrap();
        assert_eq!(reply.body, "ok");
        jam.detach(new_gen);
        assert!(!jam.host_online());
    }

    #[test]
    fn jams_are_capped_per_host_address() {
        let registry = Registry::default();
        let first = registry.create("A".into(), IP, 10, 2).unwrap();
        let _second = registry.create("B".into(), IP, 10, 2).unwrap();
        assert_eq!(
            registry.create("C".into(), IP, 10, 2).err(),
            Some(CreateError::TooManyForAddress)
        );
        // Another address keeps its slots.
        assert!(registry.create("C".into(), OTHER, 10, 2).is_ok());
        // The host coming back from another address (resume) frees a slot of the old one.
        let _rx = first.attach("A".into(), OTHER);
        assert!(registry.create("D".into(), IP, 10, 2).is_ok());
        assert_eq!(
            registry.create("E".into(), OTHER, 10, 2).err(),
            Some(CreateError::TooManyForAddress)
        );
    }

    #[test]
    fn sweep_closes_abandoned_jams() {
        let registry = Registry::default();
        let jam = registry.create("AB23CD".into(), IP, 5, 3).unwrap();
        let (_rx, _) = jam.attach("AB23CD".into(), IP);
        let events = jam.subscribe().unwrap();
        let grace = Duration::from_secs(120);
        let life = Duration::from_secs(3600);
        registry.sweep(Instant::now() + Duration::from_secs(10), life, grace);
        assert_eq!(registry.len(), 1);
        registry.sweep(Instant::now() + life, life, grace);
        assert_eq!(registry.len(), 0);
        assert!(events.has_changed().is_err());
    }

    #[test]
    fn codes_are_checked_and_guessing_blocked() {
        let registry = Registry::default();
        let jam = registry.create("AB23CD".into(), IP, 5, 3).unwrap();
        let t0 = Instant::now();
        assert_eq!(jam.check_code_at(IP, "ab23cd", t0), Ok(()));
        for _ in 0..MAX_FAILURES {
            assert_eq!(jam.check_code_at(IP, "nope", t0), Err(CodeError::Wrong));
        }
        assert_eq!(jam.check_code_at(IP, "AB23CD", t0), Err(CodeError::Blocked));
        // The block lifts at the end of the window (#256).
        assert_eq!(jam.check_code_at(IP, "AB23CD", t0 + FAILURE_WINDOW), Ok(()));
    }

    #[tokio::test]
    async fn requests_reach_the_host_and_come_back() {
        let registry = Registry::default();
        let jam = registry.create("AB23CD".into(), IP, 5, 3).unwrap();
        assert_eq!(
            jam.forward(IP, "/api/join".into(), None, String::new())
                .await,
            Err(ForwardError::Offline)
        );
        let (mut rx, _) = jam.attach("AB23CD".into(), IP);
        let host = Arc::clone(&jam);
        tokio::spawn(async move {
            if let Some(ToHost::Request { rid, client, .. }) = rx.recv().await {
                host.respond(
                    rid,
                    Reply {
                        status: 200,
                        body: client,
                    },
                );
            }
        });
        let reply = jam
            .forward(IP, "/api/join".into(), Some("AB23CD".into()), String::new())
            .await
            .unwrap();
        assert_eq!(reply.body, "10.0.0.2");
    }
}
