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

/// Pourquoi une Jam n'a pas pu être créée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateError {
    /// Le relais a déjà toutes ses Jams ouvertes.
    Full,
    /// Cette adresse a déjà toutes les siennes (#257).
    TooManyForAddress,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CodeError {
    Wrong,
    Blocked,
}

struct Inner {
    code: String,
    /// Adresse de l'hôte, suivie à chaque reprise (Wi-Fi puis réseau mobile) : compte dans
    /// le plafond de Jams par adresse (#257).
    host_ip: IpAddr,
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
        // Aucun code ne panique sous ce verrou : un verrou empoisonné reste utilisable.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Branche une connexion d'hôte (nouvelle ou reprise) : renvoie la file de ses messages
    /// et son numéro. Une connexion encore branchée est remplacée (#253) : son émetteur
    /// disparaît, ce qui la termine, et ses requêtes en attente échouent tout de suite
    /// (la page des invités réessaie) au lieu d'attendre une réponse qui ne viendra pas.
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

    fn host_ip(&self) -> IpAddr {
        self.lock().host_ip
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

    /// Jam à reprendre : avec le bon secret seulement. Une connexion d'hôte encore branchée
    /// n'empêche pas la reprise (#253) : quand le téléphone change de réseau, il revient
    /// bien avant que le relais ait vu mourir l'ancienne connexion (jusqu'à 80 s), et le
    /// secret prouve que c'est le même hôte.
    pub fn resume(&self, id: &str, secret: &str) -> Option<Arc<Jam>> {
        let jam = self.get(id)?;
        same(&jam.secret, secret).then_some(jam)
    }

    /// Nouvelle Jam, sauf si `max` sont déjà ouvertes sur le relais ou `max_per_address`
    /// pour l'adresse `host_ip` (#257 : une seule adresse ne remplit pas le relais).
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
    const OTHER: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 3));

    #[test]
    fn registry_limits_and_resume() {
        let registry = Registry::default();
        let jam = registry.create("AB23CD".into(), IP, 1, 3).unwrap();
        assert_eq!(
            registry.create("X".into(), IP, 1, 3).err(),
            Some(CreateError::Full)
        );
        // Reprise avec le bon secret seulement, qu'un hôte soit branché ou non (#253).
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
        // Une requête attend l'ancienne connexion, qui ne répondra plus.
        let waiting = {
            let jam = Arc::clone(&jam);
            tokio::spawn(async move {
                jam.forward(IP, "/api/join".into(), None, String::new())
                    .await
            })
        };
        assert!(matches!(old.recv().await, Some(ToHost::Request { .. })));

        let (mut new, new_gen) = jam.attach("AB23CD".into(), OTHER);
        // Elle échoue tout de suite, et l'ancienne connexion voit sa file se fermer.
        assert_eq!(waiting.await.unwrap(), Err(ForwardError::Offline));
        assert!(old.recv().await.is_none());
        // L'ancienne connexion qui se ferme ne détache pas la nouvelle.
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
        // Une autre adresse garde ses places.
        assert!(registry.create("C".into(), OTHER, 10, 2).is_ok());
        // L'hôte revenu par une autre adresse (reprise) libère une place de l'ancienne.
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
        assert_eq!(jam.check_code(IP, "ab23cd"), Ok(()));
        for _ in 0..MAX_FAILURES {
            assert_eq!(jam.check_code(IP, "nope"), Err(CodeError::Wrong));
        }
        assert_eq!(jam.check_code(IP, "AB23CD"), Err(CodeError::Blocked));
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
