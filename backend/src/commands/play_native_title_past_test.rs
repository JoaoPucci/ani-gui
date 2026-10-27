//! Property coverage for what is read past a stem: the whole stem at
//! the start of a title, and nothing past an empty one.

use super::*;
use proptest::prelude::*;

const NAMES: [&str; 4] = ["Show", "Other Show", "ショー", "Kaiju Story"];
const DIVISIONS: [&str; 7] = [
    "Season 1",
    "Season 2",
    "1st Season",
    "2nd Season",
    "Part 2",
    "Season 3 Part 2",
    "第2期",
];

/// A title of a name and a division, or any text at all.
fn text() -> impl Strategy<Value = String> {
    prop_oneof![
        (
            proptest::sample::select(NAMES.to_vec()),
            proptest::sample::select(DIVISIONS.to_vec()),
        )
            .prop_map(|(n, d)| format!("{n} {d}")),
        proptest::sample::select(DIVISIONS.to_vec()).prop_map(str::to_string),
        any::<String>(),
    ]
}

/// What a stem can be: a name, nothing, blank, or any text at all.
fn own() -> impl Strategy<Value = String> {
    prop_oneof![
        proptest::sample::select(NAMES.to_vec()).prop_map(str::to_string),
        Just(String::new()),
        "[ \t]{1,3}",
        any::<String>(),
    ]
}

proptest! {
    /// An empty stem is no stem: nothing is read past it, whatever the
    /// text opens with.
    #[test]
    fn nothing_is_read_past_an_empty_stem(text in text()) {
        prop_assert_eq!(rest_after(&text, ""), None);
    }

    /// Whatever is read past a stem is what follows the whole of it at
    /// the start of the text, and the stem is never empty.
    #[test]
    fn a_rest_is_what_follows_the_whole_stem(text in text(), own in own()) {
        if let Some(rest) = rest_after(&text, &own) {
            prop_assert!(!own.is_empty());
            prop_assert_eq!(format!("{own}{rest}"), text);
        }
    }

    /// A title is read past its own name, its division and all.
    #[test]
    fn a_title_is_read_past_its_name(
        name in proptest::sample::select(NAMES.to_vec()),
        division in proptest::sample::select(DIVISIONS.to_vec()),
    ) {
        let title = format!("{name} {division}");
        prop_assert_eq!(rest_after(&title, name), Some(format!(" {division}")));
    }

    /// A broad listing that is nothing but a division has an empty
    /// stem, and no title is read past it.
    #[test]
    fn no_title_is_read_past_a_division_alone(
        wide in proptest::sample::select(DIVISIONS.to_vec()),
        title in text(),
    ) {
        prop_assert!(stem(wide).is_empty(), "{wide}");
        prop_assert_eq!(past_stem(&title, wide), None);
    }
}

proptest! {
    /// A stem whose last word is a number glued to a Japanese word, as
    /// "ショー2" writes one, is read without that number as a stem
    /// written apart is, the number its own.
    #[test]
    fn a_glued_japanese_stem_number_comes_off_as_one_apart_does(
        before in proptest::sample::select(vec!["", "Big ", "Big Small "]),
        j in proptest::sample::select(vec!["ショー", "推しの子", "ｼｮｰ"]),
        n in 1u32..100,
    ) {
        let glued = stem(&format!("{before}{j}{n}"));
        let apart = stem(&format!("{before}{j} {n}"));
        prop_assert_eq!(&glued, &apart);
        let want = format!("{}{j}", before.to_lowercase());
        prop_assert_eq!(without_stem_number(&glued), Some((want, n)));
    }
}
