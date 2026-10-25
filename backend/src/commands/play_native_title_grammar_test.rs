//! Property coverage for the word-level readings the title grammar
//! makes before any rule reads a title: a word read apart, the numbers
//! a word writes standing as numbers, a stem's glued number, and a
//! catalogue tag glued between two words.

use super::*;
use proptest::prelude::*;

const LATIN: [&str; 4] = ["Show", "Arc", "Café", "Kaiju"];
const JAPANESE: [&str; 4] = ["ショー", "推しの子", "怪獣", "ｼｮｰ"];
const DIVISIONS: [&str; 6] = [
    "第2期",
    "第十二部",
    "第３クール",
    "第〇期",
    "第2部",
    "第3期",
];
const SUFFIXES: [&str; 4] = ["st", "nd", "rd", "th"];
const TAGS: [&str; 4] = ["TV", "2011", "ＴＶ", "OVA"];
const OPENS: [&str; 4] = ["(", "[", "（", "［"];
const CLOSES: [&str; 4] = [")", "]", "）", "］"];

/// One word as a title glues it: a head of either script or none, any
/// glued divisions, any glued number, or any text without whitespace.
fn word() -> impl Strategy<Value = String> {
    let head = prop_oneof![
        proptest::sample::select(LATIN.to_vec()),
        proptest::sample::select(JAPANESE.to_vec()),
        Just(""),
    ];
    let glued = proptest::collection::vec(proptest::sample::select(DIVISIONS.to_vec()), 0..3);
    let number = prop_oneof![
        Just(String::new()),
        (0u32..1000).prop_map(|n| n.to_string())
    ];
    prop_oneof![
        (head, glued, number).prop_map(|(h, g, n)| format!("{h}{}{n}", g.concat())),
        "[^\\s]{0,12}",
    ]
}

proptest! {
    /// A word read apart is split, never changed: its pieces put back
    /// together are the word, none of them is empty, and reading the
    /// pieces apart again splits nothing further.
    #[test]
    fn a_word_read_apart_is_its_pieces_in_order(w in word()) {
        let pieces = apart_glued(vec![w.clone()]);
        prop_assert_eq!(pieces.concat(), w);
        prop_assert!(pieces.iter().all(|p| !p.is_empty()));
        prop_assert_eq!(apart_glued(pieces.clone()), pieces);
    }

    /// Every number a word writes standing is one whole run of its
    /// digits, and under a thousand. A number glued to a Japanese letter
    /// stands, up to two digits bare and up to three suffixed
    /// ("ショー100th"); one glued to any other letter, before it or
    /// after it, is the word's own; each number of a span stands either
    /// side of its "+".
    #[test]
    fn a_number_stands_where_japanese_glues_it(
        w in word(),
        j in proptest::sample::select(JAPANESE.to_vec()),
        l in proptest::sample::select(LATIN.to_vec()),
        sfx in proptest::sample::select(SUFFIXES.to_vec()),
        n in 0u32..1000,
        a in 0u32..100,
        b in 0u32..100,
    ) {
        let runs: Vec<u32> = w
            .split(|c: char| !c.is_ascii_digit())
            .filter_map(|run| run.parse().ok())
            .collect();
        for m in standing_numbers(&w) {
            prop_assert!(m < 1000 && runs.contains(&m), "{w}: {m}");
        }
        let suffixed = format!("{j}{n}{sfx}");
        prop_assert!(standing_numbers(&suffixed).contains(&n), "{suffixed}");
        let bare = format!("{j}{n}");
        prop_assert_eq!(standing_numbers(&bare).contains(&n), n < 100, "{}", bare);
        let latin = format!("{l}{n}");
        prop_assert!(standing_numbers(&latin).is_empty(), "{latin}");
        let before_latin = format!("{n}{l}");
        prop_assert!(standing_numbers(&before_latin).is_empty(), "{before_latin}");
        let span = format!("{a}+{b}");
        let got = standing_numbers(&span);
        prop_assert!(got.contains(&a) && got.contains(&b), "{span}: {got:?}");
    }

    /// A number glued to the end of a Japanese stem is the stem's
    /// number, as one written apart is; glued to any other letter it is
    /// the word's own, and the stem ends on no number.
    #[test]
    fn a_stem_ends_on_a_number_glued_as_japanese_writes_one(
        j in proptest::sample::select(JAPANESE.to_vec()),
        l in proptest::sample::select(LATIN.to_vec()),
        n in 1u32..100,
    ) {
        let glued = format!("{j}{n}");
        let apart = format!("{j} {n}");
        prop_assert_eq!(stem_number(&glued), Some(n));
        prop_assert_eq!(stem_number(&glued), stem_number(&apart));
        let latin = format!("{l}{n}");
        prop_assert_eq!(stem_number(&latin), None);
    }

    /// A catalogue tag written with no space either side parts the
    /// words it stands between: the title reads as the two words with a
    /// space between them, in every bracket pair.
    #[test]
    fn a_tag_glued_between_two_words_parts_them(
        a in proptest::sample::select(LATIN.to_vec()),
        b in prop_oneof![proptest::sample::select(LATIN.to_vec()), Just("2"), Just("No")],
        t in proptest::sample::select(TAGS.to_vec()),
        o in 0..OPENS.len(),
        c in 0..CLOSES.len(),
    ) {
        let glued = format!("{a}{}{t}{}{b}", OPENS[o], CLOSES[c]);
        let spaced = format!("{a} {b}");
        prop_assert_eq!(words(&glued), words(&spaced), "{}", glued);
        prop_assert_eq!(untagged_normalized(&glued), untagged_normalized(&spaced), "{}", glued);
    }
}
