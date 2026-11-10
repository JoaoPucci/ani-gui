//! Whether a provider title names a Kitsu entry by one of its own
//! titles. See `cour_same_name.rs`.

use super::*;

#[test]
fn a_title_the_entry_carries_names_it() {
    // Gintama°: Kitsu's en_us title is the provider's card title.
    assert!(names_the_entry(
        "Gintama Season 4",
        ["Gintama°", "Gintama Season 4", "銀魂°", "Gintama' (2015)"]
    ));
}

#[test]
fn case_and_punctuation_do_not_matter() {
    assert!(names_the_entry(
        "Re:ZERO -Starting Life in Another World- Season 2",
        ["re zero starting life in another world season 2"]
    ));
}

#[test]
fn the_cour_suffix_may_be_spelled_either_way() {
    for kitsu in [
        "Show Season 2",
        "Show 2nd Season",
        "Show Second Season",
        "Show Part 2",
        "Show Cour 2",
    ] {
        assert!(
            names_the_entry("Show 2nd Season", [kitsu]),
            "\"Show 2nd Season\" against \"{kitsu}\""
        );
    }
}

#[test]
fn another_cour_number_names_another_entry() {
    assert!(!names_the_entry("Show Season 3", ["Show Season 2"]));
}

#[test]
fn the_same_number_under_another_name_is_not_the_entry() {
    // Read the number alone and the Season 3 Part 2 card agrees with
    // the Season 2 entry: both say "2".
    assert!(!names_the_entry(
        "Attack on Titan Season 3 Part 2",
        ["Attack on Titan Season 2", "Shingeki no Kyojin Season 2"]
    ));
    assert!(!names_the_entry(
        "JoJo's Bizarre Adventure: Stone Ocean Part 3",
        ["JoJo's Bizarre Adventure Part 3"]
    ));
}

#[test]
fn a_title_without_words_names_nothing() {
    assert!(!names_the_entry("", [""]));
    assert!(!names_the_entry("!!", ["??"]));
}

#[test]
fn a_bare_suffix_is_not_a_name() {
    // Nothing precedes the suffix, so there is no name to compare;
    // only the identical words would count.
    assert!(!names_the_entry("Season 2", ["2nd Season"]));
}

#[test]
fn words_a_slug_runs_together_name_the_entry() {
    // hianime's key for Re:ZERO Season 4 is
    // `rezero-starting-life-in-another-world-season-4-4587`; Kitsu
    // titles the entry "Re:ZERO -Starting Life in Another World- Season
    // 4", so "rezero" is Kitsu's "re" and "zero" run together.
    let entry = [
        "Re:Zero kara Hajimeru Isekai Seikatsu 4th Season",
        "Re:ZERO -Starting Life in Another World- Season 4",
        "Re:ゼロから始める異世界生活 4th season",
    ];
    assert!(names_the_entry(
        "rezero starting life in another world season 4",
        entry
    ));
    // The cour suffix may still be spelled either way.
    assert!(names_the_entry(
        "rezero starting life in another world 4th season",
        entry
    ));
}

#[test]
fn words_run_together_still_name_no_other_season() {
    let season_4 = ["Re:ZERO -Starting Life in Another World- Season 4"];
    let season_3 = ["Re:ZERO -Starting Life in Another World- Season 3"];
    assert!(!names_the_entry(
        "rezero starting life in another world season 3",
        season_4
    ));
    assert!(!names_the_entry(
        "rezero starting life in another world season 4",
        season_3
    ));
    assert!(!names_the_entry(
        "rezero starting life in another world",
        season_4
    ));
}

#[test]
fn numbers_are_never_run_together() {
    // Only letters meet across a gap: two numbers stay two, so a
    // season and part never read as one larger number, and a number
    // stays apart from the word beside it.
    assert!(!names_the_entry("Show Season 1 2", ["Show Season 12"]));
    assert!(!names_the_entry("Show 1 2", ["Show 12"]));
    assert!(!names_the_entry("Show 2", ["Show2"]));
}

mod props {
    use super::super::names_the_entry;
    use proptest::prelude::*;

    const KEYWORDS: [&str; 3] = ["Season", "Part", "Cour"];
    const WORDS: [&str; 10] = [
        "First", "Second", "Third", "Fourth", "Fifth", "Sixth", "Seventh", "Eighth", "Ninth",
        "Tenth",
    ];

