//! A run of one-offs is not a main story when it names a parent story:
//! My Hero Academia's four films are linked to each other by sequel,
//! and to the TV seasons only as parent story or other, so a walk that
//! stopped at the links would make the films the run and the seasons
//! its side stories. A run that holds no series (no TV, OVA or ONA of
//! more than one episode) is walked from the earliest parent story its
//! entries name instead, and the anchor is listed around that run: a
//! film under Films.

use proptest::prelude::*;

use super::tests::{drive, Catalogue};
use super::*;

fn ids(entries: &[FranchiseEntry]) -> Vec<&str> {
    entries.iter().map(|e| e.id.as_str()).collect()
}

/// `id` with `episodes` episodes, before anything relates it.
fn episodes(c: &mut Catalogue, id: &str, n: Option<u32>) {
    c.entries.get_mut(id).expect("entry").episodes = n;
}

/// My Hero Academia's shape: three seasons by sequel, three films by
/// sequel; the first film names the second season its parent story,
/// the second names the third season as other (and the third season
/// it), the third names the third season its parent story.
fn hero() -> Catalogue {
    let mut c = Catalogue::default();
    c.add("1", "TV", Some("2016-04-03"))
        .add("2", "TV", Some("2017-04-01"))
        .add("3", "TV", Some("2018-04-07"))
        .add("11", "movie", Some("2018-08-03"))
        .add("12", "movie", Some("2019-12-20"))
        .add("13", "movie", Some("2021-08-06"));
    for f in ["11", "12", "13"] {
        episodes(&mut c, f, Some(1));
    }
    c.rel("1", "sequel", "2")
        .rel("2", "prequel", "1")
        .rel("2", "sequel", "3")
        .rel("3", "prequel", "2")
        .rel("2", "side_story", "11")
        .rel("3", "side_story", "13")
        .rel("3", "other", "12")
        .rel("11", "parent_story", "2")
        .rel("11", "sequel", "12")
        .rel("12", "prequel", "11")
        .rel("12", "other", "3")
        .rel("12", "sequel", "13")
        .rel("13", "prequel", "12")
        .rel("13", "parent_story", "3");
    c
}

#[test]
fn from_a_film_the_seasons_are_the_main_story_and_the_film_is_under_films() {
    let f = drive("11", &hero());
    assert_eq!(ids(&f.entries), ["1", "2", "3"]);
    assert_eq!(ids(&f.films), ["11", "12", "13"]);
    assert!(f.sides.is_empty());
    assert_eq!(f.parents.get("11").cloned(), Some(vec!["2".to_string()]));
}

/// A film that names no parent story of its own is placed under the
/// parent stories the films linked to it name.
#[test]
fn a_film_that_names_no_parent_takes_the_parents_its_films_name() {
    let f = drive("12", &hero());
    assert_eq!(ids(&f.entries), ["1", "2", "3"]);
    assert_eq!(ids(&f.films), ["11", "12", "13"]);
    assert_eq!(
        f.parents.get("12").cloned(),
        Some(vec!["2".to_string(), "3".to_string()])
    );
}

#[test]
fn from_a_season_the_answer_is_unchanged() {
    let f = drive("1", &hero());
    assert_eq!(ids(&f.entries), ["1", "2", "3"]);
    assert_eq!(ids(&f.films), ["11", "12", "13"]);
    assert!(f.parents.is_empty());
}

/// Kara no Kyoukai's shape: films linked by sequel that name no parent
/// story are the main story.
#[test]
fn films_that_name_no_parent_story_are_the_main_story() {
    let mut c = Catalogue::default();
    c.add("1", "movie", Some("2007-12-01"))
        .add("2", "movie", Some("2007-12-29"));
    c.rel("1", "sequel", "2").rel("2", "prequel", "1");
    let f = drive("2", &c);
    assert_eq!(ids(&f.entries), ["1", "2"]);
    assert!(f.parents.is_empty());
}

