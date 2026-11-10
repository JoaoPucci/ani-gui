//! Properties of the rules a Continue row's guess rules hits out by.

use super::*;
use proptest::prelude::*;
use std::collections::HashMap;

fn hit(canonical: &str, subtype: &str, count: Option<u32>) -> KitsuAnimeRef {
    KitsuAnimeRef {
        id: "1".into(),
        canonical_title: canonical.into(),
        titles: HashMap::new(),
        abbreviated_titles: Vec::new(),
        slug: None,
        synopsis: None,
        start_date: None,
        end_date: None,
        episode_count: count,
        average_rating: None,
        subtype: Some(subtype.into()),
        status: None,
        age_rating: None,
        popularity_rank: None,
        poster_image: None,
        cover_image: None,
        anilist_banner: None,
    }
}

/// A title of plain words, at times ending in a season or a part.
fn title() -> impl Strategy<Value = String> {
    (
        prop::collection::vec("[a-z]{1,8}", 1..5),
        prop::option::of((
            prop::sample::select(vec!["Season", "Part", "Cour"]),
            1u32..12,
        )),
    )
        .prop_map(|(words, tail)| {
            let mut t = words.join(" ");
            if let Some((kw, n)) = tail {
                t.push_str(&format!(" {kw} {n}"));
            }
            t
        })
}

fn subtype() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec!["TV", "ONA", "OVA", "movie", "special", "music"])
}

