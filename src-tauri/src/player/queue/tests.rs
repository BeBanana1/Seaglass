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
    assert_eq!(
        q.set_context(ids(0..4), 1, None, &mut rng()),
        Some(TrackId(1))
    );
    assert_eq!(drain(&mut q), vec![2, 3]);
    assert_eq!(q.current(), Some(TrackId(3)));
    assert_eq!(q.peek_next(), None);
}

#[test]
fn invalid_start_is_rejected() {
    let mut q = Queue::default();
    assert_eq!(q.set_context(ids(0..2), 5, None, &mut rng()), None);
    assert_eq!(q.set_context(Vec::new(), 0, None, &mut rng()), None);
    assert_eq!(q.current(), None);
}

#[test]
fn repeat_all_wraps_around() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 1, None, &mut rng());
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
    q.set_context(ids(0..2), 0, None, &mut rng());
    q.set_repeat(Repeat::One);
    assert_eq!(q.peek_next(), Some(TrackId(1)));
    assert_eq!(q.advance(), Some(TrackId(1)));
}

#[test]
fn manual_queue_has_priority_and_play_next_goes_first() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, None, &mut rng());
    q.add_to_queue(TrackId(100));
    q.add_to_queue(TrackId(101));
    q.play_next(TrackId(99));
    assert_eq!(q.peek_next(), Some(TrackId(99)));
    assert_eq!(drain(&mut q), vec![99, 100, 101, 1, 2]);
}

#[test]
fn enqueue_next_keeps_album_order_ahead_of_manual_queue() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, None, &mut rng());
    q.add_to_queue(TrackId(100));
    q.enqueue(&ids(10..13), true);
    assert_eq!(drain(&mut q), vec![10, 11, 12, 100, 1, 2]);
}

#[test]
fn enqueue_appends_album_in_order_after_manual_queue() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, None, &mut rng());
    q.add_to_queue(TrackId(100));
    q.enqueue(&ids(10..13), false);
    assert_eq!(drain(&mut q), vec![100, 10, 11, 12, 1, 2]);
}

#[test]
fn enqueue_changes_the_preloaded_next_track() {
    // Le titre suivant préchargé dans mpv n'est plus le bon : l'acteur doit le retirer.
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, None, &mut rng());
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
    q.set_context(ids(0..2), 0, None, &mut rng());
    let before = q.clone();
    q.enqueue(&[], true);
    q.enqueue(&[], false);
    assert_eq!(q, before);
}

#[test]
fn remove_manual_entries_by_id() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, None, &mut rng());
    q.enqueue(&[TrackId(7), TrackId(8), TrackId(7), TrackId(9)], false);
    // Une entrée par identifiant demandé ; le contexte n'est pas touché.
    assert_eq!(
        q.remove_manual_ids(&[TrackId(7), TrackId(1), TrackId(9)]),
        2
    );
    assert_eq!(q.manual().map(|t| t.0).collect::<Vec<_>>(), vec![8, 7]);
    assert_eq!(q.peek_next(), Some(TrackId(8)));
}

#[test]
fn remove_manual_entry() {
    let mut q = Queue::default();
    q.set_context(ids(0..2), 0, None, &mut rng());
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
    q.set_context(ids(0..2), 0, None, &mut rng());
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
    q.set_context(ids(0..2), 0, None, &mut rng());
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
    q.set_context(ids(0..2), 0, None, &mut rng());
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
    q.set_context(ids(0..20), 7, None, &mut rng());
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
    q.set_context(ids(0..10), 0, None, &mut rng());
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
    assert_eq!(
        q.set_context(ids(0..10), 4, None, &mut rng()),
        Some(TrackId(4))
    );
    assert_eq!(drain(&mut q).len(), 9);
}

#[test]
fn previous_then_next_returns_to_left_track() {
    let mut q = Queue::default();
    q.set_context(ids(0..5), 0, None, &mut rng());
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
    q.set_context(ids(0..10), 0, None, &mut rng());
    q.advance();
    q.advance(); // historique : 0, 1 ; courant : 2
    q.set_shuffle(true, &mut rng());
    assert_eq!(q.previous(), Some(TrackId(1)));
    assert_eq!(q.previous(), Some(TrackId(0)));
}

#[test]
fn previous_from_manual_track_requeues_it() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, None, &mut rng());
    q.play_next(TrackId(50));
    assert_eq!(q.advance(), Some(TrackId(50)));
    assert_eq!(q.previous(), Some(TrackId(0)));
    assert_eq!(q.peek_next(), Some(TrackId(50)));
}

#[test]
fn previous_into_replaced_context_keeps_cursor() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 2, None, &mut rng());
    q.set_context(ids(10..13), 0, None, &mut rng());
    assert_eq!(q.previous(), Some(TrackId(2)));
    // « Suivant » ramène au titre quitté (le curseur est resté dessus), puis continue.
    assert_eq!(q.advance(), Some(TrackId(10)));
    assert_eq!(q.advance(), Some(TrackId(11)));
}

