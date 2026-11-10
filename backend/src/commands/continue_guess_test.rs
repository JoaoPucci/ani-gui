//! The rules a Continue row's guess rules a search hit out by, one
//! case each, with the hits the corpus recorded where a case comes
//! from one.

use super::*;
use std::collections::HashMap;

fn hit(id: &str, canonical: &str, slug: &str, subtype: &str, count: Option<u32>) -> KitsuAnimeRef {
    KitsuAnimeRef {
        id: id.into(),
        canonical_title: canonical.into(),
        titles: HashMap::new(),
        abbreviated_titles: Vec::new(),
        slug: Some(slug.into()),
        synopsis: None,
        start_date: None,
        end_date: None,
        episode_count: count,
        average_rating: None,
        subtype: Some(subtype.into()),
        status: Some("finished".into()),
        age_rating: None,
        popularity_rank: None,
        poster_image: None,
        cover_image: None,
        anilist_banner: None,
    }
}

fn titled(mut h: KitsuAnimeRef, en: &str) -> KitsuAnimeRef {
    h.titles.insert("en".into(), en.into());
    h
}

// — How far the row got ——————————————————————————————————————————

#[test]
fn a_row_past_twice_the_count_and_two_is_past_the_hit() {
    assert!(past_reach(24.0, Some(1)));
    assert!(past_reach(5.0, Some(1)));
    assert!(!past_reach(4.0, Some(1)));
    assert!(past_reach(27.0, Some(12)));
    assert!(!past_reach(26.0, Some(12)));
}

#[test]
fn a_hit_with_no_count_is_never_past() {
    assert!(!past_reach(1180.0, None));
}

#[test]
fn a_listing_spanning_two_cours_keeps_the_first() {
    // hianime's "Attack on Titan Season 3" runs 22 episodes, Kitsu's
    // Season 3 12: the row at its last episode still names that entry.
    let s3 = hit(
        "13569",
        "Attack on Titan Season 3",
        "shingeki-no-kyojin-season-3",
        "TV",
        Some(12),
    );
    assert!(!ruled_out("Attack on Titan Season 3", Some(22.0), &s3));
}

#[test]
fn a_recap_is_out_for_a_row_past_it() {
    let recap = titled(
        hit(
            "47880",
            "Jujutsu Kaisen 2nd Season Recap",
            "jujutsu-kaisen-2nd-season-recap",
            "special",
            Some(2),
        ),
        "Jujutsu Kaisen 2nd Season Recap",
    );
    assert!(ruled_out("Jujutsu Kaisen 2nd Season", Some(23.0), &recap));
}

#[test]
fn a_fractional_episode_reads_as_its_number() {
    assert!(!past_reach(4.5, Some(2)));
    assert!(past_reach(6.5, Some(2)));
}

// — The format the title names ——————————————————————————————————

#[test]
fn tv_as_a_word_names_a_series() {
    assert!(names_series("Jujutsu Kaisen (TV)"));
    assert!(names_series("HenSemi TV"));
    assert!(!names_series("Jujutsu Kaisen"));
    // A word that starts with tv is not the word.
    assert!(!names_series("TVnimation Special"));
}

#[test]
fn tv_and_ona_are_series() {
    assert!(is_series(Some("TV")));
    assert!(is_series(Some("ONA")));
    assert!(is_series(Some("tv")));
    for other in ["special", "movie", "OVA", "music"] {
        assert!(!is_series(Some(other)), "{other}");
    }
    assert!(!is_series(None));
}

#[test]
fn a_series_title_rules_out_a_special() {
    let special = titled(
        hit(
            "43886",
            "Jujutsu Kaisen New Year's Special",
            "jujutsu-kaisen-new-year-s-special",
            "special",
            Some(1),
        ),
        "Jujutsu Kaisen New Year's Special",
    );
    assert!(ruled_out("Jujutsu Kaisen (TV)", Some(1.0), &special));
    // Without the mark, the special stands at the row's first episode.
    assert!(!ruled_out("Jujutsu Kaisen", Some(1.0), &special));
}

// — The title's first word ——————————————————————————————————————

#[test]
fn a_hit_carrying_the_first_word_in_any_title_stands() {
    let kaiju = titled(
        hit("46300", "Kaijuu 8-gou", "kaijuu-8-gou", "TV", Some(12)),
        "Kaiju No. 8",
    );
    assert!(carries_first_word("Kaiju No. 8", &kaiju));
    // From the slug's words alone.
    let slugged = hit("3919", "化物語", "bakemonogatari", "TV", Some(15));
    assert!(carries_first_word(
        "Bakemonogatari (The Monogatari Series)",
        &slugged
    ));
}

