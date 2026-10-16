use super::*;

use std::collections::BTreeMap;

use crate::meta::anilist_streaming_eps::{parse_streaming_episodes_response, StreamingEntry};
use crate::meta::kitsu::parse_episodes_response;

/// The recorded AniList listing and Kitsu episode page for `slug`
/// under `tests/fixtures/episode-thumbs/`.
fn recorded(slug: &str, kitsu_page: u32) -> (StreamingListing, Vec<KitsuEpisode>) {
    let dir = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../tests/fixtures/episode-thumbs"
    );
    let anilist = std::fs::read(format!("{dir}/{slug}.anilist.json")).expect("anilist fixture");
    let kitsu =
        std::fs::read(format!("{dir}/{slug}.kitsu-p{kitsu_page}.json")).expect("kitsu fixture");
    (
        parse_streaming_episodes_response(&anilist).expect("anilist parses"),
        parse_episodes_response(&kitsu).expect("kitsu parses"),
    )
}

// --- The recorded shows -------------------------------------------
//
// Every Gintama entry AniList keeps a numbered listing for carries the
// same 49 entries — the 2006 series' first 49 episodes. Only the 2006
// series may use them.

#[test]
fn the_2006_series_keeps_its_own_listing() {
    let (listing, page) = recorded("gintama", 1);
    assert!(listing_fits(&listing, &page));
}

#[test]
fn every_later_gintama_entry_refuses_the_2006_listing() {
    for slug in [
        "gintama-dot",
        "gintama-dash",
        "gintama-dash-enchousen",
        "gintama-degree",
        "gintama-dot-porori",
        "gintama-dot-shirogane",
        "gintama-dot-shirogane-2",
    ] {
        let (listing, page) = recorded(slug, 1);
        assert!(
            !listing_fits(&listing, &page),
            "{slug} took the 2006 listing"
        );
    }
}

#[test]
fn a_listing_shorter_than_the_count_is_still_refused_when_its_titles_disagree() {
    // Gintama' (51 episodes) and Gintama° (51): 49 entries, all within
    // the count, so only the titles can tell.
    for slug in ["gintama-dash", "gintama-degree"] {
        let (listing, page) = recorded(slug, 1);
        assert!(listing.entries.keys().all(|n| Some(*n) <= listing.episodes));
        assert!(!listing_fits(&listing, &page), "{slug}");
    }
}

#[test]
fn a_listing_running_past_the_count_is_kept_when_kitsu_titles_agree() {
    // SPY×FAMILY's first cour: AniList lists both cours (25 entries)
    // on a 12-episode entry, and the first twelve are this show's.
    let (listing, page) = recorded("spy-family", 1);
    assert!(listing.entries.keys().any(|n| Some(*n) > listing.episodes));
    assert!(listing_fits(&listing, &page));
}

#[test]
fn titles_worded_differently_still_agree() {
    // "To You Two Thousand Years Later" against "To You, 2,000 Years
    // in the Future -The Fall of Zhiganshina (1)", and the like.
    let (listing, page) = recorded("attack-on-titan", 1);
    assert!(listing_fits(&listing, &page));
}

#[test]
fn words_every_title_of_the_show_shares_are_not_agreement() {
    // Kaguya-sama season 2 carries season 1's listing; nearly every
    // title on both sides is "Kaguya Wants to …".
    let (listing, page) = recorded("kaguya-2", 1);
    assert!(!listing_fits(&listing, &page));
}

#[test]
fn without_kitsu_titles_a_finished_show_refuses_a_listing_past_its_count() {
    // Mushoku Tensei part 1: 11 episodes, a listing of 0-24, no Kitsu
    // titles to compare.
    let (listing, page) = recorded("mushoku-tensei", 1);
    assert!(page.iter().all(|e| e.canonical_title.is_none()));
    assert!(!listing_fits(&listing, &page));
}

#[test]
fn an_ongoing_long_runner_keeps_its_listing() {
    let (listing, page) = recorded("one-piece", 4);
    assert_eq!(listing.episodes, None);
    assert!(listing_fits(&listing, &page));
}

// --- The rule's edges ---------------------------------------------

fn ep(number: u32, title: Option<&str>) -> KitsuEpisode {
    KitsuEpisode {
        id: format!("e{number}"),
        canonical_title: title.map(str::to_string),
        season_number: Some(1),
        number: Some(number),
        relative_number: Some(number),
        length: None,
        synopsis: None,
        airdate: None,
        thumbnail: None,
    }
}