/// #166 : remonter l'historique d'un contexte remplacé remettait chaque titre quitté dans
/// la file manuelle, pour de bon : ils passaient ensuite avant toute nouvelle liste.
#[test]
fn titles_recalled_by_previous_return_with_next_then_drop_with_new_context() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, None, &mut rng());
    q.advance();
    q.advance(); // historique : 0, 1 ; courant : 2
    q.set_context(ids(10..13), 0, None, &mut rng());
    assert_eq!(q.previous(), Some(TrackId(2)));
    assert_eq!(q.previous(), Some(TrackId(1)));
    assert_eq!(q.previous(), Some(TrackId(0)));
    // « Suivant » ramène au point de départ, sans sauter le titre 10, puis suit le contexte.
    assert_eq!(drain(&mut q), vec![1, 2, 10, 11, 12]);

    q.set_context(ids(0..3), 0, None, &mut rng());
    q.play_next(TrackId(50));
    q.set_context(ids(10..13), 0, None, &mut rng());
    assert_eq!(q.previous(), Some(TrackId(0)));
    assert_eq!(q.manual().map(|t| t.0).collect::<Vec<_>>(), vec![10, 50]);
    // Une nouvelle liste écarte les titres rappelés, pas ceux ajoutés par l'utilisateur.
    q.set_context(ids(20..23), 0, None, &mut rng());
    assert_eq!(q.manual().map(|t| t.0).collect::<Vec<_>>(), vec![50]);
    assert_eq!(drain(&mut q), vec![50, 21, 22]);
}

#[test]
fn previous_after_clear_then_new_list_plays_the_list() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, None, &mut rng());
    q.advance(); // courant : 1
    assert!(q.clear_upcoming());
    assert_eq!(q.previous(), Some(TrackId(0)));
    q.set_context(ids(10..13), 1, None, &mut rng());
    assert_eq!(q.peek_next(), Some(TrackId(12)));
}

#[test]
fn reads_queue_saved_before_origin() {
    let json = r#"{"context":[0,1,2],"order":[0,1,2],"pos":1,"manual":[60],
        "history":[{"id":0,"from_context":true}],"current":{"id":50,"from_context":false},
        "repeat":"off","shuffle":false}"#;
    let mut q: Queue = serde_json::from_str(json).unwrap();
    assert_eq!(q.current(), Some(TrackId(50)));
    // Ancien format : faux = file manuelle, remis en tête par « Précédent » ; un identifiant
    // seul dans la file manuelle = ajouté par l'utilisateur, gardé par une nouvelle liste.
    assert_eq!(q.previous(), Some(TrackId(0)));
    assert_eq!(q.manual().map(|t| t.0).collect::<Vec<_>>(), vec![50, 60]);
    q.set_context(ids(10..12), 0, None, &mut rng());
    assert_eq!(q.manual().map(|t| t.0).collect::<Vec<_>>(), vec![50, 60]);
}

#[test]
fn serde_roundtrip() {
    let mut q = Queue::default();
    q.set_context(ids(0..5), 1, None, &mut rng());
    q.add_to_queue(TrackId(9));
    q.set_repeat(Repeat::All);
    let json = serde_json::to_string(&q).unwrap();
    let back: Queue = serde_json::from_str(&json).unwrap();
    assert_eq!(back, q);
}

#[test]
fn clear_upcoming_keeps_current_and_history() {
    let mut q = Queue::default();
    q.set_context(ids(0..5), 0, None, &mut rng());
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
    q.set_context(ids(0..3), 0, None, &mut rng());
    q.set_shuffle(true, &mut rng());
    assert!(q.clear_upcoming());
    q.set_shuffle(false, &mut rng());
    q.add_to_queue(TrackId(7));
    assert_eq!(drain(&mut q), vec![7]);
    assert_eq!(
        q.set_context(ids(10..12), 0, None, &mut rng()),
        Some(TrackId(10))
    );
    assert_eq!(drain(&mut q), vec![11]);
}

// --- Radio automatique (#78) ---------------------------------------------------------

/// Album de 3 titres joué jusqu'au dernier.
fn finished_album() -> Queue {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, None, &mut rng());
    drain(&mut q);
    q
}

#[test]
fn radio_extends_an_exhausted_queue_without_recent_tracks() {
    let mut q = finished_album();
    assert!(q.wants_radio());
    // Titre courant, historique et doublons écartés.
    let radio = [2, 20, 0, 21, 20, 1, 22].map(TrackId);
    assert!(q.extend_with_radio(TrackId(2), &radio));
    assert_eq!(q.radio(), Some(TrackId(2)));
    assert_eq!(q.current(), Some(TrackId(2)));
    assert_eq!(q.peek_next(), Some(TrackId(20)));
    assert_eq!(q.upcoming_context(10), ids(20..23));
    assert!(!q.wants_radio());
    assert_eq!(drain(&mut q), vec![20, 21, 22]);
    // Fin de la radio : elle peut à son tour être prolongée.
    assert!(q.wants_radio());
}