#[test]
fn a_hit_none_of_whose_titles_carries_the_first_word_is_out() {
    // hianime's "Bakemonogatari (The Monogatari Series)" brings back
    // Off & Monster Season alone.
    let off = titled(
        hit(
            "48434",
            "Monogatari Series: Off & Monster Season",
            "monogatari-series-off-monster-season",
            "ONA",
            Some(14),
        ),
        "Monogatari Series: Off & Monster Season",
    );
    assert!(ruled_out(
        "Bakemonogatari (The Monogatari Series)",
        Some(1.0),
        &off
    ));
}

#[test]
fn an_article_is_not_a_first_word() {
    let diaries = titled(
        hit(
            "47083",
            "Kusuriya no Hitorigoto",
            "kusuriya-no-hitorigoto",
            "TV",
            Some(24),
        ),
        "The Apothecary Diaries",
    );
    assert!(carries_first_word("The Apothecary Diaries", &diaries));
}

#[test]
fn a_title_with_no_words_rules_nothing_out() {
    let any = hit("1", "Anything", "anything", "TV", Some(12));
    assert!(carries_first_word("！？", &any));
    assert!(!ruled_out("", Some(1.0), &any));
}

// — A later season ——————————————————————————————————————————————

#[test]
fn a_season_row_takes_a_hit_ending_in_its_season_in_any_form() {
    let s2 = titled(
        hit(
            "45857",
            "Jujutsu Kaisen Season 2",
            "jujutsu-kaisen-2",
            "TV",
            Some(23),
        ),
        "Jujutsu Kaisen Season 2",
    );
    assert!(!ruled_out("Jujutsu Kaisen 2nd Season", Some(1.0), &s2));
    let fourth = hit(
        "45556",
        "Golden Kamuy 4th Season",
        "golden-kamuy-4th-season",
        "TV",
        Some(13),
    );
    assert!(!ruled_out("Golden Kamuy Season 4", Some(1.0), &fourth));
}

#[test]
fn a_bare_trailing_number_on_the_slug_names_the_season() {
    let slime = titled(
        hit(
            "42196",
            "Tensei shitara Slime Datta Ken 2",
            "tensei-shitara-slime-datta-ken-2",
            "TV",
            Some(12),
        ),
        "That Time I Got Reincarnated as a Slime 2",
    );
    assert!(!ruled_out(
        "That Time I Got Reincarnated as a Slime Season 2",
        Some(1.0),
        &slime
    ));
}

#[test]
fn a_season_row_rules_out_the_first_season() {
    // "Haikyuu!!: To the Top 2nd Season" against To the Top itself.
    let first = titled(
        hit(
            "42059",
            "Haikyuu!!: To the Top",
            "haikyuu-to-the-top",
            "TV",
            Some(13),
        ),
        "Haikyuu!! To the Top",
    );
    assert!(ruled_out(
        "Haikyuu!!: To the Top 2nd Season",
        Some(1.0),
        &first
    ));
    let part1 = titled(
        hit(
            "44294",
            "JoJo's Bizarre Adventure: Stone Ocean",
            "jojo-s-bizarre-adventure-part-6-stone-ocean",
            "TV",
            Some(12),
        ),
        "JoJo's Bizarre Adventure: Stone Ocean",
    );
    assert!(ruled_out(
        "JoJo's Bizarre Adventure Part 6: Stone Ocean Part 2",
        Some(1.0),
        &part1
    ));
}

#[test]
fn a_hit_titled_as_the_row_names_its_season_whatever_its_slug() {
    let mut gintama = hit("5971", "Gintama'", "gintama-second", "TV", Some(51));
    gintama.abbreviated_titles.push("Gintama Season 2".into());
    assert!(!ruled_out("Gintama Season 2", Some(1.0), &gintama));
}

#[test]
fn a_mid_title_part_is_no_season() {
    // "Part 6" names the JoJo series, not a cour of it.
    let so = titled(
        hit(
            "44294",
            "JoJo's Bizarre Adventure: Stone Ocean",
            "jojo-s-bizarre-adventure-part-6-stone-ocean",
            "TV",
            Some(12),
        ),
        "JoJo's Bizarre Adventure: Stone Ocean",
    );
    assert!(!ruled_out(
        "JoJo's Bizarre Adventure Part 6: Stone Ocean",
        Some(1.0),
        &so
    ));
}

