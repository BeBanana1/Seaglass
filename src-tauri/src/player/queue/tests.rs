use super::*;
use rand::SeedableRng;
use rand::rngs::StdRng;

fn ids(range: std::ops::Range<u64>) -> Vec<TrackId> {
    range.map(TrackId).collect()
}

fn rng() -> StdRng {
    StdRng::seed_from_u64(42)
}

fn drain(q: &mut Queue) -> Vec<u64> {
    std::iter::from_fn(|| q.advance()).map(|t| t.0).collect()
}

#[test]
fn plays_context_in_order_then_stops() {
    let mut q = Queue::default();
    assert_eq!(q.set_context(ids(0..4), 1, &mut rng()), Some(TrackId(1)));
    assert_eq!(drain(&mut q), vec![2, 3]);
    assert_eq!(q.current(), Some(TrackId(3)));
    assert_eq!(q.peek_next(), None);
}

#[test]
fn invalid_start_is_rejected() {
    let mut q = Queue::default();
    assert_eq!(q.set_context(ids(0..2), 5, &mut rng()), None);
    assert_eq!(q.set_context(Vec::new(), 0, &mut rng()), None);
    assert_eq!(q.current(), None);
}

#[test]
fn repeat_all_wraps_around() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 1, &mut rng());
    q.set_repeat(Repeat::All);
    let played: Vec<u64> = (0..5).filter_map(|_| q.advance()).map(|t| t.0).collect();
    assert_eq!(played, vec![2, 0, 1, 2, 0]);
    assert_eq!(
        q.upcoming_context(10),
        ids(1..3).into_iter().chain(ids(0..1)).collect::<Vec<_>>()
    );
}

#[test]
fn repeat_one_is_left_to_mpv() {
    let mut q = Queue::default();
    q.set_context(ids(0..2), 0, &mut rng());
    q.set_repeat(Repeat::One);
    assert_eq!(q.peek_next(), Some(TrackId(1)));
    assert_eq!(q.advance(), Some(TrackId(1)));
}

#[test]
fn manual_queue_has_priority_and_play_next_goes_first() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, &mut rng());
    q.add_to_queue(TrackId(100));
    q.add_to_queue(TrackId(101));
    q.play_next(TrackId(99));
    assert_eq!(q.peek_next(), Some(TrackId(99)));
    assert_eq!(drain(&mut q), vec![99, 100, 101, 1, 2]);
}

#[test]
fn enqueue_next_keeps_album_order_ahead_of_manual_queue() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, &mut rng());
    q.add_to_queue(TrackId(100));
    q.enqueue(&ids(10..13), true);
    assert_eq!(drain(&mut q), vec![10, 11, 12, 100, 1, 2]);
}

#[test]
fn enqueue_appends_album_in_order_after_manual_queue() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, &mut rng());
    q.add_to_queue(TrackId(100));
    q.enqueue(&ids(10..13), false);
    assert_eq!(drain(&mut q), vec![100, 10, 11, 12, 1, 2]);
}

#[test]
fn enqueue_changes_the_preloaded_next_track() {
    // Le titre suivant préchargé dans mpv n'est plus le bon : l'acteur doit le retirer.
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, &mut rng());
    assert_eq!(q.peek_next(), Some(TrackId(1)));
    q.enqueue(&ids(10..12), false);
    assert_eq!(q.peek_next(), Some(TrackId(10)));
    q.enqueue(&ids(20..22), true);
    assert_eq!(q.peek_next(), Some(TrackId(20)));
    assert_eq!(q.current(), Some(TrackId(0)));
}

#[test]
fn enqueue_nothing_is_a_no_op() {
    let mut q = Queue::default();
    q.set_context(ids(0..2), 0, &mut rng());
    let before = q.clone();
    q.enqueue(&[], true);
    q.enqueue(&[], false);
    assert_eq!(q, before);
}

#[test]
fn remove_manual_entry() {
    let mut q = Queue::default();
    q.set_context(ids(0..2), 0, &mut rng());
    q.add_to_queue(TrackId(100));
    q.add_to_queue(TrackId(101));
    assert!(q.remove_manual(0));
    assert!(!q.remove_manual(5));
    assert_eq!(q.manual().collect::<Vec<_>>(), vec![TrackId(101)]);
}

fn manual_ids(q: &Queue) -> Vec<u64> {
    q.manual().map(|t| t.0).collect()
}

#[test]
fn move_manual_down_and_up() {
    let mut q = Queue::default();
    q.set_context(ids(0..2), 0, &mut rng());
    q.enqueue(&ids(10..14), false);
    // Vers le bas : l'entrée finit à l'index `to`.
    assert!(q.move_manual(0, 2));
    assert_eq!(manual_ids(&q), vec![11, 12, 10, 13]);
    // Vers le haut, jusqu'en tête.
    assert!(q.move_manual(3, 0));
    assert_eq!(manual_ids(&q), vec![13, 11, 12, 10]);
    // Monter / Descendre d'un cran.
    assert!(q.move_manual(2, 1));
    assert_eq!(manual_ids(&q), vec![13, 12, 11, 10]);
    assert!(q.move_manual(2, 3));
    assert_eq!(manual_ids(&q), vec![13, 12, 10, 11]);
    // Le contexte n'est pas touché.
    assert_eq!(drain(&mut q), vec![13, 12, 10, 11, 1]);
}

#[test]
fn move_manual_rejects_no_op_and_out_of_range() {
    let mut q = Queue::default();
    q.set_context(ids(0..2), 0, &mut rng());
    q.enqueue(&ids(10..13), false);
    let before = q.clone();
    assert!(!q.move_manual(1, 1));
    assert!(!q.move_manual(3, 0));
    assert!(!q.move_manual(0, 3));
    assert_eq!(q, before);
    let mut empty = Queue::default();
    assert!(!empty.move_manual(0, 0));
}

