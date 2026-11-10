//! What the Watch Later rail gets from the bridge: each show once.
//!
//! Two MAL ids can lead to the same Kitsu show — Kitsu maps both to
//! it, directly or through their AniList ids — and the rail keys its
//! cards by Kitsu id, so a show it is handed twice is a duplicate key
//! the renderer refuses. Its own file rather than an append to
//! `account_test.rs`, which grows by appending.

use super::rail_cards;
use crate::meta::kitsu::KitsuAnimeRef;

/// A card for Kitsu show `id`, named `title` so a test can tell two
/// cards for the same show apart.
fn card(id: &str, title: &str) -> KitsuAnimeRef {
    KitsuAnimeRef {
        id: id.into(),
        canonical_title: title.into(),
        titles: std::collections::HashMap::new(),
        abbreviated_titles: Vec::new(),
        slug: None,
        synopsis: None,
        start_date: None,
        end_date: None,
        episode_count: None,
        average_rating: None,
        subtype: None,
        status: None,
        age_rating: None,
        popularity_rank: None,
        poster_image: None,
        cover_image: None,
        anilist_banner: None,
    }
}

fn titles(cards: &[KitsuAnimeRef]) -> Vec<&str> {
    cards.iter().map(|c| c.canonical_title.as_str()).collect()
}

#[test]
fn a_show_bridged_twice_is_listed_once_at_its_first_place() {
    let got = rail_cards(vec![
        card("1", "first"),
        card("2", "second"),
        card("1", "first again"),
        card("3", "third"),
    ]);
    assert_eq!(titles(&got), ["first", "second", "third"]);
}

#[test]
fn every_card_for_one_show_leaves_one() {
    let got = rail_cards(vec![card("7", "a"), card("7", "b"), card("7", "c")]);
    assert_eq!(titles(&got), ["a"]);
}

#[test]
fn distinct_shows_pass_through_in_order() {
    let got = rail_cards(vec![card("3", "c"), card("1", "a"), card("2", "b")]);
    assert_eq!(titles(&got), ["c", "a", "b"]);
}

proptest::proptest! {
    /// Over lists whose ids come from a small pool, so repeats are
    /// common: no show twice, each show kept as its first card, and
    /// nothing dropped that is not a repeat — the output is the input
    /// with every later card for an id already seen left out.
    #[test]
    fn each_show_once_as_its_first_card(ids in proptest::collection::vec(0u8..6, 0..40)) {
        let input: Vec<KitsuAnimeRef> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| card(&id.to_string(), &i.to_string()))
            .collect();
        let got = rail_cards(input.clone());

        let mut seen = std::collections::HashSet::new();
        proptest::prop_assert!(got.iter().all(|c| seen.insert(c.id.clone())));

        let firsts: Vec<KitsuAnimeRef> = input
            .iter()
            .enumerate()
            .filter(|(i, c)| input.iter().position(|o| o.id == c.id) == Some(*i))
            .map(|(_, c)| c.clone())
            .collect();
        proptest::prop_assert_eq!(got, firsts);
    }
}