// — A later part of a season ————————————————————————————————————

#[test]
fn a_whole_season_row_rules_out_the_season_s_second_part() {
    let part2 = hit(
        "43361",
        "Tensei shitara Slime Datta Ken 2nd Season Part 2",
        "tensei-shitara-slime-datta-ken-2nd-season-part-2",
        "TV",
        Some(12),
    );
    assert!(ruled_out(
        "That Time I Got Reincarnated as a Slime Season 2",
        Some(1.0),
        &part2
    ));
}

#[test]
fn a_row_naming_a_part_keeps_the_part() {
    let part2 = titled(
        hit(
            "43247",
            "Re:Zero kara Hajimeru Isekai Seikatsu 2nd Season Part 2",
            "re-zero-kara-hajimeru-isekai-seikatsu-2nd-season-part-2",
            "TV",
            Some(12),
        ),
        "Re:ZERO -Starting Life in Another World- Season 2 Part 2",
    );
    assert!(!ruled_out(
        "Re:Zero - Starting Life in Another World 2nd Season (Part 2)",
        Some(1.0),
        &part2
    ));
}

#[test]
fn a_part_with_no_season_is_not_a_later_part_of_one() {
    // "86 Part 2" is the second season of 86, titled by its part alone.
    let p2 = titled(
        hit("44398", "86 Part 2", "86-part-2", "TV", Some(12)),
        "86 EIGHTY-SIX Part 2",
    );
    assert!(!ruled_out("Eighty Six: 2nd Season", Some(1.0), &p2));
}

#[test]
fn ruled_out_ids_keeps_the_order_of_the_hits() {
    let hits = vec![
        hit("1", "Show Special", "show-special", "special", Some(1)),
        hit("2", "Show", "show", "TV", Some(24)),
        hit("3", "Show Extra", "show-extra", "special", Some(1)),
    ];
    assert_eq!(ruled_out_ids("Show", "", Some(20.0), &hits), vec!["1", "3"]);
    assert!(ruled_out_ids("Show", "", Some(1.0), &hits).is_empty());
}

#[test]
fn ruled_out_ids_reads_the_key_s_words_too() {
    let recap = vec![hit("1", "Show Recap", "show-recap", "special", Some(1))];
    assert_eq!(ruled_out_ids("Show", "show", Some(1.0), &recap), vec!["1"]);
    assert!(ruled_out_ids("Show", "show recaps", Some(1.0), &recap).is_empty());
}

#[test]
fn a_refuted_hit_beside_one_that_stands_is_left_to_the_picker() {
    // Kitsu files some films as specials and tags some movies "Recap"
    // in an alternate title alone, so a refuted hit may be the row's
    // own show: with another hit standing, the search answered the row,
    // and the picker chooses among them all.
    let hits = vec![
        hit("1", "Show Recap", "show-recap", "special", Some(1)),
        hit("2", "Show", "show", "TV", Some(24)),
    ];
    assert!(refutes("Show", "show", &hits[0]));
    assert!(ruled_out_ids("Show", "show", Some(1.0), &hits).is_empty());
}

#[test]
fn a_refuted_hit_is_withheld_when_every_other_is_ruled_out() {
    // The special is past the row; the recap is refuted; nothing stands,
    // so the search missed and both are withheld.
    let hits = vec![
        hit("1", "Show Special", "show-special", "special", Some(1)),
        hit("2", "Show Recap", "show-recap", "special", None),
    ];
    assert!(ruled_out("Show", Some(20.0), &hits[0]));
    assert!(!ruled_out("Show", Some(20.0), &hits[1]));
    assert_eq!(
        ruled_out_ids("Show", "show", Some(20.0), &hits),
        vec!["1", "2"]
    );
    // At the first episode the special stands, and the recap is kept.
    assert!(ruled_out_ids("Show", "show", Some(1.0), &hits).is_empty());
}

// — A movie ————————————————————————————————————————————————————

