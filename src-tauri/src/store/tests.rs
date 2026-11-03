use std::cell::RefCell;

use super::*;
use crate::tidal::models::{AlbumRef, TrackId};
use rand::SeedableRng;

fn track(id: u64) -> Track {
    Track {
        id: TrackId(id),
        title: format!("t{id}"),
        version: None,
        duration: 100,
        artists: Vec::new(),
        album: AlbumRef {
            id: 1,
            title: "a".into(),
            cover_url: None,
            cover_small_url: None,
            cover_large_url: None,
            color: None,
        },
        explicit: false,
        hires: true,
        isrc: None,
    }
}

fn queue_of(ids: impl IntoIterator<Item = u64>) -> Queue {
    let mut q = Queue::default();
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    q.set_context(ids.into_iter().map(TrackId).collect(), 0, &mut rng);
    q
}

/// Ids présents dans la table, triés.
fn stored_ids(store: &Store) -> Vec<u64> {
    let db = store.db.lock().unwrap();
    let mut stmt = db
        .conn
        .prepare("SELECT id FROM tracks ORDER BY id")
        .unwrap();
    stmt.query_map([], |r| r.get::<_, i64>(0))
        .unwrap()
        .map(|id| id.unwrap() as u64)
        .collect()
}

/// Sauvegarde la file en notant les ids demandés à `lookup`.
fn save(store: &Store, queue: &Queue, asked: &RefCell<Vec<u64>>) {
    store
        .save_player(Some(queue), 0.0, |ids| {
            asked.borrow_mut().extend(ids.iter().map(|id| id.0));
            ids.iter().map(|id| track(id.0)).collect()
        })
        .unwrap();
}

#[test]
fn settings_roundtrip() {
    let store = Store::in_memory().unwrap();
    assert_eq!(store.get_setting::<String>("ui").unwrap(), None);
    store.set_setting("ui", &"studio").unwrap();
    store.set_setting("ui", &"atelier").unwrap();
    assert_eq!(
        store.get_setting::<String>("ui").unwrap().as_deref(),
        Some("atelier")
    );
}

/// Un réglage lu une fois est servi depuis la mémoire, et une écriture le remplace.
#[test]
fn settings_are_cached_and_kept_in_sync() {
    let store = Store::in_memory().unwrap();
    store.set_setting("ui", &"studio").unwrap();
    assert_eq!(
        store.get_setting::<String>("ui").unwrap().as_deref(),
        Some("studio")
    );
    // Modification directe en base : la valeur en mémoire fait foi.
    store
        .db
        .lock()
        .unwrap()
        .conn
        .execute("UPDATE kv SET value = '\"atelier\"' WHERE key = 'ui'", [])
        .unwrap();
    assert_eq!(
        store.get_setting::<String>("ui").unwrap().as_deref(),
        Some("studio")
    );
    // Une absence est aussi gardée, puis remplacée par l'écriture.
    assert_eq!(store.get_setting::<String>("other").unwrap(), None);
    store.set_setting("other", &"x").unwrap();
    assert_eq!(
        store.get_setting::<String>("other").unwrap().as_deref(),
        Some("x")
    );
}

#[test]
fn empty_store_has_nothing() {
    let store = Store::in_memory().unwrap();
    assert_eq!(store.load_player().unwrap(), None);
    assert_eq!(store.load_volume().unwrap(), None);
}

#[test]
fn volume_roundtrip_and_clamp() {
    let store = Store::in_memory().unwrap();
    let v = Volume {
        level: 35,
        muted: true,
    };
    store.save_volume(v).unwrap();
    assert_eq!(store.load_volume().unwrap(), Some(v));

    // Valeur hors limites écrite à la main dans la base : ramenée à 100.
    store
        .set_setting(
            KEY_VOLUME,
            &serde_json::json!({ "level": 250, "muted": false }),
        )
        .unwrap();
    assert_eq!(store.load_volume().unwrap(), Some(Volume::default()));
}

#[test]
fn roundtrip_queue_position_and_tracks() {
    let store = Store::in_memory().unwrap();
    let q = queue_of([1, 2]);
    let asked = RefCell::default();
    save(&store, &q, &asked);
    store.save_player(None, 42.0, |_| unreachable!()).unwrap();

    let saved = store.load_player().unwrap().unwrap();
    assert_eq!(saved.queue, q);
    assert_eq!(saved.position, 42.0);
    assert_eq!(saved.tracks, vec![track(1), track(2)]);
}

/// Seuls les titres absents de la table sont demandés et écrits ; ceux qui ne sont plus
/// référencés sont supprimés.
#[test]
fn queue_tracks_are_written_once_and_purged() {
    let store = Store::in_memory().unwrap();
    let asked = RefCell::default();
    save(&store, &queue_of([1, 2]), &asked);
    assert_eq!(*asked.borrow(), [1, 2]);

    asked.borrow_mut().clear();
    save(&store, &queue_of([1, 2, 3]), &asked);
    assert_eq!(*asked.borrow(), [3]);
    assert_eq!(stored_ids(&store), [1, 2, 3]);

    // Nouveau contexte : les anciens titres sortent (l'historique garde le précédent courant).
    asked.borrow_mut().clear();
    let mut q = queue_of([1, 2, 3]);
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    q.set_context(vec![TrackId(4), TrackId(5)], 0, &mut rng);
    save(&store, &q, &asked);
    assert_eq!(*asked.borrow(), [4, 5]);
    let mut expected = q.referenced_ids().iter().map(|id| id.0).collect::<Vec<_>>();
    expected.sort_unstable();
    assert_eq!(stored_ids(&store), expected);
    assert!(!stored_ids(&store).contains(&2));

    // Même file : rien à demander.
    asked.borrow_mut().clear();
    save(&store, &q, &asked);
    assert!(asked.borrow().is_empty());
}

/// Un titre inconnu du cache n'est pas compté comme enregistré : redemandé ensuite.
#[test]
fn unknown_track_is_retried() {
    let store = Store::in_memory().unwrap();
    let q = queue_of([1, 2]);
    store
        .save_player(Some(&q), 0.0, |_| vec![track(1)])
        .unwrap();
    assert_eq!(stored_ids(&store), [1]);
    let asked = RefCell::default();
    save(&store, &q, &asked);
    assert_eq!(*asked.borrow(), [2]);
    assert_eq!(stored_ids(&store), [1, 2]);
}

/// Les lignes laissées par les versions précédentes (jamais purgées) partent à la première
/// sauvegarde de la file.
#[test]
fn legacy_rows_are_purged_on_first_save() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    for id in [1_i64, 7, 8] {
        conn.execute(
            "INSERT INTO tracks (id, json) VALUES (?1, ?2)",
            params![id, serde_json::to_string(&track(id as u64)).unwrap()],
        )
        .unwrap();
    }
    let store = Store::init(conn).unwrap();
    let asked = RefCell::default();
    save(&store, &queue_of([1, 2]), &asked);
    assert_eq!(*asked.borrow(), [2]);
    assert_eq!(stored_ids(&store), [1, 2]);
}