fn listing(episodes: Option<u32>, finished: bool, titles: &[(u32, &str)]) -> StreamingListing {
    StreamingListing {
        episodes,
        finished,
        entries: titles
            .iter()
            .map(|(n, t)| {
                (
                    *n,
                    StreamingEntry {
                        title: (*t).to_string(),
                        thumbnail: format!("https://cr.cdn/{n}.jpg"),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>(),
    }
}

#[test]
fn a_minority_of_disagreeing_titles_does_not_refuse_the_listing() {
    let l = listing(
        Some(4),
        true,
        &[(1, "Alpha"), (2, "Bravo"), (3, "Charlie"), (4, "Delta")],
    );
    let page = [
        ep(1, Some("Alpha")),
        ep(2, Some("Bravo")),
        ep(3, Some("Something Else")),
        ep(4, Some("Another Thing")),
    ];
    assert!(listing_fits(&l, &page), "half agreeing keeps the listing");
    let page = [
        ep(1, Some("Alpha")),
        ep(2, Some("Unrelated")),
        ep(3, Some("Something Else")),
        ep(4, Some("Another Thing")),
    ];
    assert!(!listing_fits(&l, &page), "a quarter agreeing does not");
}

#[test]
fn fewer_than_three_compared_titles_leave_the_decision_to_the_count() {
    // Two disagreeing titles say nothing on their own: within the
    // count the listing is kept, past it on a finished show it is not.
    let page = [ep(1, Some("Gamma")), ep(2, Some("Omega"))];
    let within = listing(Some(12), true, &[(1, "Rho"), (2, "Sigma")]);
    assert!(listing_fits(&within, &page));
    let past = listing(Some(1), true, &[(1, "Rho"), (2, "Sigma")]);
    assert!(!listing_fits(&past, &page));
}

#[test]
fn a_count_says_nothing_until_the_show_has_finished() {
    let l = listing(Some(1), false, &[(1, "Rho"), (2, "Sigma")]);
    assert!(listing_fits(&l, &[ep(1, None)]));
    let l = listing(None, true, &[(1, "Rho"), (2, "Sigma")]);
    assert!(listing_fits(&l, &[ep(1, None)]));
}

#[test]
fn case_and_punctuation_do_not_count_as_disagreement() {
    let l = listing(
        Some(3),
        true,
        &[
            (1, "Departure x And x Friends"),
            (2, "TEST X OF X TESTS"),
            (3, "That Day - The Fall of Zhiganshina (2)"),
        ],
    );
    let page = [
        ep(1, Some("Departure × And × Friends")),
        ep(2, Some("Test × of × Tests")),
        ep(3, Some("That Day")),
    ];
    assert!(listing_fits(&l, &page));
}

proptest::proptest! {
    /// A listing whose titles are Kitsu's own always fits once enough
    /// of them are compared, whatever its length says.
    #[test]
    fn a_listing_with_kitsus_titles_fits(
        titles in proptest::collection::vec("[a-z]{3,8}( [a-z]{3,8}){0,3}", 3..20),
        count in proptest::option::of(0u32..30),
        finished in proptest::bool::ANY,
    ) {
        let pairs: Vec<(u32, &str)> =
            titles.iter().enumerate().map(|(i, t)| (i as u32 + 1, t.as_str())).collect();
        let l = listing(count, finished, &pairs);
        let page: Vec<KitsuEpisode> =
            pairs.iter().map(|(n, t)| ep(*n, Some(t))).collect();
        proptest::prop_assert!(listing_fits(&l, &page));
    }

    /// Three or more compared titles that share no word and contain
    /// none of each other refuse the listing, whatever its length says.
    #[test]
    fn titles_with_nothing_in_common_refuse_the_listing(
        ours in proptest::collection::vec("[a-m]{3,8}( [a-m]{3,8}){0,3}", 3..20),
        theirs in proptest::collection::vec("[n-z]{3,8}( [n-z]{3,8}){0,3}", 20),
        count in proptest::option::of(0u32..30),
        finished in proptest::bool::ANY,
    ) {
        let pairs: Vec<(u32, &str)> =
            theirs.iter().enumerate().map(|(i, t)| (i as u32 + 1, t.as_str())).collect();
        let l = listing(count, finished, &pairs);
        let page: Vec<KitsuEpisode> =
            ours.iter().enumerate().map(|(i, t)| ep(i as u32 + 1, Some(t))).collect();
        proptest::prop_assert!(!listing_fits(&l, &page));
    }

    /// With no Kitsu title to compare, a listing for a show that has
    /// not finished, or whose count is unknown, is always kept.
    #[test]
    fn without_titles_an_open_count_keeps_the_listing(
        numbers in proptest::collection::btree_set(0u32..200, 0..30),
        count in proptest::option::of(0u32..30),
        page_numbers in proptest::collection::vec(0u32..200, 0..20),
    ) {
        let pairs: Vec<(u32, &str)> = numbers.iter().map(|n| (*n, "Title")).collect();
        let page: Vec<KitsuEpisode> = page_numbers.iter().map(|n| ep(*n, None)).collect();
        proptest::prop_assert!(listing_fits(&listing(count, false, &pairs), &page));
        proptest::prop_assert!(listing_fits(&listing(None, true, &pairs), &page));
    }
}
