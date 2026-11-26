use super::*;
use crate::tidal::models::{AlbumRef, ArtistRef, TrackId};
use rand::SeedableRng;
use rand::rngs::StdRng;

fn track(id: u64, title: &str, artist: &str) -> Track {
    Track {
        id: TrackId(id),
        title: title.into(),
        version: None,
        duration: 200,
        artists: vec![ArtistRef {
            id,
            name: artist.into(),
        }],
        album: AlbumRef {
            id,
            title: String::new(),
            cover_url: None,
            cover_small_url: None,
            cover_large_url: None,
            color: None,
        },
        explicit: false,
        ai: false,
        hires: false,
        isrc: None,
    }
}

fn pool(n: u64) -> Vec<Track> {
    (1..=n)
        .map(|i| track(i, &format!("Song {i}"), &format!("Artist {i}")))
        .collect()
}

/// Answer of the host alone: the revealed round.
fn solo(game: &mut Game, answer: Answer) -> Option<RoundResult> {
    game.answer(HOST_KEY, None, answer).ok()?;
    game.results().last().cloned()
}

fn rng() -> StdRng {
    StdRng::seed_from_u64(7)
}

#[test]
fn needs_four_different_titles() {
    let mut rng = rng();
    assert_eq!(
        Game::new(pool(3), Mode::Typing, 5, Party::SOLO, &mut rng).err(),
        Some(GameError::TooFewTracks)
    );
    // Duplicates and editions of the same track only count once.
    let mut same = pool(3);
    same.push(track(1, "Song 1", "Artist 1"));
    same.push(track(9, "Song 1 (Remastered)", "Artist 1"));
    assert!(Game::new(same, Mode::Typing, 5, Party::SOLO, &mut rng).is_err());
    assert!(Game::new(pool(4), Mode::Typing, 5, Party::SOLO, &mut rng).is_ok());
}

#[test]
fn rounds_are_drawn_without_replacement() {
    let mut rng = rng();
    let mut game = Game::new(pool(6), Mode::Typing, 10, Party::SOLO, &mut rng).unwrap();
    assert_eq!(game.rounds(), 6, "no more rounds than tracks");
    let mut seen = Vec::new();
    loop {
        seen.push(game.track().unwrap().id);
        solo(&mut game, Answer::Pass);
        if !game.next_round(&mut rng) {
            break;
        }
    }
    seen.sort_by_key(|id| id.0);
    seen.dedup();
    assert_eq!(seen.len(), 6);
    assert_eq!(game.phase, Phase::Finished);
    assert!(game.track().is_none());
    assert_eq!(game.round_number(), 6);
}

#[test]
fn typing_scores_title_and_artist() {
    let mut rng = rng();
    let mut game = Game::new(pool(5), Mode::Typing, 3, Party::SOLO, &mut rng).unwrap();
    let t = game.track().unwrap().clone();
    assert!(game.hides_track());
    let r = solo(
        &mut game,
        Answer::Text {
            text: t.title.clone(),
        },
    )
    .unwrap();
    assert_eq!((r.title, r.artist, r.points), (true, false, 2));
    assert!(!game.hides_track());
    // A single answer per round.
    assert!(solo(&mut game, Answer::Pass).is_none());

    assert!(game.next_round(&mut rng));
    let t = game.track().unwrap().clone();
    let text = format!("{} {}", t.artists[0].name, t.title);
    assert_eq!(solo(&mut game, Answer::Text { text }).unwrap().points, 3);
    assert_eq!(game.host_score(), (5, 2, 2));

    assert!(game.next_round(&mut rng));
    solo(
        &mut game,
        Answer::Text {
            text: "rien à voir".into(),
        },
    );
    assert_eq!(game.host_score(), (5, 0, 2));
    assert!(!game.next_round(&mut rng));
    assert_eq!(game.results().len(), 3);
}

#[test]
fn choices_hold_the_answer_once() {
    let mut rng = rng();
    let mut game = Game::new(pool(8), Mode::Choices, 4, Party::SOLO, &mut rng).unwrap();
    for _ in 0..4 {
        let id = game.track().unwrap().id;
        let choices = game.choices().to_vec();
        assert_eq!(choices.len(), 4);
        let right: Vec<usize> = choices
            .iter()
            .enumerate()
            .filter(|(_, c)| c.title == format!("Song {}", id.0))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(right.len(), 1);
        let r = solo(&mut game, Answer::Choice { index: right[0] }).unwrap();
        assert_eq!(
            (r.points, r.chosen, r.correct),
            (3, Some(right[0]), Some(right[0]))
        );
        game.next_round(&mut rng);
    }
    assert_eq!(game.host_score().0, 12);
}

