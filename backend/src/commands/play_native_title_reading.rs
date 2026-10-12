//! The one reader the spanning cut compares titles with: what a title
//! names past a broad listing's stem, and how a title reads as a
//! sequence of divisions — split from `play_native_title_grammar`,
//! whose marker parser it reads through, for the per-file complexity
//! bar.

use super::play_native_title_grammar::{
    glued_number, half_width, japanese_trailing, kind_of, marker_of, normalized,
    ordinals_after_marker, small_number, spelled_ordinal, stem, trailing_markers, words, Kind,
    Marker,
};

#[path = "play_native_title_past.rs"]
mod title_past;
pub(crate) use title_past::*;

#[path = "play_native_title_name.rs"]
mod title_name;
pub(crate) use title_name::*;

#[path = "play_native_title_tags.rs"]
mod title_tags;
pub(crate) use title_tags::*;

#[path = "play_native_title_entry_before.rs"]
mod title_entry_before;
pub(crate) use title_entry_before::*;

#[path = "play_native_title_beside.rs"]
mod title_beside;
pub(crate) use title_beside::*;

/// The divisions `sibling` names as its own right after `wide`'s stem
/// ([`divisions_beside`]), with their kinds: "Attack on Titan Season 3
/// Part 2" beside "Attack on Titan Season 3" names part 2;
/// "Gintama.: Silver Soul Arc - Second Half War" beside "Gintama.:
/// Silver Soul Arc" names 2; "Show Season 3 Recap" beside "Show Season
/// 3" and "Show Side Story 2" beside "Show" name nothing. Empty when
/// `sibling` does not start with the stem.
pub(super) fn later_division_markers(sibling: &str, wide: &str) -> Vec<Marker> {
    divisions_beside(sibling, wide).map_or_else(Vec::new, |(named, _)| named)
}

/// Every division `words` name, first word to last: a marker pair, a
/// Japanese division (apart or glued to the word before it), and an
/// ordinal standing alone — spelled ("Second", "II"), bare ("2") or a
/// number glued to a Japanese title ("ショー2"). A numeral right
/// before a marker that carries its own ordinal is a division of its
/// own: "II Part 2" is a season 2 and a part 2, not a part 2 and a
/// stray 2.
fn divisions(words: &[String]) -> Vec<Marker> {
    scan(words, false)
}

/// [`divisions`], stopping at the first word that names none when
/// `leading` holds: the divisions a title opens with.
fn scan(words: &[String], leading: bool) -> Vec<Marker> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < words.len() {
        if let Some((glued, _)) = glued_divisions(&words[i]) {
            out.extend(glued);
            i += 1;
            continue;
        }
        match division_at(words, i) {
            Some((m, taken)) => {
                out.push(m);
                i += taken;
            }
            None if leading => break,
            None => i += 1,
        }
    }
    out
}

/// The division `words` name starting at `i`, and how many words it
/// takes: a marker pair, unless the first word is a Roman numeral or a
/// bare number before a marker carrying its own ordinal ("II Part 2",
/// "II Part Two") — a suffixed or spelled ordinal pairs with the marker
/// after it ("3rd Season Second Half"); a Japanese division
/// (a word of its own or glued to the end of one); or an ordinal
/// standing alone — spelled, bare, or a number glued to a Japanese
/// word.
fn division_at(words: &[String], i: usize) -> Option<(Marker, usize)> {
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
fn glued_divisions(word: &str) -> Option<(Vec<Marker>, &str)> {
    let (first, mut head) = japanese_trailing(word)?;
    let mut out = vec![first];
    while let Some((m, before)) = japanese_trailing(head) {
        out.push(m);
        head = before;
    }
    out.reverse();
    Some((out, head))
}

/// How a title reads: the ordinals of every division it names
/// ([`divisions`]), in order, a division whose first ordinal is 1 left
/// out as the whole ("Part 1", "First Half", "(Part 1+2)"). The one
/// reader the spanning cut compares titles with: "Show 2", "Show 2nd
/// Season" and "ショー 第2期" read `[2]`; "Show", "Show Part 1" and
/// "Show First Half" read `[]`; "Lucky 2 2nd Season" reads `[2, 2]`;
/// "Show II Part 2: Arc" reads `[2, 2]`. What follows a division is
/// read too, so "Show 2nd Season Recap" reads `[2]`.
pub(super) fn reading(title: &str) -> Vec<u32> {
    divisions(&words(title))
        .into_iter()
        .filter(|m| m.ordinals.first() != Some(&1))
        .flat_map(|m| m.ordinals)
        .collect()
}
