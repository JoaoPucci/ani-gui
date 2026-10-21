//! The one reader the spanning cut compares titles with: what a title
//! names past a broad listing's stem, and how a title reads as a
//! sequence of divisions — split from `play_native_title_grammar`,
//! whose marker parser it reads through, for the per-file complexity
//! bar.

use super::play_native_title_grammar::{
    glued_number, half_width, is_japanese, japanese_trailing, kind_of, marker_of,
    ordinals_after_marker, small_number, spelled_ordinal, stem, trailing_markers,
    untagged_normalized, words, Kind, Marker,
};

#[path = "play_native_title_past.rs"]
mod title_past;
pub(crate) use title_past::*;

#[path = "play_native_title_name.rs"]
mod title_name;
pub(crate) use title_name::*;

#[path = "play_native_title_rest.rs"]
mod title_rest;
pub(crate) use title_rest::*;

#[path = "play_native_title_entry_before.rs"]
mod title_entry_before;
pub(crate) use title_entry_before::*;

#[path = "play_native_title_beside.rs"]
mod title_beside;
pub(crate) use title_beside::*;

#[path = "play_native_title_alone.rs"]
mod title_alone;
pub(crate) use title_alone::*;

#[path = "play_native_title_division.rs"]
mod title_division;
pub(crate) use title_division::*;

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

/// A title's words as the reader reads them ([`words`]), each word that
/// holds Japanese divisions glued to its end read as what it holds
/// before them and each division in turn: "Fourth第十二部" is "fourth"
/// and "第十二部", "ショー第2期第2部" is "ショー", "第2期" and "第2部".
pub(crate) fn read_words(title: &str) -> Vec<String> {
    super::play_native_title_grammar::apart_glued(words(title))
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
        match division_at(words, i).filter(|_| !inside_the_name(words, i)) {
            Some((m, taken)) => {
                out.push(m);
                i += taken + if taken == 1 { half_after(words, i) } else { 0 };
            }
            None if leading => break,
            None => i += 1,
        }
    }
    out
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
    divisions(&read_words(title))
        .into_iter()
        .filter(|m| m.ordinals.first() != Some(&1))
        .flat_map(|m| m.ordinals)
        .collect()
}