#[test]
fn a_wrong_choice_scores_nothing() {
    let mut rng = rng();
    let mut game = Game::new(pool(4), Mode::Choices, 1, Party::SOLO, &mut rng).unwrap();
    let wrong = (0..4).find(|i| game.choices()[*i].title != game.track().unwrap().title);
    let r = solo(
        &mut game,
        Answer::Choice {
            index: wrong.unwrap(),
        },
    )
    .unwrap();
    assert_eq!(r.points, 0);
    // A choice is not a valid answer when typing.
    let mut game = Game::new(pool(4), Mode::Typing, 1, Party::SOLO, &mut rng).unwrap();
    assert_eq!(
        solo(&mut game, Answer::Choice { index: 0 }).unwrap().points,
        0
    );
}

#[test]
fn an_unplayable_track_is_replaced() {
    let mut rng = rng();
    let mut game = Game::new(pool(5), Mode::Typing, 5, Party::SOLO, &mut rng).unwrap();
    let first = game.track().unwrap().id;
    assert!(game.replace_track(&mut rng));
    assert_ne!(game.track().unwrap().id, first);
    assert_eq!(game.round_number(), 1);
    // Nothing left to replace it: the game stops on the rounds played.
    solo(&mut game, Answer::Pass);
    assert!(game.next_round(&mut rng));
    while game.replace_track(&mut rng) {}
    assert_eq!(game.phase, Phase::Finished);
    assert_eq!(game.rounds(), 1);
}

const JAM: Party = Party {
    jam: true,
    host_plays: true,
};

#[test]
fn with_guests_the_round_waits_for_everyone() {
    let mut rng = rng();
    let mut game = Game::new(pool(6), Mode::Typing, 3, JAM, &mut rng).unwrap();
    let title = game.track().unwrap().title.clone();
    assert_eq!(game.answer("lea", Some("Léa"), Answer::Pass), Ok(1));
    assert!(game.hides_track(), "the host has not answered");
    assert_eq!(
        game.answer(
            HOST_KEY,
            None,
            Answer::Text {
                text: title.clone()
            }
        ),
        Ok(0)
    );
    // All known players answered: revealed.
    assert!(!game.hides_track());
    let r = game.results().last().unwrap();
    assert_eq!(r.points, 3, "title + bonus for the fastest");
    assert_eq!(r.players.len(), 2);
    assert_eq!(
        game.answer("max", None, Answer::Pass),
        Err(Refusal::Closed),
        "trop tard"
    );

    // The host reveals without waiting for Léa.
    assert!(game.next_round(&mut rng));
    let title = game.track().unwrap().title.clone();
    game.answer(HOST_KEY, None, Answer::Text { text: title })
        .unwrap();
    assert!(game.hides_track());
    assert!(game.reveal());
    let lea = game
        .results()
        .last()
        .unwrap()
        .players
        .iter()
        .find(|p| p.id == 1)
        .cloned();
    assert_eq!(lea.map(|p| (p.answered, p.points)), Some((false, 0)));
    assert_eq!(game.host_score(), (6, 2, 2));
}

#[test]
fn a_host_who_watches_reveals_on_the_guests() {
    let mut rng = rng();
    let party = Party {
        jam: true,
        host_plays: false,
    };
    let mut game = Game::new(pool(6), Mode::Typing, 2, party, &mut rng).unwrap();
    assert_eq!(
        game.answer(HOST_KEY, None, Answer::Pass),
        Err(Refusal::Closed)
    );
    game.answer("lea", None, Answer::Pass).unwrap();
    assert!(!game.hides_track(), "Léa was the only player");
    assert_eq!(game.results().last().unwrap().points, 0);
    // Alone, a guest cannot answer.
    let mut solo_game = Game::new(pool(6), Mode::Typing, 2, Party::SOLO, &mut rng).unwrap();
    assert_eq!(
        solo_game.answer("lea", None, Answer::Pass),
        Err(Refusal::Closed)
    );
}

#[test]
fn clips_avoid_the_edges() {
    let mut rng = rng();
    for _ in 0..200 {
        let start = clip_start(240, &mut rng);
        assert!((48.0..=215.0).contains(&start), "{start}");
        let start = clip_start(600, &mut rng);
        assert!((60.0..=575.0).contains(&start), "{start}");
    }
    assert_eq!(clip_start(20, &mut rng), 2.0);
    assert_eq!(clip_start(10, &mut rng), 0.0);
}
