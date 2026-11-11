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

/// A film that names no parent is placed by the parent check of the
/// film that does, run with that film's own start: a named parent that
/// started first is its parent whatever the film lists beneath it.
#[test]
fn a_film_s_parent_is_checked_with_its_own_start_from_any_film() {
    let mut c = Catalogue::default();
    c.add("1", "TV", Some("2010-01-01"))
        .add("11", "movie", Some("2012-01-01"))
        .add("12", "movie", Some("2013-01-01"))
        .add("13", "special", Some("2014-01-01"));
    for f in ["11", "12"] {
        episodes(&mut c, f, Some(1));
    }
    c.rel("11", "sequel", "12")
        .rel("12", "prequel", "11")
        .rel("12", "parent_story", "1")
        .rel("12", "summary", "13");
    for anchor in ["11", "12"] {
        let f = drive(anchor, &c);
        assert_eq!(ids(&f.entries), ["1"], "from {anchor}");
        assert_eq!(ids(&f.films), ["11", "12"], "from {anchor}");
    }
}

/// A film that names a film of its own chain as its parent story is not
/// placed under it: the chain is walked from the series the films name.
#[test]
fn a_parent_inside_the_films_own_chain_is_no_parent() {
    let mut c = Catalogue::default();
    c.add("1", "TV", Some("2010-01-01"))
        .add("11", "movie", Some("2012-01-01"))
        .add("12", "movie", Some("2013-01-01"));
    for f in ["11", "12"] {
        episodes(&mut c, f, Some(1));
    }
    c.rel("11", "sequel", "12")
        .rel("12", "prequel", "11")
        .rel("11", "parent_story", "1")
        .rel("12", "parent_story", "11");
    for anchor in ["11", "12"] {
        let f = drive(anchor, &c);
        assert_eq!(ids(&f.entries), ["1"], "from {anchor}");
        assert_eq!(ids(&f.films), ["11", "12"], "from {anchor}");
        assert_eq!(f.parents.get(anchor).cloned(), Some(vec!["1".to_string()]));
    }
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

/// Which entry a film names as its parent story: none, a season, or
/// another film of its own chain.
#[derive(Debug, Clone, Copy)]
enum Named {
    Nothing,
    Season(usize),
    Film(usize),
}

/// A season chain and a film chain, each started in any year, each
/// listing recaps beneath it (a season at least as many as any film,
/// as a series is the hub its films hang off), and which entry each
/// film names as its parent story, at least one film naming a season.
/// Seasons list none of the films.
fn hero_like() -> BoxedStrategy<Catalogue> {
    (1usize..5, 1usize..5)
        .prop_flat_map(|(seasons, films)| {
            let named = prop_oneof![
                Just(Named::Nothing),
                (0..seasons).prop_map(Named::Season),
                (0..films).prop_map(Named::Film),
            ];
            (
                prop::collection::vec(1990u16..2030, seasons),
                prop::collection::vec(1990u16..2030, films),
                prop::collection::vec(named, films),
                prop::collection::vec(0usize..3, films),
            )
        })
        .prop_filter("a film names a season", |(_, _, named, _)| {
            named.iter().any(|n| matches!(n, Named::Season(_)))
        })
        .prop_map(|(season_years, film_years, named, film_recaps)| {
            let s = |i: usize| format!("{}", 100 + i);
            let f = |i: usize| format!("{}", 200 + i);
            let mut c = Catalogue::default();
            let mut recap = 300;
            let mut recaps = |c: &mut Catalogue, of: &str, n: usize| {
                for _ in 0..n {
                    recap += 1;
                    let id = recap.to_string();
                    c.add(&id, "special", Some("2031-01-01"));
                    c.rel(of, "summary", &id);
                }
            };
            for (i, y) in season_years.iter().enumerate() {
                c.add(&s(i), "TV", Some(&format!("{y}-01-01")));
            }
            for (i, y) in film_years.iter().enumerate() {
                c.add(&f(i), "movie", Some(&format!("{y}-06-01")));
                episodes(&mut c, &f(i), Some(1));
            }
            for i in 1..season_years.len() {
                c.rel(&s(i - 1), "sequel", &s(i))
                    .rel(&s(i), "prequel", &s(i - 1));
            }
            for i in 1..film_years.len() {
                c.rel(&f(i - 1), "sequel", &f(i))
                    .rel(&f(i), "prequel", &f(i - 1));
            }
            for i in 0..season_years.len() {
                recaps(&mut c, &s(i), 3);
            }
            for (i, n) in film_recaps.iter().enumerate() {
                recaps(&mut c, &f(i), *n);
            }
            for (i, n) in named.iter().enumerate() {
                match n {
                    Named::Nothing => {}
                    Named::Season(p) => {
                        c.rel(&f(i), "parent_story", &s(*p));
                    }
                    Named::Film(p) => {
                        c.rel(&f(i), "parent_story", &f(*p));
                    }
                }
            }
            c
        })
        .boxed()
}

/// The ids of `cat`'s entries of `subtype`, sorted.
fn of_subtype<'a>(cat: &'a Catalogue, subtype: &str) -> Vec<&'a str> {
    let mut out: Vec<&str> = cat
        .entries
        .values()
        .filter(|e| e.subtype.as_deref() == Some(subtype))
        .map(|e| e.id.as_str())
        .collect();
    out.sort_unstable();
    out
}

fn sorted(entries: &[FranchiseEntry]) -> Vec<&str> {
    let mut out = ids(entries);
    out.sort_unstable();
    out
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    /// From every film of a film chain that names a season chain its
    /// parent, the answer is the same: the seasons are the main story,
    /// every film is under Films, and the film opened is placed under
    /// seasons only, whichever film it is, whatever the years, and
    /// whatever the films name or list beneath them.
    #[test]
    fn films_naming_a_series_are_never_its_main_story(cat in hero_like()) {
        let seasons = of_subtype(&cat, "TV");
        let films = of_subtype(&cat, "movie");
        for anchor in &films {
            let f = drive(anchor, &cat);
            prop_assert_eq!(sorted(&f.entries), seasons.clone(), "from {}", anchor);
            prop_assert_eq!(sorted(&f.films), films.clone(), "from {}", anchor);
            let parents = f.parents.get(*anchor).cloned().unwrap_or_default();
            prop_assert!(!parents.is_empty(), "from {}", anchor);
            prop_assert!(parents.iter().all(|p| seasons.contains(&p.as_str())));
        }
    }
}
