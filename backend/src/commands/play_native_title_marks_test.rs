//! Property coverage for the marks a title is written with and the
//! release years it is tagged with.

use super::title_marks;
use proptest::prelude::*;

const WORDS: [&str; 5] = ["Show", "Gintama", "Kaiju", "No", "Arc"];
const SPACING: [&str; 7] = [" ", ": ", " - ", " ~ ", ", ", "・", "："];
const PRIMES: [&str; 5] = ["'", "’", "′", "‘", "＇"];
const FORMATS: [&str; 4] = [" (TV)", " [OVA]", " （ＯＮＡ）", " [OAD]"];

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
}