    fn suffix(n: u32) -> &'static str {
        match (n % 10, n % 100) {
            (_, 11..=13) => "th",
            (1, _) => "st",
            (2, _) => "nd",
            (3, _) => "rd",
            _ => "th",
        }
    }

    /// Every spelling of cour `n` after `name`.
    fn spellings(name: &str, n: u32, kw: &str) -> Vec<String> {
        let mut out = vec![
            format!("{name} {kw} {n}"),
            format!("{name} {n}{} {kw}", suffix(n)),
        ];
        if let Some(w) = WORDS.get((n as usize).wrapping_sub(1)) {
            out.push(format!("{name} {w} {kw}"));
        }
        out
    }

    proptest! {
        /// A title names an entry that carries it, however the case
        /// and punctuation around its words differ.
        #[test]
        fn a_title_names_an_entry_that_carries_it(
            words in prop::collection::vec("[A-Za-z0-9]{1,6}", 1..5),
            seps in prop::collection::vec("[ :!?.'-]{1,2}", 5),
        ) {
            let plain = words.join(" ");
            let dressed: String = words
                .iter()
                .zip(&seps)
                .map(|(w, s)| format!("{}{s}", w.to_uppercase()))
                .collect();
            prop_assert!(names_the_entry(&plain, [plain.as_str()]));
            prop_assert!(names_the_entry(&plain, [dressed.as_str()]), "{} / {}", plain, dressed);
        }

        /// The same name and cour number name the entry whichever
        /// spelling of the suffix either side uses; another number
        /// never does, in any spelling.
        #[test]
        fn the_cour_number_decides_and_its_spelling_does_not(
            name in "[a-z]{2,6}( [a-z]{2,6}){0,2}",
            n in 1u32..40,
            m in 1u32..40,
            kw_a in prop::sample::select(KEYWORDS.to_vec()),
            kw_b in prop::sample::select(KEYWORDS.to_vec()),
        ) {
            for provider in spellings(&name, n, kw_a) {
                for kitsu in spellings(&name, m, kw_b) {
                    prop_assert_eq!(
                        names_the_entry(&provider, [kitsu.as_str()]),
                        n == m,
                        "{} / {}", provider, kitsu
                    );
                }
            }
        }

        /// A name never names an entry whose name extends it, nor
        /// the other way round — the case independent draws almost
        /// never reach ("attack on titan" against "attack on titan
        /// final season").
        #[test]
        fn a_name_and_its_extension_are_not_one_entry(
            a in "[a-z]{2,6}( [a-z]{2,6}){0,2}",
            more in "[a-z]{2,6}( [a-z]{2,6}){0,1}",
            n in 1u32..40,
            kw in prop::sample::select(KEYWORDS.to_vec()),
        ) {
            let b = format!("{a} {more}");
            for provider in spellings(&a, n, kw) {
                for kitsu in spellings(&b, n, kw) {
                    prop_assert!(!names_the_entry(&provider, [kitsu.as_str()]), "{} / {}", provider, kitsu);
                    prop_assert!(!names_the_entry(&kitsu, [provider.as_str()]), "{} / {}", kitsu, provider);
                }
            }
        }

        /// A different name never names the entry, whatever cour
        /// either side carries.
        #[test]
        fn another_name_never_names_the_entry(
            a in "[a-z]{2,6}( [a-z]{2,6}){0,2}",
            b in "[a-z]{2,6}( [a-z]{2,6}){0,2}",
            n in 1u32..40,
            kw in prop::sample::select(KEYWORDS.to_vec()),
        ) {
            // Names that differ only in where their words break are
            // one name run together ("re zero" and "rezero").
            prop_assume!(a.replace(' ', "") != b.replace(' ', ""));
            for provider in spellings(&a, n, kw) {
                for kitsu in spellings(&b, n, kw) {
                    prop_assert!(!names_the_entry(&provider, [kitsu.as_str()]), "{} / {}", provider, kitsu);
                }
            }
        }

        /// A name whose words are run together, in any places, names
        /// the entry it names with them apart, in every spelling of the
        /// same cour; never one of another cour.
        #[test]
        fn a_name_run_together_names_only_its_own_cour(
            words in prop::collection::vec("[a-z]{1,6}", 2..5),
            joins in prop::collection::vec(any::<bool>(), 4),
            n in 1u32..40,
            m in 1u32..40,
            kw_a in prop::sample::select(KEYWORDS.to_vec()),
            kw_b in prop::sample::select(KEYWORDS.to_vec()),
        ) {
            let apart = words.join(" ");
            let mut together = words[0].clone();
            for (w, join) in words[1..].iter().zip(&joins) {
                if !*join {
                    together.push(' ');
                }
                together.push_str(w);
            }
            for provider in spellings(&together, n, kw_a) {
                for kitsu in spellings(&apart, m, kw_b) {
                    prop_assert_eq!(
                        names_the_entry(&provider, [kitsu.as_str()]),
                        n == m,
                        "{} / {}", provider, kitsu
                    );
                }
            }
        }

        /// Numbers never run together: a cour written as two numbers
        /// never names an entry whose cour is the two written as one.
        #[test]
        fn two_numbers_are_never_one(
            name in "[a-z]{2,6}( [a-z]{2,6}){0,2}",
            a in 1u32..100,
            b in 0u32..100,
            kw in prop::sample::select(KEYWORDS.to_vec()),
        ) {
            let provider = format!("{name} {kw} {a} {b}");
            let kitsu = format!("{name} {kw} {a}{b}");
            prop_assert!(!names_the_entry(&provider, [kitsu.as_str()]), "{} / {}", provider, kitsu);
            prop_assert!(!names_the_entry(&kitsu, [provider.as_str()]), "{} / {}", kitsu, provider);
        }
    }
}

#[derive(serde::Deserialize)]
struct Vector {
    case: String,
    title: String,
    names: Vec<String>,
    names_entry: bool,
}

/// The vectors the frontend's port runs too: the two sides answer
/// the same for the same title and names.
#[test]
fn every_shared_vector_reads_the_same_as_on_the_frontend() {
    let vectors: Vec<Vector> = serde_json::from_slice(include_bytes!(
        "../../../tests/fixtures/same-name/vectors.json"
    ))
    .expect("vectors");
    assert!(vectors.len() >= 30, "the shared vectors are all there");
    let wrong: Vec<&str> = vectors
        .iter()
        .filter(|v| names_the_entry(&v.title, v.names.iter().map(String::as_str)) != v.names_entry)
        .map(|v| v.case.as_str())
        .collect();
    assert!(wrong.is_empty(), "vectors read otherwise: {wrong:?}");
}
