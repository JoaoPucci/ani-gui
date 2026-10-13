//! The marks that tell two titles of one name apart where their words
//! do not: the prime of "Gintama'" against "Gintama", and the release
//! years of "Film (2020)" and "Film (2019)". Split from
//! `play_native_title_own` for the per-file complexity bar.
//!
//! Only these count. Catalogues write one show with and without an
//! exclamation or question mark, a decorative symbol, quotes single or
//! double, an elision that opens a word ("'Tis"), a possessive inside
//! one ("JoJo's") or a wave dash in one form or another, and with or
//! without its release year; none of that drift names another show. An
//! apostrophe that ends a word is a prime whatever it means there, so
//! a plural possessive ("Girls' Last Tour") reads as one.

use super::play_native_title_grammar::split_tags;

/// The primes `title` is written with, as that many `'`, and the
/// release years it is tagged with ("(2011)", "（２０１１）"), sorted.
/// A prime is an apostrophe, in any form, that ends a word ("Gintama'")
/// and closes no quote: the apostrophes that open a word — a quote
/// opening, an elision ("'Tis") — each cancel one that ends a word, so
/// "Show 'Arc'" carries none, and one inside a word ("JoJo's") is
/// neither. Every other mark and every format tag ("(TV)", "[OVA]") is
/// left out: the format badge speaks for the format.
pub(crate) fn title_marks(title: &str) -> (String, Vec<String>) {
    let (text, tags) = split_tags(title);
    let primes = "'".repeat(prime_count(&text.chars().collect::<Vec<_>>()));
    let years = tags
        .into_iter()
        .filter(|t| t.chars().all(|c| c.is_ascii_digit()))
        .collect();
    (primes, years)
}

/// Whether two titles' marks ([`title_marks`]) agree: the same primes,
/// and release years that do not conflict — equal, or carried by one
/// side alone, a year one catalogue leaves off naming no other show.
pub(crate) fn marks_agree(a: &(String, Vec<String>), b: &(String, Vec<String>)) -> bool {
    a.0 == b.0 && (a.1.is_empty() || b.1.is_empty() || a.1 == b.1)
}

/// The apostrophes in `chars` that end a word, less those that open
/// one, each run of them counted by its length.
fn prime_count(chars: &[char]) -> usize {
    let alnum = |k: Option<&char>| k.is_some_and(|c| c.is_alphanumeric());
    let (mut ending, mut opening) = (0, 0);
    let mut i = 0;
    while i < chars.len() {
        if !is_prime(chars[i]) {
            i += 1;
            continue;
        }
        let start = i;
        while i < chars.len() && is_prime(chars[i]) {
            i += 1;
        }
        let before = alnum(start.checked_sub(1).and_then(|b| chars.get(b)));
        let after = alnum(chars.get(i));
        if before && !after {
            ending += i - start;
        } else if !before && after {
            opening += i - start;
        }
    }
    ending.saturating_sub(opening)
}

/// Whether `c` is a prime or an apostrophe, in any of the forms
/// titles write one with.
fn is_prime(c: char) -> bool {
    matches!(
        c,
        '\'' | '\u{2019}' | '\u{2018}' | '\u{2032}' | '\u{00B4}' | '`' | '\u{FF07}'
    )
}

#[cfg(test)]
#[path = "play_native_title_marks_test.rs"]
mod tests;