#[test]
fn radio_keeps_previous_coherent() {
    let mut q = finished_album();
    assert!(q.extend_with_radio(TrackId(2), &ids(20..22)));
    assert_eq!(q.advance(), Some(TrackId(20)));
    // Retour au titre de départ, puis « Suivant » repart sur la radio.
    assert_eq!(q.previous(), Some(TrackId(2)));
    assert_eq!(q.advance(), Some(TrackId(20)));
    assert_eq!(q.previous(), Some(TrackId(2)));
    assert_eq!(q.previous(), Some(TrackId(1)));
}

#[test]
fn radio_is_refused_when_the_queue_moved_on() {
    // Titre courant changé pendant la requête.
    let mut q = finished_album();
    assert!(!q.extend_with_radio(TrackId(1), &ids(20..22)));
    // Un titre ajouté à la file entre-temps.
    q.add_to_queue(TrackId(9));
    assert!(!q.wants_radio());
    assert!(!q.extend_with_radio(TrackId(2), &ids(20..22)));
    // Rien de neuf dans la radio.
    let mut q = finished_album();
    assert!(!q.extend_with_radio(TrackId(2), &ids(0..3)));
    assert_eq!(q.radio(), None);
    assert_eq!(q.peek_next(), None);
}

#[test]
fn repeat_takes_priority_over_radio() {
    let mut q = finished_album();
    q.set_repeat(Repeat::All);
    assert!(!q.wants_radio());
    assert!(!q.extend_with_radio(TrackId(2), &ids(20..22)));
    q.set_repeat(Repeat::One);
    assert!(!q.wants_radio());
    // Repeat All après un « Vider la file » : rien ne suit, mais pas de radio.
    q.set_repeat(Repeat::All);
    q.clear_upcoming();
    assert_eq!(q.peek_next(), None);
    assert!(!q.wants_radio());
}

#[test]
fn new_context_or_clear_ends_the_radio() {
    let mut q = finished_album();
    assert!(q.extend_with_radio(TrackId(2), &ids(20..22)));
    assert!(q.clear_upcoming());
    assert_eq!(q.radio(), None);
    let mut q = finished_album();
    assert!(q.extend_with_radio(TrackId(2), &ids(20..22)));
    q.set_context(ids(10..12), 0, None, &mut rng());
    assert_eq!(q.radio(), None);
    assert_eq!(drain(&mut q), vec![11]);
}

#[test]
fn saved_queue_without_radio_field_is_read() {
    let q = finished_album();
    let mut json = serde_json::to_value(&q).unwrap();
    json.as_object_mut().unwrap().remove("radio");
    let back: Queue = serde_json::from_value(json).unwrap();
    assert_eq!(back, q);
}

#[test]
fn source_follows_the_current_track() {
    let album = PlaySource::Album {
        id: 1,
        title: "Discovery".into(),
    };
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, Some(album.clone()), &mut rng());
    assert_eq!(q.source(), Some(&album));

    // Un titre de la file manuelle ne vient pas de l'album.
    q.play_next(TrackId(9));
    q.advance();
    assert_eq!(q.source(), None);
    q.advance();
    assert_eq!(q.source(), Some(&album));

    // « Vider la file » détache le titre courant du contexte.
    q.clear_upcoming();
    assert_eq!(q.source(), None);

    // Un nouveau contexte sans source n'hérite pas de l'ancienne.
    q.set_context(ids(0..3), 0, Some(album), &mut rng());
    q.set_context(ids(5..7), 0, None, &mut rng());
    assert_eq!(q.source(), None);
}

#[test]
fn source_is_dropped_by_radio_and_restored_from_disk() {
    let source = PlaySource::LikedTracks;
    let mut q = Queue::default();
    q.set_context(ids(0..1), 0, Some(source.clone()), &mut rng());
    let saved = serde_json::to_string(&q).unwrap();
    let restored: Queue = serde_json::from_str(&saved).unwrap();
    assert_eq!(restored.source(), Some(&source));

    assert!(q.extend_with_radio(TrackId(0), &ids(10..12)));
    assert_eq!(q.source(), None);
    // Le titre de départ vient encore de l'ancien contexte ; la suite vient de la radio.
    assert_eq!(q.radio_playing(), None);
    q.advance();
    assert_eq!(q.radio_playing(), Some(TrackId(0)));
    q.play_next(TrackId(20));
    q.advance();
    assert_eq!(q.radio_playing(), None);
}

