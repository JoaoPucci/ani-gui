//! The grammar of the season and part markers titles carry — one
//! parser, read by the title rules (`play_native_title_marker`), by
//! the spanning cut's reader (`play_native_title_reading`) and by the
//! split detection's part reading (`play_native_part_title`).

use std::collections::BTreeSet;

use super::play_native_kanji_number::{kanji_number, with_kanji_numbers};

#[path = "play_native_title_tags.rs"]
mod title_tags;
pub(crate) use title_tags::*;

#[path = "play_native_title_words.rs"]
mod title_words;
pub(crate) use title_words::*;

#[path = "play_native_title_japanese.rs"]
mod title_japanese;
pub(crate) use title_japanese::*;

#[path = "play_native_title_named.rs"]
mod title_named;
pub(crate) use title_named::*;

#[path = "play_native_title_standing.rs"]
mod title_standing;
pub(crate) use title_standing::*;

/// Which kind of division a marker word names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Season,
    Part,
}

/// One season or part marker: its kind and the ordinals it names —
/// several for a span ("Part 1+2").
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Marker {
    pub(crate) kind: Kind,
    pub(crate) ordinals: Vec<u32>,
}

impl Marker {
    /// Whether `named` ordinals cover the marker: a first part, or a
    /// span through it, always; anything else only when every ordinal
    /// it names is among them — "Part 0+2" is not covered by 2 alone.
    pub(super) fn named_by(&self, named: &BTreeSet<u32>) -> bool {
        self.ordinals.contains(&1) || self.ordinals.iter().all(|n| named.contains(n))
    }
}

/// The ordinals the word after a marker names: "2", "2nd", "ii",
/// "two", or a span of plain numbers ("1+2").
pub(super) fn ordinals_after_marker(word: &str) -> Option<Vec<u32>> {
    if word.contains('+') {
        return word.split('+').map(small_number).collect();
    }
    number(word)
        .or_else(|| spelled_ordinal(word))
        .or_else(|| cardinal(word))
        .map(|n| vec![n])
}

/// The marker two consecutive words make, if they make one: "season
/// 2", "part 1+2", "2nd season", "second cour", "2nd stage".
pub(super) fn marker_of(first: &str, second: &str) -> Option<Marker> {
    if let Some(kind) = kind_of(first) {
        if let Some(ordinals) = ordinals_after_marker(second) {
            return Some(Marker { kind, ordinals });
        }
    }
    let kind = kind_of(second)?;
    let n = spelled_ordinal(first)?;
    Some(Marker {
        kind,
        ordinals: vec![n],
    })
}

/// The markers `words` end on, last first, and the words before them.
/// The words are read once, first to last, as the reader reads them
/// ([`division_at`]) — a spelled or suffixed ordinal pairs with the
/// marker after it, so "Third Stage 2" is a third stage and a bare 2 —
/// each word holding Japanese divisions glued to its end read as what
/// it holds before them and each division ([`apart_glued`]); the
/// markers are what that one reading ends on: a marker pair ("Season
/// 2", "2nd Season"), a Japanese division (第N期, 第N部, 第Nクール), or
/// an ordinal spelled as one standing alone ("Overlord II", "X 2nd") —
/// a sequel's number, read as its season. A bare number ends no marker
/// run: it stays in the stem.
///
/// [`division_at`]: super::play_native_title_reading::division_at
fn split_markers(words: Vec<String>) -> (Vec<String>, Vec<Marker>) {
    let words = apart_glued(words);
    let mut items: Vec<(usize, Option<Marker>)> = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let found = super::play_native_title_reading::division_at(&words, i);
        let taken = found.as_ref().map_or(1, |(_, taken)| *taken);
        let marker = found.and_then(|(m, taken)| {
            let w = &words[i];
            (taken == 2 || japanese_trailing(w).is_some() || spelled_ordinal(w).is_some())
                .then_some(m)
        });
        items.push((i, marker));
        i += taken;
    }
    let mut out = Vec::new();
    let mut cut = words.len();
    while let Some((start, Some(marker))) = items.pop() {
        out.push(marker);
        cut = start;
    }
    (words[..cut].to_vec(), out)
}

/// A title's tail parsed: the words before the markers it ends on,
/// and those markers, last first ([`split_markers`]). The one parse
/// every reader of a title's tail goes through.
fn parse_tail(title: &str) -> (Vec<String>, Vec<Marker>) {
    split_markers(words(title))
}

/// The markers a title ends on, last first: "Season 3 Part 2" ends on
/// both, "Part 4: Diamond is Unbreakable" on none, "進撃の巨人 第3期"
/// on its season.
pub(super) fn trailing_markers(title: &str) -> Vec<Marker> {
    parse_tail(title).1
}

/// A title's words with the markers it ends on removed: the name the
/// show's seasons and parts share ("Attack on Titan" for "Attack on
/// Titan Season 3 Part 2").
pub(crate) fn stem(title: &str) -> Vec<String> {
    parse_tail(title).0
}

/// A title as written, for comparing two: lowercased, its whitespace
/// runs collapsed, its catalogue tags kept — a tag is what tells a
/// remake from its original ("Hunter x Hunter (2011)") — and read
/// wherever they stand ([`split_tags`]): "Show (2011) Season 2" and
/// "Show Season 2 (2011)" are one title as written.
pub(crate) fn normalized(title: &str) -> String {
    let (text, tags) = split_tags(title);
    std::iter::once(text.as_str())
        .chain(tags.iter().map(String::as_str))
        .flat_map(str::split_whitespace)
        .collect::<Vec<_>>()
        .join(" ")
}

/// A title as its structure is read: [`normalized`] without its
/// catalogue tags ([`untagged`]).
pub(crate) fn untagged_normalized(title: &str) -> String {
    untagged(title)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
#[path = "play_native_title_grammar_test.rs"]
mod tests;