/// A run that holds a series is the main story even when one of its
/// films names a parent story: Berserk's films lead into its 2016
/// series, Boruto's film sits among the series.
#[test]
fn a_run_holding_a_series_is_the_main_story_whatever_its_films_name() {
    let mut c = hero();
    c.add("20", "TV", Some("2022-01-01"));
    c.rel("13", "sequel", "20").rel("20", "prequel", "13");
    let f = drive("11", &c);
    assert_eq!(ids(&f.entries), ["11", "12", "13", "20"]);
    assert!(f.parents.is_empty());
}

/// What counts as a series: a TV series, OVA or ONA of more than one
/// episode, or whose count Kitsu does not know yet; a one-episode OVA,
/// a special or a film is a one-off whatever its count.
#[test]
fn a_run_of_one_episode_ovas_is_a_run_of_one_offs_and_an_ova_of_unknown_count_a_series() {
    let shape = |count: Option<u32>| {
        let mut c = Catalogue::default();
        c.add("1", "TV", Some("2010-01-01"))
            .add("2", "OVA", Some("2011-01-01"))
            .add("3", "OVA", Some("2012-01-01"));
        episodes(&mut c, "2", count);
        episodes(&mut c, "3", Some(1));
        c.rel("2", "parent_story", "1")
            .rel("2", "sequel", "3")
            .rel("3", "prequel", "2");
        c
    };
    let f = drive("2", &shape(Some(1)));
    assert_eq!(ids(&f.entries), ["1"]);
    assert_eq!(ids(&f.sides), ["2"]);
    for count in [None, Some(0), Some(2)] {
        let f = drive("2", &shape(count));
        assert_eq!(ids(&f.entries), ["2", "3"], "{count:?}");
    }
}

/// A season chain, a film chain, and which films name which seasons
/// as their parent story (at least one); the anchor is a film.
fn hero_like() -> BoxedStrategy<(Catalogue, String)> {
    (1usize..5, 1usize..5)
        .prop_flat_map(|(seasons, films)| {
            (
                Just(seasons),
                Just(films),
                prop::collection::vec(prop::option::of(0..seasons), films),
                0..films,
            )
        })
        .prop_filter("a film names a parent", |(_, _, named, _)| {
            named.iter().any(Option::is_some)
        })
        .prop_map(|(seasons, films, named, anchor)| {
            let s = |i: usize| format!("{}", 100 + i);
            let f = |i: usize| format!("{}", 200 + i);
            let mut c = Catalogue::default();
            for i in 0..seasons {
                c.add(&s(i), "TV", Some(&format!("{}-01-01", 2000 + i)));
            }
            for i in 0..films {
                c.add(&f(i), "movie", Some(&format!("{}-06-01", 2000 + i)));
                episodes(&mut c, &f(i), Some(1));
            }
            for i in 1..seasons {
                c.rel(&s(i - 1), "sequel", &s(i))
                    .rel(&s(i), "prequel", &s(i - 1));
            }
            for i in 1..films {
                c.rel(&f(i - 1), "sequel", &f(i))
                    .rel(&f(i), "prequel", &f(i - 1));
            }
            for (i, p) in named.iter().enumerate() {
                if let Some(p) = p {
                    c.rel(&f(i), "parent_story", &s(*p));
                }
            }
            (c, f(anchor))
        })
        .boxed()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// From any film of a film chain that names a season chain its
    /// parent, the seasons are the main story, every film is under
    /// Films, and the anchor is placed under seasons only.
    #[test]
    fn films_naming_a_series_are_never_its_main_story((cat, anchor) in hero_like()) {
        let f = drive(&anchor, &cat);
        let seasons: Vec<&str> = cat.entries.values()
            .filter(|e| e.subtype.as_deref() == Some("TV"))
            .map(|e| e.id.as_str())
            .collect();
        let mut got = ids(&f.entries);
        got.sort_unstable();
        let mut want = seasons.clone();
        want.sort_unstable();
        prop_assert_eq!(got, want);
        prop_assert!(f.films.iter().any(|e| e.id == anchor));
        let parents = f.parents.get(&anchor).cloned().unwrap_or_default();
        prop_assert!(!parents.is_empty());
        prop_assert!(parents.iter().all(|p| seasons.contains(&p.as_str())));
    }
}
