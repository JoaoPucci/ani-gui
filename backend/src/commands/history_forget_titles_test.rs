//! The title Continue Watching searched, read back from a row's title.

use super::*;

#[test]
fn a_legacy_episode_tail_is_left_out() {
    assert_eq!(
        without_episode_tail("One Piece (1100 episodes)"),
        "One Piece"
    );
    assert_eq!(without_episode_tail("Mushi-shi (1 Episode)"), "Mushi-shi");
    assert_eq!(
        without_episode_tail("Hellsing (13 episodes) (2001)"),
        "Hellsing"
    );
}

#[test]
fn a_title_without_the_tail_is_kept_whole() {
    assert_eq!(
        without_episode_tail("Mob Psycho 100 (II)"),
        "Mob Psycho 100 (II)"
    );
    assert_eq!(
        without_episode_tail("Steins;Gate (2011)"),
        "Steins;Gate (2011)"
    );
    assert_eq!(without_episode_tail("Re:Zero"), "Re:Zero");
}
