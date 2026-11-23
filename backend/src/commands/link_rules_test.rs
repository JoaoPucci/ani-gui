//! The shows a guessed history row can be linked to, and the warnings a
//! link to one of them earns.

use super::*;
use proptest::prelude::*;
use std::collections::HashMap;

fn show(id: &str) -> KitsuAnimeRef {
    KitsuAnimeRef {
        id: id.into(),
        canonical_title: format!("Show {id}"),
        titles: HashMap::new(),
        abbreviated_titles: Vec::new(),
        slug: Some(format!("show-{id}")),
        synopsis: None,
        start_date: None,
        end_date: None,
        episode_count: None,
        average_rating: None,
        subtype: Some("TV".into()),
        status: Some("finished".into()),
        age_rating: None,
        popularity_rank: None,
        poster_image: None,
        anilist_banner: None,
        cover_image: None,
    }
}

fn titled(id: &str, canonical: &str, slug: &str) -> KitsuAnimeRef {
    KitsuAnimeRef {
        canonical_title: canonical.into(),
        slug: Some(slug.into()),
        ..show(id)
    }
}

fn counted(id: &str, count: Option<u32>) -> KitsuAnimeRef {
    KitsuAnimeRef {
        episode_count: count,
        ..show(id)
    }
}

fn ids(out: &[(KitsuAnimeRef, bool)]) -> Vec<(&str, bool)> {
    out.iter().map(|(s, c)| (s.id.as_str(), *c)).collect()
}

#[test]
fn the_show_shown_now_leads_then_the_two_searches_interleave() {
    let out = merge_candidates(
        Some(&show("7158")),
        &[show("7158"), show("41410"), show("8739")],
        &[show("7730"), show("41410")],
    );
    assert_eq!(
        ids(&out),
        [
            ("7158", true),
            ("7730", false),
            ("41410", false),
            ("8739", false)
        ]
    );
}

#[test]
fn no_music_video_is_offered() {
    let music = KitsuAnimeRef {
        subtype: Some("Music".into()),
        ..show("1")
    };
    let out = merge_candidates(None, &[music, show("2")], &[]);
    assert_eq!(ids(&out), [("2", false)]);
}

#[test]
fn a_season_the_saved_title_names_and_the_show_does_not_is_pointed_out() {
    let gintama = titled("10083", "Gintama°", "gintama-2015-tv");
    assert_eq!(
        link_warnings("Gintama Season 4", &gintama, 1.0),
        [LinkWarning::Season]
    );
}

#[test]
fn a_show_carrying_the_same_season_by_title_or_slug_earns_none() {
    let stone_ocean = titled(
        "46598",
        "JoJo no Kimyou na Bouken: Stone Ocean Part 3",
        "jojo-no-kimyou-na-bouken-stone-ocean-part-3",
    );
    assert_eq!(
        link_warnings(
            "JoJo's Bizarre Adventure: Stone Ocean Part 3",
            &stone_ocean,
            1.0
        ),
        []
    );
    let by_slug = titled("5", "Show II", "show-season-2");
    assert_eq!(link_warnings("Show Season 2", &by_slug, 1.0), []);
}

#[test]
fn a_shows_slug_is_read_as_the_backend_reads_every_kitsu_slug() {
    // The number before the keyword, as Kitsu writes `…-2nd-season` and
    // the page's own slug matcher reads it: the second season.
    assert_eq!(season_of(&titled("1", "Show", "show-2-season")), 2);
    assert_eq!(season_of(&titled("1", "Show", "show-2nd-season")), 2);
    assert_eq!(season_of(&titled("1", "Show", "show-second-season")), 2);
    assert_eq!(
        link_warnings("Show Season 2", &titled("1", "Show", "show-2-season"), 1.0),
        []
    );
    // Kitsu slugs keep every word apart: a keyword glued to its number
    // names no season.
    assert_eq!(season_of(&titled("1", "Show", "show-season2")), 1);
}

#[test]
fn a_season_any_of_the_shows_titles_carries_counts() {
    let mut second = titled("5", "Shingeki", "shingeki");
    second
        .titles
        .insert("en".into(), "Attack on Titan Season 2".into());
    assert_eq!(link_warnings("Attack on Titan Season 2", &second, 1.0), []);
    assert_eq!(season_of(&second), 2);
}

