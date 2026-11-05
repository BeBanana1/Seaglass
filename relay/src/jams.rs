//! Jams ouvertes sur le relais : connexion de l'hôte, requêtes en attente, dernier état.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, oneshot, watch};

use crate::limits::{code_matches, random_token, same};
use crate::protocol::ToHost;

/// Essais de code ratés avant qu'une adresse soit bloquée pour cette Jam.
pub const MAX_FAILURES: u32 = 10;
/// Réponse de l'hôte attendue au plus ce délai.
pub const HOST_TIMEOUT: Duration = Duration::from_secs(15);
/// Messages en attente vers un hôte (requêtes des invités).
const HOST_QUEUE: usize = 64;
/// Requêtes en attente d'une réponse de l'hôte, par Jam.
const MAX_PENDING: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub body: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ForwardError {
    /// Hôte déconnecté (réseau mobile coupé) : il peut revenir.
    Offline,
    /// Pas de réponse à temps, ou trop de requêtes en attente.
    Busy,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CodeError {
    Wrong,
    Blocked,
}

struct Inner {
    code: String,
    host: Option<mpsc::Sender<ToHost>>,
    /// Numéro de la connexion de l'hôte attachée : une ancienne connexion qui se ferme
    /// après la reprise ne détache pas la nouvelle.
    host_gen: u64,
    disconnected_at: Option<Instant>,
    pending: HashMap<u64, oneshot::Sender<Reply>>,
    next_rid: u64,
    /// Dernier état ; `None` une fois la Jam fermée (les flux ouverts se terminent).
    state: Option<watch::Sender<Arc<str>>>,
    failures: HashMap<IpAddr, u32>,
}

pub struct Jam {
    pub id: String,
    secret: String,
    created: Instant,
    inner: Mutex<Inner>,
    streams: AtomicUsize,
}

/// Place d'une page ouverte (flux d'état), rendue à la fermeture du flux.
pub struct StreamSlot(Arc<Jam>);

impl Drop for StreamSlot {
    fn drop(&mut self) {
        self.0.streams.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Jam {
    /// Réserve la place d'une page ouverte, sauf si `max` le sont déjà.
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

    fn new(code: String) -> Self {
        let (state, _) = watch::channel(Arc::<str>::from(""));
        Self {
            id: random_token(),
            secret: random_token(),
            created: Instant::now(),
            inner: Mutex::new(Inner {
                code,
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
        // Aucun code ne panique sous ce verrou : un verrou empoisonné reste utilisable.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Branche une connexion d'hôte (nouvelle ou reprise) : renvoie la file de ses messages
    /// et son numéro.
    pub fn attach(&self, code: String) -> (mpsc::Receiver<ToHost>, u64) {
        let (tx, rx) = mpsc::channel(HOST_QUEUE);
        let mut inner = self.lock();
        inner.code = code;
        inner.host = Some(tx);
        inner.host_gen += 1;
        inner.disconnected_at = None;
        (rx, inner.host_gen)
    }

    /// La connexion `generation` est fermée : les requêtes en attente échouent.
    pub fn detach(&self, generation: u64) {
        let mut inner = self.lock();
        if inner.host_gen != generation {
            return;
        }
        inner.host = None;
        inner.disconnected_at = Some(Instant::now());
        inner.pending.clear();
    }

    /// Transmet l'appel d'un invité à l'hôte et attend sa réponse.
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

    /// Flux de l'état pour une page ; `None` si la Jam est fermée.
    pub fn subscribe(&self) -> Option<watch::Receiver<Arc<str>>> {
        self.lock().state.as_ref().map(watch::Sender::subscribe)
    }

    /// Vérifie le code d'un flux d'état (les appels de l'API sont vérifiés par l'hôte).
    pub fn check_code(&self, ip: IpAddr, given: &str) -> Result<(), CodeError> {
        let mut inner = self.lock();
        if inner.failures.get(&ip).is_some_and(|n| *n >= MAX_FAILURES) {
            return Err(CodeError::Blocked);
        }
        if code_matches(&inner.code, given) {
            return Ok(());
        }
        if inner.failures.len() < 10_000 || inner.failures.contains_key(&ip) {
            *inner.failures.entry(ip).or_default() += 1;
        }
        Err(CodeError::Wrong)
    }

    pub fn host_online(&self) -> bool {
        self.lock().host.is_some()
    }

    /// Ferme la Jam : l'hôte est prévenu, les flux et requêtes en attente se terminent.
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

    /// Jam à reprendre : seulement avec le bon secret et si aucun hôte n'y est branché.
    pub fn resume(&self, id: &str, secret: &str) -> Option<Arc<Jam>> {
        let jam = self.get(id)?;
        (same(&jam.secret, secret) && !jam.host_online()).then_some(jam)
    }

    /// Nouvelle Jam, sauf si `max` sont déjà ouvertes.
    pub fn create(&self, code: String, max: usize) -> Option<Arc<Jam>> {
        let mut jams = self.lock();
        if jams.len() >= max {
            return None;
        }
        let jam = Arc::new(Jam::new(code));
        jams.insert(jam.id.clone(), Arc::clone(&jam));
        Some(jam)
    }

    pub fn len(&self) -> usize {
        self.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// Ferme les Jams trop vieilles ou dont l'hôte n'est pas revenu à temps.
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
            tracing::info!("Jam fermée ({} encore ouvertes)", self.len());
            jam.close("Jam expirée");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));

    #[test]
    fn registry_limits_and_resume() {
        let registry = Registry::default();
        let jam = registry.create("AB23CD".into(), 1).unwrap();
        assert!(registry.create("X".into(), 1).is_none());
        // Pas d'hôte branché : reprise possible avec le bon secret seulement.
        assert!(registry.resume(&jam.id, "faux").is_none());
        assert!(registry.resume(&jam.id, jam.secret()).is_some());
        let (_rx, generation) = jam.attach("AB23CD".into());
        assert!(registry.resume(&jam.id, jam.secret()).is_none());
        jam.detach(generation + 1);
        assert!(jam.host_online());
        jam.detach(generation);
        assert!(!jam.host_online());
    }

    #[test]
    fn sweep_closes_abandoned_jams() {
        let registry = Registry::default();
        let jam = registry.create("AB23CD".into(), 5).unwrap();
        let (_rx, _) = jam.attach("AB23CD".into());
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
        let jam = registry.create("AB23CD".into(), 5).unwrap();
        assert_eq!(jam.check_code(IP, "ab23cd"), Ok(()));
        for _ in 0..MAX_FAILURES {
            assert_eq!(jam.check_code(IP, "nope"), Err(CodeError::Wrong));
        }
        assert_eq!(jam.check_code(IP, "AB23CD"), Err(CodeError::Blocked));
    }

    #[tokio::test]
    async fn requests_reach_the_host_and_come_back() {
        let registry = Registry::default();
        let jam = registry.create("AB23CD".into(), 5).unwrap();
        assert_eq!(
            jam.forward(IP, "/api/join".into(), None, String::new())
                .await,
            Err(ForwardError::Offline)
        );
        let (mut rx, _) = jam.attach("AB23CD".into());
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