#[test]
fn move_manual_changes_next_only_through_the_head() {
    // L'acteur ne retire le préchargé que si `peek_next` change.
    let mut q = Queue::default();
    q.set_context(ids(0..2), 0, &mut rng());
    q.enqueue(&ids(10..14), false);
    q.move_manual(1, 3);
    assert_eq!(q.peek_next(), Some(TrackId(10)));
    q.move_manual(2, 0);
    assert_eq!(q.peek_next(), Some(TrackId(13)));
    q.move_manual(0, 3);
    assert_eq!(q.peek_next(), Some(TrackId(10)));
}

#[test]
fn shuffle_keeps_current_first_and_is_a_permutation() {
    let mut q = Queue::default();
    q.set_context(ids(0..20), 7, &mut rng());
    q.set_shuffle(true, &mut rng());
    assert_eq!(q.current(), Some(TrackId(7)));
    let mut rest = drain(&mut q);
    assert_eq!(rest.len(), 19);
    assert_ne!(rest, (0..20).filter(|&i| i != 7).collect::<Vec<_>>());
    rest.sort_unstable();
    assert_eq!(rest, (0..20).filter(|&i| i != 7).collect::<Vec<_>>());
}

#[test]
fn unshuffle_restores_original_order_at_current_track() {
    let mut q = Queue::default();
    q.set_context(ids(0..10), 0, &mut rng());
    q.set_shuffle(true, &mut rng());
    q.advance();
    q.advance();
    let current = q.current().map(|t| t.0).unwrap_or_default();
    q.set_shuffle(false, &mut rng());
    assert_eq!(q.current(), Some(TrackId(current)));
    assert_eq!(drain(&mut q), ((current + 1)..10).collect::<Vec<_>>());
}

#[test]
fn shuffled_context_starts_on_requested_track() {
    let mut q = Queue::default();
    q.set_shuffle(true, &mut rng());
    assert_eq!(q.set_context(ids(0..10), 4, &mut rng()), Some(TrackId(4)));
    assert_eq!(drain(&mut q).len(), 9);
}

#[test]
fn previous_then_next_returns_to_left_track() {
    let mut q = Queue::default();
    q.set_context(ids(0..5), 0, &mut rng());
    q.advance();
    q.advance(); // courant : 2
    assert_eq!(q.previous(), Some(TrackId(1)));
    assert_eq!(q.previous(), Some(TrackId(0)));
    assert_eq!(q.previous(), None);
    assert_eq!(q.advance(), Some(TrackId(1)));
    assert_eq!(q.advance(), Some(TrackId(2)));
}

#[test]
fn previous_is_independent_of_shuffle() {
    let mut q = Queue::default();
    q.set_context(ids(0..10), 0, &mut rng());
    q.advance();
    q.advance(); // historique : 0, 1 ; courant : 2
    q.set_shuffle(true, &mut rng());
    assert_eq!(q.previous(), Some(TrackId(1)));
    assert_eq!(q.previous(), Some(TrackId(0)));
}

#[test]
fn previous_from_manual_track_requeues_it() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, &mut rng());
    q.play_next(TrackId(50));
    assert_eq!(q.advance(), Some(TrackId(50)));
    assert_eq!(q.previous(), Some(TrackId(0)));
    assert_eq!(q.peek_next(), Some(TrackId(50)));
}

#[test]
fn previous_into_replaced_context_keeps_cursor() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 2, &mut rng());
    q.set_context(ids(10..13), 0, &mut rng());
    assert_eq!(q.previous(), Some(TrackId(2)));
    assert_eq!(q.advance(), Some(TrackId(11)));
}

#[test]
fn serde_roundtrip() {
    let mut q = Queue::default();
    q.set_context(ids(0..5), 1, &mut rng());
    q.add_to_queue(TrackId(9));
    q.set_repeat(Repeat::All);
    let json = serde_json::to_string(&q).unwrap();
    let back: Queue = serde_json::from_str(&json).unwrap();
    assert_eq!(back, q);
}

#[test]
fn clear_upcoming_keeps_current_and_history() {
    let mut q = Queue::default();
    q.set_context(ids(0..5), 0, &mut rng());
    q.advance();
    q.add_to_queue(TrackId(100));
    q.set_repeat(Repeat::All);
    assert!(q.clear_upcoming());
    assert_eq!(q.current(), Some(TrackId(1)));
    assert_eq!(q.manual().count(), 0);
    assert!(q.upcoming_context(10).is_empty());
    assert_eq!(q.peek_next(), None);
    assert_eq!(q.advance(), None);
    assert_eq!(q.current(), Some(TrackId(1)));
    // L'historique reste : « Précédent » revient au titre 0, « Suivant » ramène au titre quitté.
    assert_eq!(q.previous(), Some(TrackId(0)));
    assert_eq!(q.advance(), Some(TrackId(1)));
    assert!(!q.clear_upcoming());
}

#[test]
fn clear_upcoming_then_new_items_play_normally() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, &mut rng());
    q.set_shuffle(true, &mut rng());
    assert!(q.clear_upcoming());
    q.set_shuffle(false, &mut rng());
    q.add_to_queue(TrackId(7));
    assert_eq!(drain(&mut q), vec![7]);
    assert_eq!(q.set_context(ids(10..12), 0, &mut rng()), Some(TrackId(10)));
    assert_eq!(drain(&mut q), vec![11]);
}
