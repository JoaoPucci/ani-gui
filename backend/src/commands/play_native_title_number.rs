//! The first number a title carries, and whether one title only
//! continues another's — the title rule's guard against another show
//! numbered on from the entry's ("Show20" for "Show 2") — split from
//! `play_native_title_reading` for the per-file complexity bar.

use super::play_native_kanji_number::with_kanji_numbers;
use super::play_native_title_grammar::words;

/// A title's words before its first number, run together, and that
/// number's digits — the first run of them in the first word holding
/// any, so a separate
/// word after it never lengthens it ("Kaiju No. 8 2nd Season" carries
/// 8): "Show 2" and "Show20" are ("show", "2") and
/// ("show", "20"), "Mob Psycho 100 II" is ("mobpsycho", "100"), "86
/// Part 2" is ("", "86").
fn first_number(title: &str) -> Option<(String, String)> {
    // A kanji number where one is expected counts as the number it is
    // ("怪獣八十号" carries 80).
    let words: Vec<String> = words(title).iter().map(|w| with_kanji_numbers(w)).collect();
    let at = words
        .iter()
        .position(|w| w.chars().any(|c| c.is_ascii_digit()))?;
    let word = &words[at];
    let start = word.find(|c: char| c.is_ascii_digit())?;
    let digits: String = word[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    let before = words[..at].concat() + &word[..start];
    Some((before, digits))
}

/// Whether `candidate` matches `title` up to its first number and then
/// only continues that number with more digits ([`first_number`]):
/// "Show20" continues "Show 2", "Mob Psycho 1000 Part 2" continues "Mob
/// Psycho 100 II", "861" continues "86 Part 2"; "Show 2nd Season" and
/// "Kaiju No. 8 Season 2" continue nothing.
pub(super) fn continues_number_of(candidate: &str, title: &str) -> bool {
    match (first_number(candidate), first_number(title)) {
        (Some((before, digits)), Some((theirs, number))) => {
            before == theirs && digits != number && digits.starts_with(&number)
        }
        _ => false,
    }
}

/// Whether `candidate` carries `title`'s first number as `title` does —
/// the same words before it and the same digits.
pub(super) fn carries_number_of(candidate: &str, title: &str) -> bool {
    first_number(candidate).is_some_and(|c| first_number(title).is_some_and(|t| c == t))
}