proptest! {
    /// A row within a hit's count is never past it.
    #[test]
    fn a_row_within_the_count_is_never_past(count in 0u32..2000, frac in 0.0f64..=1.0) {
        prop_assert!(!past_reach(f64::from(count) * frac, Some(count)));
    }

    /// Once past a hit, a row further on stays past it.
    #[test]
    fn past_reach_holds_further_on(count in 0u32..2000, e in 0.0f64..5000.0, more in 0.0f64..5000.0) {
        if past_reach(e, Some(count)) {
            prop_assert!(past_reach(e + more, Some(count)));
        }
    }

    /// A row further on rules out every hit an earlier one did: only
    /// the episode moves, and only the reach reads it.
    #[test]
    fn ruled_out_holds_further_on(
        t in title(),
        sub in subtype(),
        count in prop::option::of(0u32..300),
        e in 0.0f64..600.0,
        more in 0.0f64..600.0,
    ) {
        let h = hit("Something Else", sub, count);
        if ruled_out(&t, Some(e), &h) {
            prop_assert!(ruled_out(&t, Some(e + more), &h));
        }
    }

    /// A series titled exactly as the row, with no count, is never
    /// ruled out: it is the show the row names, unless the title names
    /// a movie.
    #[test]
    fn the_series_the_row_names_is_never_ruled_out(
        t in title(),
        sub in prop::sample::select(vec!["TV", "ONA"]),
        e in 0.0f64..10_000.0,
    ) {
        prop_assume!(!names_movie(&t));
        prop_assert!(!ruled_out(&t, Some(e), &hit(&t, sub, None)));
    }

    /// The first-word rule never refuses a hit titled as the row.
    #[test]
    fn a_hit_titled_as_the_row_carries_its_first_word(t in title(), sub in subtype()) {
        prop_assert!(carries_first_word(&t, &hit(&t, sub, None)));
    }

    /// A title that does not carry "tv" as a word names no format, so
    /// the series rule refuses nothing for it.
    #[test]
    fn only_a_tv_word_names_a_series(t in title()) {
        let named = t.to_ascii_lowercase().split(' ').any(|w| w == "tv");
        prop_assert_eq!(names_series(&t), named);
    }

    /// `ruled_out_ids` is the hits `ruled_out` or `ruled_out_by_key`
    /// refuses, in their order.
    #[test]
    fn ruled_out_ids_lists_the_refused_hits_in_order(
        t in title(),
        k in title(),
        e in 0.0f64..100.0,
        hits in prop::collection::vec((title(), subtype(), prop::option::of(0u32..50)), 0..8),
    ) {
        let hits: Vec<KitsuAnimeRef> = hits
            .iter()
            .enumerate()
            .map(|(i, (c, s, n))| KitsuAnimeRef { id: i.to_string(), ..hit(c, s, *n) })
            .collect();
        let want: Vec<String> = hits
            .iter()
            .filter(|h| ruled_out(&t, Some(e), h) || ruled_out_by_key(&t, &k, h))
            .map(|h| h.id.clone())
            .collect();
        prop_assert_eq!(ruled_out_ids(&t, &k, Some(e), &hits), want);
    }

    /// A title that does not carry "movie" as a word names no movie.
    #[test]
    fn only_a_movie_word_names_a_movie(t in title()) {
        let named = t.to_ascii_lowercase().split(' ').any(|w| w == "movie");
        prop_assert_eq!(names_movie(&t), named);
    }

    /// A movie titled exactly as the row, with no count, is never ruled
    /// out, by the title or by a key of the same words: it is the movie
    /// the row names.
    #[test]
    fn the_movie_the_row_names_is_never_ruled_out(
        words in prop::collection::vec("[a-z]{1,8}", 1..4),
        n in prop::option::of(0u32..30),
        e in 0.0f64..10_000.0,
    ) {
        let t = match n {
            Some(n) => format!("{} Movie {n}", words.join(" ")),
            None => format!("{} Movie", words.join(" ")),
        };
        prop_assume!(!names_series(&t));
        let h = hit(&t, "movie", None);
        prop_assert!(!ruled_out(&t, Some(e), &h));
        prop_assert!(!ruled_out_by_key(&t, &t, &h));
    }

    /// A hit that numbers no movie is never numbered as another.
    #[test]
    fn a_hit_numbering_no_movie_is_never_another(t in title(), c in title(), n in 0u32..30) {
        let row = format!("{t} Movie {n}");
        prop_assert!(!numbered_as_another_movie(&row, &hit(&c, "movie", Some(1))));
    }

    /// A hit one of whose titles numbers the row's movie is never
    /// another, whatever else it numbers.
    #[test]
    fn a_hit_numbering_the_row_s_movie_is_never_another(
        t in title(),
        n in 0u32..30,
        others in prop::collection::vec(0u32..30, 0..4),
    ) {
        let row = format!("{t} Movie {n}");
        let mut h = hit(&format!("Show Movie {n}"), "movie", Some(1));
        h.abbreviated_titles = others.iter().map(|m| format!("Show Movie {m}")).collect();
        prop_assert!(!numbered_as_another_movie(&row, &h));
    }

    /// A row whose title or key names a recap refuses no hit for being
    /// one.
    #[test]
    fn a_row_naming_a_recap_refuses_no_recap(
        t in title(),
        k in title(),
        in_title in any::<bool>(),
        c in title(),
    ) {
        let (t, k) = if in_title { (format!("{t} Recap"), k) } else { (t, format!("{k} recaps")) };
        let h = hit(&format!("{c} Recap"), "special", Some(1));
        prop_assert!(!a_recap_the_row_does_not_name(&t, &k, &h));
    }

    /// A season is read off "Season N" and "Nth Season" wherever it
    /// stands in the words.
    #[test]
    fn a_season_is_read_wherever_it_stands(
        before in prop::collection::vec("[a-z]{1,8}", 0..3),
        after in prop::collection::vec("[a-z]{1,8}", 0..3),
        n in 1u32..40,
        ordinal in any::<bool>(),
    ) {
        prop_assume!(!before.iter().chain(after.iter()).any(|w| w == "season"));
        let s = if ordinal {
            let suffix = match (n % 100, n % 10) {
                (11..=13, _) => "th",
                (_, 1) => "st",
                (_, 2) => "nd",
                (_, 3) => "rd",
                _ => "th",
            };
            format!("{n}{suffix} Season")
        } else {
            format!("Season {n}")
        };
        let text = [before.join(" "), s, after.join(" ")].join(" ");
        prop_assert_eq!(season_named(&text), Some(n));
    }

    /// A title that names a season is judged by its own season, never by
    /// its key's.
    #[test]
    fn a_title_naming_a_season_is_never_judged_by_its_key(
        t in title(),
        n in 1u32..12,
        k in title(),
        m in 2u32..12,
        c in title(),
    ) {
        let row = format!("{t} Season {n}");
        let key = format!("{k} season {m}");
        prop_assert!(!key_season::a_season_the_key_names_and_the_hit_does_not(
            &row,
            &key,
            &hit(&c, "TV", None)
        ));
    }

    /// A hit naming the season the key names, anywhere in a title, is
    /// never refused by the key's season.
    #[test]
    fn a_hit_naming_the_key_s_season_is_never_refused_by_it(
        k in prop::collection::vec("[a-z]{1,8}", 1..4),
        m in 2u32..12,
        c in prop::collection::vec("[a-z]{1,8}", 1..3),
        tail in prop::collection::vec("[a-z]{1,8}", 0..3),
    ) {
        prop_assume!(!k.iter().any(|w| w == "season"));
        let key = format!("{} season {m} extras", k.join(" "));
        let h = hit(&format!("{} Season {m} {}", c.join(" "), tail.join(" ")), "special", None);
        prop_assert!(!key_season::a_season_the_key_names_and_the_hit_does_not("Show: Extras", &key, &h));
    }

    /// A row whose episode says nothing (a bare key's) is judged as one
    /// at no episode: only the reach reads the episode.
    #[test]
    fn no_episode_is_the_same_as_none_reached(
        t in title(),
        sub in subtype(),
        count in prop::option::of(0u32..300),
    ) {
        let h = hit("Something Else", sub, count);
        prop_assert_eq!(ruled_out(&t, None, &h), ruled_out(&t, Some(0.0), &h));
    }

    /// A bare key's answer does not depend on the episode it sends.
    #[test]
    fn a_bare_key_s_answer_ignores_the_episode(
        t in title(),
        e in 0.0f64..5000.0,
        hits in prop::collection::vec((title(), subtype(), prop::option::of(0u32..50)), 0..6),
    ) {
        let req = |episode: f64| RuledOutRequest {
            key: "show-part-2-3687".into(),
            title: t.clone(),
            episode,
            hits: hits
                .iter()
                .enumerate()
                .map(|(i, (c, s, n))| SentHit {
                    id: i.to_string(),
                    canonical_title: c.clone(),
                    titles: HashMap::new(),
                    abbreviated_titles: Vec::new(),
                    slug: None,
                    episode_count: *n,
                    subtype: Some((*s).to_string()),
                })
                .collect(),
        };
        prop_assert_eq!(answer(req(e)), answer(req(0.0)));
    }
}
