//! The entry's own titles, as the picker reads them — split from
//! `play_native` for the per-file complexity bar.
//!
//! A pick is made for one anime-database entry, under one search
//! term at a time; the term is only one of the entry's names. The
//! picker carries all of them, canonical first, so a rule about what
//! the entry is called can read every name the entry goes by rather
//! than the alias that happened to be searched.

use std::collections::BTreeSet;

/// Every title the entry goes by: its canonical title, then the
/// fallbacks the walk searches in order.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EntryTitles<'a> {
    /// The canonical title, searched first.
    canonical: &'a str,
    /// The fallback titles.
    alts: &'a [&'a str],
}

impl<'a> EntryTitles<'a> {
    /// The entry under `titles`, canonical first — the slice the walk
    /// hands the pick. An empty slice is an entry with no names.
    #[must_use]
    pub(crate) fn new(titles: &'a [&'a str]) -> Self {
        match titles.split_first() {
            Some((canonical, alts)) => Self { canonical, alts },
            None => Self::bare(""),
        }
    }

    /// An entry known by one title alone.
    #[must_use]
    pub(crate) fn bare(title: &'a str) -> Self {
        Self {
            canonical: title,
            alts: &[],
        }
    }
}

impl EntryTitles<'_> {
    /// Every title, canonical first.
    fn all(&self) -> impl Iterator<Item = &str> + '_ {
        std::iter::once(self.canonical).chain(self.alts.iter().copied())
    }

    /// Whether a candidate titled `candidate` may be this entry, by
    /// the season and part markers the two carry.
    ///
    /// - A season or part the candidate's title ends on must be one
    ///   some title of the entry names — "My Star: Season 2" is not
    ///   `[Oshi no Ko]`, whose titles name no second anything. Which
    ///   kind names it does not matter: the catalogues disagree on
    ///   it, and "86 Part 2" is listed as "Eighty Six: 2nd Season".
    ///   An ordinal of 1 never disqualifies ("Final Season, Part 1"),
    ///   nor does a span that includes 1 ("(Part 1+2)").
    ///
    /// Only markers that end a title are read as the title's own, so
    /// "JoJo's Bizarre Adventure Part 4: Diamond is Unbreakable" names
    /// a story part, not a cour.
    pub(crate) fn admits(&self, candidate: &str) -> bool {
        let named: BTreeSet<u32> = self.all().flat_map(named_ordinals).collect();
        trailing_markers(candidate)
            .iter()
            .all(|m| m.ordinals.iter().any(|n| *n < 2 || named.contains(n)))
    }
}

/// One season or part marker: the ordinals it names — several for a
/// span ("Part 1+2").
#[derive(Debug, Clone, PartialEq, Eq)]
struct Marker {
    ordinals: Vec<u32>,
}

/// A title's words, lowercased, split at anything that is neither a
/// letter, a digit, nor the `+` a span is written with.
fn words(title: &str) -> Vec<String> {
    title
        .to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '+'))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

fn is_marker_word(word: &str) -> bool {
    matches!(word, "season" | "part" | "cour")
}

/// The ordinal a word spells as an ordinal: "2nd", "second". A bare
/// number is not one ("100" in "Mob Psycho 100").
fn spelled_ordinal(word: &str) -> Option<u32> {
    const SPELLED: &[(&str, u32)] = &[
        ("first", 1),
        ("second", 2),
        ("third", 3),
        ("fourth", 4),
        ("fifth", 5),
        ("sixth", 6),
    ];
    if let Some((_, n)) = SPELLED.iter().find(|(w, _)| *w == word) {
        return Some(*n);
    }
    let digits = ["st", "nd", "rd", "th"]
        .iter()
        .find_map(|suffix| word.strip_suffix(suffix))?;
    small_number(digits)
}

/// A one- or two-digit number.
fn small_number(word: &str) -> Option<u32> {
    (!word.is_empty() && word.len() <= 2 && word.chars().all(|c| c.is_ascii_digit()))
        .then(|| word.parse().ok())
        .flatten()
}

fn roman(word: &str) -> Option<u32> {
    match word {
        "ii" => Some(2),
        "iii" => Some(3),
        "iv" => Some(4),
        "v" => Some(5),
        "vi" => Some(6),
        _ => None,
    }
}

/// The ordinals the word after a marker names: "2", "2nd", "ii", or
/// a span of plain numbers ("1+2").
fn ordinals_after_marker(word: &str) -> Option<Vec<u32>> {
    if word.contains('+') {
        return word.split('+').map(small_number).collect();
    }
    small_number(word)
        .or_else(|| spelled_ordinal(word))
        .or_else(|| roman(word))
        .map(|n| vec![n])
}

/// The marker two consecutive words make, if they make one: "season
/// 2", "part 1+2", "2nd season", "second cour".
fn marker_of(first: &str, second: &str) -> Option<Marker> {
    if is_marker_word(first) {
        if let Some(ordinals) = ordinals_after_marker(second) {
            return Some(Marker { ordinals });
        }
    }
    if !is_marker_word(second) {
        return None;
    }
    let n = spelled_ordinal(first)?;
    Some(Marker { ordinals: vec![n] })
}

/// The markers a title ends on, last first: "Season 3 Part 2" ends on
/// both, "Part 4: Diamond is Unbreakable" on none. Japanese titles
/// end on 第N期 (a season) or 第N部 (a part).
fn trailing_markers(title: &str) -> Vec<Marker> {
    let mut words = words(title);
    let mut out = Vec::new();
    while words.len() >= 2 {
        let n = words.len();
        let Some(marker) = marker_of(&words[n - 2], &words[n - 1]) else {
            break;
        };
        out.push(marker);
        words.truncate(n - 2);
    }
    out.extend(japanese_trailing(title));
    out
}

/// The 第N期 / 第N部 a title ends on, closing brackets aside.
fn japanese_trailing(title: &str) -> Option<Marker> {
    let trimmed = title.trim_end_matches(|c: char| c.is_whitespace() || ")）]】".contains(c));
    let mut chars = trimmed.chars().rev();
    if !matches!(chars.next()?, '期' | '部') {
        return None;
    }
    let n = kanji_or_digit(chars.next()?)?;
    (chars.next()? == '第').then(|| Marker { ordinals: vec![n] })
}

fn kanji_or_digit(c: char) -> Option<u32> {
    const KANJI: &str = "一二三四五六七八九";
    c.to_digit(10)
        .or_else(|| KANJI.chars().position(|k| k == c).map(|i| i as u32 + 1))
}

/// Every ordinal a title names anywhere: each season or part marker,
/// a small number or numeral the title ends on ("Tokyo Ghoul:re 2",
/// "Overlord II"), and a Japanese 第N.
fn named_ordinals(title: &str) -> BTreeSet<u32> {
    let words = words(title);
    let mut out: BTreeSet<u32> = words
        .windows(2)
        .filter_map(|w| marker_of(&w[0], &w[1]))
        .flat_map(|m| m.ordinals)
        .collect();
    if let Some(last) = words.last() {
        out.extend(small_number(last).or_else(|| roman(last)));
    }
    let chars: Vec<char> = title.chars().collect();
    for w in chars.windows(2) {
        if w[0] == '第' {
            out.extend(kanji_or_digit(w[1]));
        }
    }
    out
}

#[cfg(test)]
#[path = "play_native_title_marker_test.rs"]
mod tests;