#[test]
fn an_episode_past_the_shows_count_is_pointed_out() {
    assert_eq!(
        link_warnings("Naruto: Shippuden", &counted("12625", Some(1)), 426.0),
        [LinkWarning::Episode]
    );
    assert_eq!(
        link_warnings("Naruto: Shippuden", &counted("1555", Some(500)), 426.0),
        []
    );
    assert_eq!(
        link_warnings("Naruto: Shippuden", &counted("1555", None), 426.0),
        []
    );
}

#[test]
fn the_episode_measured_is_the_one_the_card_offers() {
    // The row saved episode 1 and the card offers 2: the one-episode
    // special is too short for the card.
    assert_eq!(
        link_warnings("Black Clover", &counted("12625", Some(1)), 2.0),
        [LinkWarning::Episode]
    );
    assert_eq!(
        link_warnings("Black Clover", &counted("12625", Some(1)), 1.0),
        []
    );
}

#[test]
fn both_warnings_come_in_the_order_the_picker_words_them() {
    let special = KitsuAnimeRef {
        episode_count: Some(1),
        ..titled("1", "Gintama", "gintama")
    };
    assert_eq!(
        link_warnings("Gintama Season 4", &special, 3.0),
        [LinkWarning::Season, LinkWarning::Episode]
    );
}

#[test]
fn every_candidate_carries_its_own_warnings() {
    let out = with_warnings(
        vec![
            (counted("1", Some(1)), true),
            (counted("2", Some(9)), false),
        ],
        "Show",
        2.0,
    );
    let got: Vec<(&str, bool, Vec<LinkWarning>)> = out
        .iter()
        .map(|c| (c.show.id.as_str(), c.current, c.warnings.clone()))
        .collect();
    assert_eq!(
        got,
        [
            ("1", true, vec![LinkWarning::Episode]),
            ("2", false, vec![])
        ]
    );
}

#[test]
fn a_candidate_serializes_as_the_renderer_reads_it() {
    let out = with_warnings(vec![(counted("1", Some(1)), true)], "Show Season 2", 2.0);
    let wire = serde_json::to_value(&out).unwrap();
    assert_eq!(wire[0]["ref"]["id"], "1");
    assert_eq!(wire[0]["current"], true);
    assert_eq!(
        wire[0]["warnings"],
        serde_json::json!(["season", "episode"])
    );
}

fn id() -> impl Strategy<Value = String> {
    proptest::sample::select(&["1", "2", "3", "4", "5"][..]).prop_map(str::to_string)
}

proptest! {
    #[test]
    fn each_show_is_offered_once_every_one_searched_the_current_first(
        cur in proptest::option::of(proptest::sample::select(&["1", "2", "9"][..])),
        a in proptest::collection::vec(id(), 0..8),
        b in proptest::collection::vec(id(), 0..8),
    ) {
        let current = cur.map(show);
        let title: Vec<KitsuAnimeRef> = a.iter().map(|i| show(i)).collect();
        let slug: Vec<KitsuAnimeRef> = b.iter().map(|i| show(i)).collect();
        let out = merge_candidates(current.as_ref(), &title, &slug);
        let got: Vec<&str> = out.iter().map(|(s, _)| s.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = got.iter().copied().collect();
        prop_assert_eq!(unique.len(), got.len());
        let mut asked: std::collections::HashSet<&str> =
            a.iter().chain(b.iter()).map(String::as_str).collect();
        if let Some(c) = cur {
            asked.insert(c);
        }
        prop_assert_eq!(unique, asked);
        prop_assert_eq!(out.iter().filter(|(_, c)| *c).count(), usize::from(cur.is_some()));
        if let Some(c) = cur {
            prop_assert_eq!(out[0].0.id.as_str(), c);
            prop_assert!(out[0].1);
        }
    }

    #[test]
    fn the_episode_warning_is_exactly_an_episode_past_a_known_count(
        count in proptest::option::of(0u32..2000),
        episode in 0.0f64..3000.0,
    ) {
        let warned = link_warnings("Show", &counted("1", count), episode)
            .contains(&LinkWarning::Episode);
        prop_assert_eq!(warned, count.is_some_and(|n| episode > f64::from(n)));
    }

    #[test]
    fn the_season_warning_is_exactly_a_season_the_show_does_not_name(
        row_season in 1u32..6,
        show_season in 1u32..6,
    ) {
        let title = if row_season == 1 { "Show".to_string() } else { format!("Show Season {row_season}") };
        let slug = if show_season == 1 { "show".to_string() } else { format!("show-season-{show_season}") };
        let warned = link_warnings(&title, &titled("1", "Show", &slug), 1.0)
            .contains(&LinkWarning::Season);
        prop_assert_eq!(warned, row_season != show_season);
    }
}
