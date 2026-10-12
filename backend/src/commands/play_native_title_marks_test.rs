//! Property coverage for the marks that tell two titles of one name
//! apart — primes and release years — and for the drift that does not.

use super::{marks_agree, title_marks};
use proptest::prelude::*;

const WORDS: [&str; 5] = ["Show", "Gintama", "Kaiju", "No", "Arc"];
const SPACING: [&str; 7] = [" ", ": ", " - ", " ~ ", ", ", "・", "："];
const PRIMES: [&str; 5] = ["'", "’", "′", "‘", "＇"];
const FORMATS: [&str; 4] = [" (TV)", " [OVA]", " （ＯＮＡ）", " [OAD]"];
const DRIFT: [&str; 10] = ["!", "!!", "?", "☆", "△", ".", ";", "〜", "\u{201C}", "/"];

fn words() -> impl Strategy<Value = Vec<&'static str>> {
    proptest::collection::vec(proptest::sample::select(WORDS.to_vec()), 1..4)
}

fn spaced(words: &[&str], spacing: &[&str]) -> String {
    let mut out = words[0].to_string();
    for (i, w) in words[1..].iter().enumerate() {
        out.push_str(spacing[i % spacing.len()]);
        out.push_str(w);
    }
    out
}

proptest! {
    /// Spacing punctuation between words is no mark: a title written
    /// with any of it reads as the same title written with spaces.
    #[test]
    fn spacing_punctuation_is_no_mark(
        words in words(),
        spacing in proptest::collection::vec(proptest::sample::select(SPACING.to_vec()), 1..4),
    ) {
        prop_assert_eq!(title_marks(&spaced(&words, &spacing)), title_marks(&words.join(" ")));
        prop_assert_eq!(title_marks(&words.join(" ")), (String::new(), Vec::new()));
    }

    /// A prime or apostrophe is a mark, however it is written.
    #[test]
    fn every_prime_reads_as_one_mark(words in words(), prime in proptest::sample::select(PRIMES.to_vec())) {
        let (marks, years) = title_marks(&format!("{}{prime}", words.join(" ")));
        prop_assert_eq!(marks, "'");
        prop_assert!(years.is_empty());
    }

    /// A bracketed release year is read as the title's year, in any
    /// bracket and digit form; a format tag is neither a mark nor a
    /// year.
    #[test]
    fn a_year_tag_counts_and_a_format_tag_does_not(
        words in words(),
        year in 1960u32..2030,
        format in proptest::sample::select(FORMATS.to_vec()),
        full_width in any::<bool>(),
    ) {
        let written = if full_width {
            format!("（{}）", year.to_string().chars().map(|c| char::from_u32(c as u32 - '0' as u32 + '０' as u32).unwrap()).collect::<String>())
        } else {
            format!("({year})")
        };
        let title = words.join(" ");
        prop_assert_eq!(title_marks(&format!("{title} {written}")), (String::new(), vec![year.to_string()]));
        prop_assert_eq!(title_marks(&format!("{title}{format}")), (String::new(), Vec::new()));
    }

    /// Marks catalogues drift on — sentence and decorative marks, a
    /// period, quotes, slashes, a wave dash — are no part of a title's
    /// marks, wherever they stand.
    #[test]
    fn drift_marks_are_no_marks(
        words in words(),
        drift in proptest::collection::vec(proptest::sample::select(DRIFT.to_vec()), 1..4),
    ) {
        let mut title = words.join(" ");
        for d in &drift {
            title.push_str(d);
        }
        prop_assert_eq!(title_marks(&title), (String::new(), Vec::new()));
    }

    /// Two titles' marks agree exactly when their primes are the same
    /// and their release years do not conflict: equal, or one side
    /// carrying none.
    #[test]
    fn marks_agree_on_primes_and_unconflicting_years(
        words in words(),
        primes in (0usize..3, 0usize..3),
        years in (proptest::option::of(2010u32..2013), proptest::option::of(2010u32..2013)),
    ) {
        let written = |n: usize, y: Option<u32>| {
            let mut t = format!("{}{}", words.join(" "), "'".repeat(n));
            if let Some(y) = y {
                t.push_str(&format!(" ({y})"));
            }
            title_marks(&t)
        };
        let (a, b) = (written(primes.0, years.0), written(primes.1, years.1));
        let want = primes.0 == primes.1
            && match years {
                (Some(x), Some(y)) => x == y,
                _ => true,
            };
        prop_assert_eq!(marks_agree(&a, &b), want);
        prop_assert_eq!(marks_agree(&b, &a), want);
    }

    /// An apostrophe that opens a quote or sits inside a word is no
    /// prime: quoting any word of a title in any apostrophe form, or
    /// writing a possessive, leaves its marks as they were, the primes
    /// it ends on included; and an elision is no prime either.
    #[test]
    fn quotes_elisions_and_possessives_are_no_primes(
        words in words(),
        quoted in 0usize..4,
        open in proptest::sample::select(PRIMES.to_vec()),
        close in proptest::sample::select(PRIMES.to_vec()),
        ending in 0usize..3,
    ) {
        let mut parts: Vec<String> = words.iter().map(|w| (*w).to_string()).collect();
        let at = quoted % parts.len();
        parts[at] = format!("{open}{}{close}", parts[at]);
        let plain = format!("{}{}", words.join(" "), "'".repeat(ending));
        let marked = format!("{}{open}s {}{}", words[0], parts.join(" "), "'".repeat(ending));
        prop_assert_eq!(title_marks(&marked), title_marks(&format!("{}s {plain}", words[0])));
        prop_assert_eq!(title_marks(&format!("{open}Tis {}", words.join(" "))), (String::new(), Vec::new()));
    }
}