#[test]
fn jump_to_manual_entry_drops_the_ones_before_it() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 0, None, &mut rng());
    q.enqueue(&ids(50..54), false);
    assert_eq!(
        q.jump_to(QueuePart::Manual, 2, TrackId(52)),
        Some(TrackId(52))
    );
    assert_eq!(drain(&mut q), vec![53, 1, 2]);
    // Les titres sautés ne sont pas dans l'historique.
    let mut back = Vec::new();
    while let Some(t) = q.previous() {
        back.push(t.0);
    }
    assert_eq!(back, vec![1, 53, 52, 0]);
}

#[test]
fn jump_to_upcoming_keeps_manual_queue_and_skips_history() {
    let mut q = Queue::default();
    q.set_context(ids(0..6), 1, None, &mut rng());
    q.add_to_queue(TrackId(50));
    // Suite du contexte : 2, 3, 4, 5.
    assert_eq!(
        q.jump_to(QueuePart::Upcoming, 2, TrackId(4)),
        Some(TrackId(4))
    );
    assert_eq!(q.current(), Some(TrackId(4)));
    assert_eq!(q.upcoming_context(10), ids(5..6));
    assert_eq!(drain(&mut q), vec![50, 5]);
    assert_eq!(q.previous(), Some(TrackId(50)));
    assert_eq!(q.previous(), Some(TrackId(4)));
    assert_eq!(q.previous(), Some(TrackId(1)));
}

#[test]
fn jump_to_upcoming_follows_shuffle_and_repeat_all() {
    let mut q = Queue::default();
    q.set_context(ids(0..8), 3, None, &mut rng());
    q.set_shuffle(true, &mut rng());
    let upcoming = q.upcoming_context(10);
    assert_eq!(
        q.jump_to(QueuePart::Upcoming, 4, upcoming[4]),
        Some(upcoming[4])
    );
    assert_eq!(q.upcoming_context(10), upcoming[5..].to_vec());

    let mut q = Queue::default();
    q.set_context(ids(0..4), 2, None, &mut rng());
    q.set_repeat(Repeat::All);
    // Suite : 3, puis 0, 1, 2 après le retour au début.
    assert_eq!(
        q.jump_to(QueuePart::Upcoming, 2, TrackId(1)),
        Some(TrackId(1))
    );
    assert_eq!(q.peek_next(), Some(TrackId(2)));
}

#[test]
fn jump_to_a_moved_entry_does_nothing() {
    let mut q = Queue::default();
    q.set_context(ids(0..4), 0, None, &mut rng());
    q.enqueue(&ids(50..52), false);
    assert_eq!(q.jump_to(QueuePart::Manual, 0, TrackId(51)), None);
    assert_eq!(q.jump_to(QueuePart::Manual, 5, TrackId(50)), None);
    assert_eq!(q.jump_to(QueuePart::Upcoming, 0, TrackId(2)), None);
    assert_eq!(q.jump_to(QueuePart::Upcoming, 3, TrackId(3)), None);
    assert_eq!(q.current(), Some(TrackId(0)));
    assert_eq!(drain(&mut q), vec![50, 51, 1, 2, 3]);
}

#[test]
fn play_order_is_current_then_manual_then_context_capped() {
    let mut q = Queue::default();
    q.set_context(ids(0..5), 1, None, &mut rng());
    q.add_to_queue(TrackId(10));
    q.play_next(TrackId(11));
    let order: Vec<u64> = q.play_order(100).iter().map(|t| t.0).collect();
    assert_eq!(order, vec![1, 11, 10, 2, 3, 4]);
    let order: Vec<u64> = q.play_order(4).iter().map(|t| t.0).collect();
    assert_eq!(order, vec![1, 11, 10, 2]);
    assert_eq!(q.play_order(2).len(), 2);
}

#[test]
fn at_end_ignores_repeat_all_and_waits_for_the_manual_queue() {
    let mut q = Queue::default();
    q.set_context(ids(0..3), 1, None, &mut rng());
    q.set_repeat(Repeat::All);
    assert!(!q.at_end());
    q.advance();
    assert!(
        q.at_end(),
        "dernier titre : le retour au début ne compte pas"
    );
    q.add_to_queue(TrackId(9));
    assert!(!q.at_end(), "la file manuelle reste à lire");
    assert_eq!(q.advance(), Some(TrackId(9)));
    assert!(q.at_end());
}

#[test]
fn at_end_follows_the_shuffled_order() {
    let mut q = Queue::default();
    q.set_shuffle(true, &mut rng());
    q.set_context(ids(0..5), 2, None, &mut rng());
    for _ in 0..4 {
        assert!(!q.at_end());
        q.advance();
    }
    assert!(q.at_end());
    assert_eq!(q.peek_next(), None);
}

#[test]
fn empty_queue_is_at_end() {
    assert!(Queue::default().at_end());
}