#[test]
fn only_a_movie_word_names_a_movie() {
    assert!(names_movie("Jujutsu Kaisen 0 Movie"));
    assert!(names_movie("Haikyu!! Movie: Battle of the Garbage Dump"));
    assert!(!names_movie("One Piece Film: Red"));
    assert!(!names_movie("Moviestar"));
    assert!(is_movie(Some("movie")));
    assert!(is_movie(Some("Movie")));
    for other in ["special", "TV", "ONA", "OVA", "music"] {
        assert!(!is_movie(Some(other)), "{other}");
    }
    assert!(!is_movie(None));
}

#[test]
fn a_movie_title_refutes_what_is_no_movie() {
    // hianime's "Jujutsu Kaisen 0 Movie" brings back the second
    // season's recap alone, a two-episode special.
    let recap = titled(
        hit(
            "47880",
            "Jujutsu Kaisen 2nd Season Recap",
            "jujutsu-kaisen-2nd-season-recap",
            "special",
            Some(2),
        ),
        "Jujutsu Kaisen Season 2 Recap",
    );
    assert!(refutes("Jujutsu Kaisen 0 Movie", "", &recap));
    let movie = titled(
        hit(
            "44212",
            "Jujutsu Kaisen 0",
            "jujutsu-kaisen-movie-0",
            "movie",
            Some(1),
        ),
        "Jujutsu Kaisen 0",
    );
    assert!(!refutes("Jujutsu Kaisen 0 Movie", "", &movie));
}

#[test]
fn the_number_after_movie_numbers_it() {
    assert_eq!(movie_number("One Piece: The Movie 14 - Stampede"), Some(14));
    assert_eq!(
        movie_number("Haikyuu!! Movie 4: Concept no Tatakai"),
        Some(4)
    );
    assert_eq!(movie_number("Jujutsu Kaisen 0 Movie"), None);
    assert_eq!(movie_number("Demon Slayer Movie: Mugen Train"), None);
}

#[test]
fn a_numbered_movie_refutes_a_movie_numbered_otherwise() {
    // hianime's "One Piece: The Movie 14 - Stampede" brings back the
    // first thirteen and none of Stampede.
    let first = hit(
        "421",
        "One Piece Movie 1",
        "one-piece-2000",
        "movie",
        Some(1),
    );
    assert!(refutes("One Piece: The Movie 14 - Stampede", "", &first));
    // Numbered in an abbreviation alone.
    let mut z = hit(
        "6827",
        "One Piece Film: Z",
        "one-piece-film-z",
        "movie",
        Some(1),
    );
    z.abbreviated_titles.push("One Piece Movie 12".into());
    assert!(numbered_as_another_movie(
        "One Piece: The Movie 14 - Stampede",
        &z
    ));
    // Stampede itself numbers no movie, and is not another.
    let stampede = titled(
        hit(
            "42061",
            "Gekijouban One Piece: Stampede",
            "one-piece-stampede",
            "movie",
            Some(1),
        ),
        "One Piece: Stampede",
    );
    assert!(!refutes(
        "One Piece: The Movie 14 - Stampede",
        "",
        &stampede
    ));
}

#[test]
fn a_numbered_movie_refutes_an_ova() {
    let prologue = hit(
        "10047",
        "One Piece: Episode of Alabasta - Prologue",
        "one-piece-episode-of-alabaster-prologue",
        "OVA",
        Some(1),
    );
    assert!(refutes("One Piece: The Movie 14 - Stampede", "", &prologue));
}

#[test]
fn a_hit_numbering_the_row_s_movie_is_not_another() {
    let mut h = hit("1", "Show Movie 4", "show-movie-4", "movie", Some(1));
    h.abbreviated_titles.push("Show Movie 9".into());
    assert!(!numbered_as_another_movie("Show Movie 4: Subtitle", &h));
    // An unnumbered movie title is never told apart by number.
    assert!(!numbered_as_another_movie("Show Movie: Subtitle", &h));
}

// — A recap ————————————————————————————————————————————————————

#[test]
fn a_row_naming_no_recap_rules_out_a_recap() {
    // hianime's "Haikyu!! Movie: Battle of the Garbage Dump" brings back
    // the fourth recap movie alone.
    let mut concept = titled(
        hit(
            "13656",
            "Haikyuu!! Movie 4: Concept no Tatakai",
            "haikyuu-movie-4-concept-no-tatakai",
            "movie",
            Some(1),
        ),
        "Haikyu!! The Movie: Battle of Concepts!!",
    );
    concept.abbreviated_titles.push("Haikyuu!! Recap 4".into());
    assert!(names_recap("Haikyuu!! Recap 4"));
    assert!(refutes(
        "Haikyu!! Movie: Battle of the Garbage Dump",
        "haikyu movie the dumpster battle",
        &concept
    ));
    // Nothing else about it rules it out.
    assert!(!ruled_out(
        "Haikyu!! Movie: Battle of the Garbage Dump",
        Some(1.0),
        &concept
    ));
}

