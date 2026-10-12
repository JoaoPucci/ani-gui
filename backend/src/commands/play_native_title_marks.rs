//! The marks that tell two titles of one name apart where their words
//! do not: the prime of "Gintama'" against "Gintama", and the release
//! years of "Film (2020)" and "Film (2019)". Split from
//! `play_native_title_own` for the per-file complexity bar.
//!
//! Only these count. Catalogues write one show with and without an
//! exclamation or question mark, a decorative symbol, quotes or a wave
//! dash in one form or another, and with or without its release year;
//! none of that drift names another show.

use super::play_native_title_grammar::split_tags;

/// The primes `title` is written with — every prime or apostrophe,
/// in whatever form, read as `'` — in order, and the release years it
/// is tagged with ("(2011)", "（２０１１）"), sorted. Every other mark
/// and every format tag ("(TV)", "[OVA]") is left out: the format
/// badge speaks for the format.
pub(crate) fn title_marks(title: &str) -> (String, Vec<String>) {
    let (text, tags) = split_tags(title);
    let primes = text
        .chars()
        .filter(|c| is_prime(*c))
        .map(|_| '\'')
        .collect();
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

/// Whether `c` is a prime or an apostrophe, in any of the forms
/// titles write one with.
fn is_prime(c: char) -> bool {
    matches!(
        c,
        '\'' | '\u{2019}' | '\u{2018}' | '\u{2032}' | '\u{00B4}' | '`' | '\u{02B9}' | '\u{FF07}'
    )
}

#[cfg(test)]
#[path = "play_native_title_marks_test.rs"]
mod tests;
