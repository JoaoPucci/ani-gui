//! Whether a word ordinal standing alone is a division of a title or a
//! word of its name, and the "Half" read with one — split from
//! `play_native_title_name` for the per-file complexity bar.

use super::*;

/// Whether the division at `words[i]` is a word ordinal standing alone
/// ("Seventh", not "Seventh Season", "VII" or "7th") with something
/// other than divisions after it — a word of the name, as in "The
/// Seventh Son", not the title's division.
pub(crate) fn inside_the_name(words: &[String], i: usize) -> bool {
    let w = &words[i];
    let word_ordinal = spelled_ordinal(w).is_some()
        && w.chars().all(|c| c.is_ascii_alphabetic())
        && !w.chars().all(|c| "ivx".contains(c));
    let standing_alone = division_at(words, i).is_some_and(|(_, taken)| taken == 1);
    word_ordinal && standing_alone && half_after(words, i) == 0 && !divisions_only(&words[i + 1..])
}

/// 1 when the division at `words[i]` is an ordinal spelled or suffixed
/// as one ("Second", "2nd") with "Half" after it, 0 otherwise: the two
/// are one division ("Second Half", "First Half"), read as the ordinal,
/// so the "Half" is neither a word of the name nor one past it.
pub(crate) fn half_after(words: &[String], i: usize) -> usize {
    let w = &words[i];
    let ordinal = spelled_ordinal(w).is_some() && !w.chars().all(|c| "ivx".contains(c));
    usize::from(ordinal && words.get(i + 1).is_some_and(|h| h == "half"))
}

/// Whether `words` are divisions and nothing else, each read by
/// `division_at`.
fn divisions_only(words: &[String]) -> bool {
    let mut j = 0;
    while j < words.len() {
        match division_at(words, j) {
            Some((_, taken)) => j += taken + if taken == 1 { half_after(words, j) } else { 0 },
            None => return false,
        }
    }
    true
}