#[test]
fn a_row_naming_a_recap_in_its_title_or_key_keeps_a_recap() {
    let recaps = titled(
        hit(
            "45545",
            "86 Part 2 Recaps",
            "86-part-2-recaps",
            "special",
            Some(2),
        ),
        "86 EIGHTY-SIX Part 2 Recaps",
    );
    assert!(!a_recap_the_row_does_not_name(
        "86 EIGHTY-SIX Season 2 Recap",
        "86 eighty six season 2 recap",
        &recaps
    ));
    let s2_recap = titled(
        hit(
            "47880",
            "Jujutsu Kaisen 2nd Season Recap",
            "jujutsu-kaisen-2nd-season-recap",
            "special",
            Some(2),
        ),
        "Jujutsu Kaisen Season 2 Recap",
    );
    assert!(!a_recap_the_row_does_not_name(
        "Jujutsu Kaisen: Specials",
        "jujutsu kaisen season 2 recaps",
        &s2_recap
    ));
}

// — A season the key names ——————————————————————————————————————

#[test]
fn a_season_is_named_anywhere_in_the_words() {
    assert_eq!(season_named("jujutsu kaisen season 2 recaps"), Some(2));
    assert_eq!(season_named("Jujutsu Kaisen 2nd Season Recap"), Some(2));
    assert_eq!(season_named("Show 1st Season"), Some(1));
    assert_eq!(season_named("Show Season"), None);
    assert_eq!(season_named("Show 2nd"), None);
    assert_eq!(season_named("Show Part 2"), None);
}

#[test]
fn a_key_naming_a_season_the_title_does_not_rules_out_a_hit_naming_none() {
    // hianime's "Jujutsu Kaisen: Specials" is
    // `jujutsu-kaisen-season-2-recaps`; Kitsu's search for the title
    // brings back the New Year's Special, the third season and a movie.
    let ny = titled(
        hit(
            "43886",
            "Jujutsu Kaisen New Year's Special",
            "jujutsu-kaisen-new-year-s-special",
            "special",
            Some(1),
        ),
        "Jujutsu Kaisen New Year's Special",
    );
    let key = "jujutsu kaisen season 2 recaps";
    assert!(refutes("Jujutsu Kaisen: Specials", key, &ny));
    let mut third = titled(
        hit(
            "48363",
            "Jujutsu Kaisen: Shimetsu Kaiyū Zenpen",
            "Jujutsu-Kaisen-3",
            "TV",
            Some(12),
        ),
        "Jujutsu Kaisen Season 3: The Culling Game Part 1",
    );
    third
        .abbreviated_titles
        .push("Jujutsu Kaisen 3rd Season".into());
    assert!(refutes("Jujutsu Kaisen: Specials", key, &third));
    let recap = titled(
        hit(
            "47880",
            "Jujutsu Kaisen 2nd Season Recap",
            "jujutsu-kaisen-2nd-season-recap",
            "special",
            Some(2),
        ),
        "Jujutsu Kaisen Season 2 Recap",
    );
    assert!(!refutes("Jujutsu Kaisen: Specials", key, &recap));
    // Without the key's season, the special stands.
    assert!(!refutes(
        "Jujutsu Kaisen: Specials",
        "jujutsu kaisen specials",
        &ny
    ));
}

#[test]
fn a_title_naming_a_season_is_not_judged_by_its_key() {
    let other = hit(
        "50837",
        "Solo Leveling: ReAwakening",
        "solo-leveling-reawakening",
        "movie",
        Some(1),
    );
    assert!(!refutes(
        "Solo Leveling Season 2: Arise from the Shadow",
        "solo leveling season 2 arise from the shadow",
        &other
    ));
    assert!(!refutes(
        "Show Season 3",
        "show season 2",
        &hit("1", "Show", "show", "TV", Some(12))
    ));
}

#[test]
fn a_key_naming_the_first_season_rules_nothing_out() {
    assert!(!refutes(
        "Show: Extras",
        "show season 1 extras",
        &hit("1", "Show Special", "show-special", "special", Some(1))
    ));
}
