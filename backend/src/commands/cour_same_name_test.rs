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

        /// A different name never names the entry, whatever cour
        /// either side carries.
        #[test]
        fn another_name_never_names_the_entry(
            a in "[a-z]{2,6}( [a-z]{2,6}){0,2}",
            b in "[a-z]{2,6}( [a-z]{2,6}){0,2}",
            n in 1u32..40,
            kw in prop::sample::select(KEYWORDS.to_vec()),
        ) {
            prop_assume!(a != b);
            for provider in spellings(&a, n, kw) {
                for kitsu in spellings(&b, n, kw) {
                    prop_assert!(!names_the_entry(&provider, [kitsu.as_str()]), "{} / {}", provider, kitsu);
                }
            }
        }
    }
}
