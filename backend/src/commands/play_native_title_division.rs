//! The one division a title's words name at a place, and the Japanese
//! divisions a word holds glued to its end — split from
//! `play_native_title_reading` for the per-file complexity bar.

use super::*;

/// The division `words` name starting at `i`, and how many words it
/// takes: a marker pair, unless the first word is a Roman numeral or a
/// bare number before a marker carrying its own ordinal ("II Part 2",
/// "II Part Two") — a suffixed or spelled ordinal pairs with the marker
/// after it ("3rd Season Second Half"); a Japanese division
/// (a word of its own or glued to the end of one); or an ordinal
/// standing alone — spelled, bare, or a number glued to a Japanese
/// word.
pub(crate) fn division_at(words: &[String], i: usize) -> Option<(Marker, usize)> {
    let w = &words[i];
    let numeral = small_number(w).is_some()
        || (w.chars().all(|c| "ivx".contains(c)) && spelled_ordinal(w).is_some());
    let own_marker_follows = numeral
        && words.get(i + 1).is_some_and(|k| kind_of(k).is_some())
        && words
            .get(i + 2)
            .is_some_and(|o| ordinals_after_marker(o).is_some());
    if !own_marker_follows {
        if let Some(m) = words.get(i + 1).and_then(|next| marker_of(w, next)) {
            return Some((m, 2));
        }
    }
    if let Some((m, _)) = japanese_trailing(w) {
        return Some((m, 1));
    }
    let n = spelled_ordinal(w)
        .or_else(|| small_number(w))
        .or_else(|| glued_number(w))?;
    Some((
        Marker {
            kind: Kind::Season,
            ordinals: vec![n],
        },
        1,
    ))
}

/// The Japanese divisions a word ends on, in order — every one of
/// several glued one after another ("ショー第2期第2部" → season 2, part
/// 2) — and what the word holds before the first ("ショー").
pub(crate) fn glued_divisions(word: &str) -> Option<(Vec<Marker>, &str)> {
    let (first, mut head) = japanese_trailing(word)?;
    let mut out = vec![first];
    while let Some((m, before)) = japanese_trailing(head) {
        out.push(m);
        head = before;
    }
    out.reverse();
    Some((out, head))
}
