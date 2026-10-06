//! The grammar of the season and part markers titles carry, shared
//! by the title rules (`play_native_title_marker`) and the split
//! detection's part reading (`play_native_part_title`).

use std::collections::BTreeSet;

/// Which kind of division a marker word names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Season,
    Part,
}

/// One season or part marker: its kind and the ordinals it names —
/// several for a span ("Part 1+2").
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Marker {
    pub(super) kind: Kind,
    pub(super) ordinals: Vec<u32>,
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

fn kind_of(word: &str) -> Option<Kind> {
    match word {
        "season" => Some(Kind::Season),
        "part" | "cour" => Some(Kind::Part),
        _ => None,
    }
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

/// The markers a title ends on, last first: "Season 3 Part 2" ends on
/// both, "Part 4: Diamond is Unbreakable" on none. Japanese titles
/// end on 第N期 (a season) or 第N部 (a part).
pub(super) fn trailing_markers(title: &str) -> Vec<Marker> {
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
    let kind = match chars.next()? {
        '期' => Kind::Season,
        '部' => Kind::Part,
        _ => return None,
    };
    let n = kanji_or_digit(chars.next()?)?;
    (chars.next()? == '第').then(|| Marker {
        kind,
        ordinals: vec![n],
    })
}

fn kanji_or_digit(c: char) -> Option<u32> {
    const KANJI: &str = "一二三四五六七八九";
    c.to_digit(10)
        .or_else(|| KANJI.chars().position(|k| k == c).map(|i| i as u32 + 1))
}

/// Every ordinal a title names anywhere: each season or part marker,
/// every small number or numeral in it ("Tokyo Ghoul:re 2", "Mushoku
/// Tensei II: Isekai Ittara Honki Dasu"), and a Japanese 第N. Read as
/// evidence of what an entry names, so reading too much only ever
/// admits more.
pub(super) fn named_ordinals(title: &str) -> BTreeSet<u32> {
    let words = words(title);
    let mut out: BTreeSet<u32> = words
        .windows(2)
        .filter_map(|w| marker_of(&w[0], &w[1]))
        .flat_map(|m| m.ordinals)
        .collect();
    out.extend(
        words
            .iter()
            .filter_map(|w| small_number(w).or_else(|| roman(w))),
    );
    let chars: Vec<char> = title.chars().collect();
    for w in chars.windows(2) {
        if w[0] == '第' {
            out.extend(kanji_or_digit(w[1]));
        }
    }
    out
}

/// The part ordinals a title ends on.
pub(super) fn part_ordinals(title: &str) -> BTreeSet<u32> {
    trailing_markers(title)
        .into_iter()
        .filter(|m| m.kind == Kind::Part)
        .flat_map(|m| m.ordinals)
        .collect()
}

/// A title's words with the markers it ends on removed: the name the
/// show's seasons and parts share ("Attack on Titan" for "Attack on
/// Titan Season 3 Part 2").
pub(crate) fn stem(title: &str) -> Vec<String> {
    let mut words = words(title);
    while words.len() >= 2 {
        let n = words.len();
        if marker_of(&words[n - 2], &words[n - 1]).is_none() {
            break;
        }
        words.truncate(n - 2);
    }
    words
}

/// Words that name a part without numbering it.
const PART_MARKERS: &[&str] = &["part", "stage", "season", "cour"];

/// A title with its whitespace runs collapsed, lowercased.
pub(crate) fn normalized(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// The ordinal one word spells, if it spells one.
fn word_ordinal(word: &str) -> Option<u32> {
    const SPELLED: &[(&str, u32)] = &[
        ("first", 1),
        ("second", 2),
        ("third", 3),
        ("fourth", 4),
        ("fifth", 5),
        ("ii", 2),
        ("iii", 3),
        ("iv", 4),
        ("v", 5),
    ];
    if let Some((_, n)) = SPELLED.iter().find(|(w, _)| *w == word) {
        return Some(*n);
    }
    let digits = ["st", "nd", "rd", "th"]
        .iter()
        .find_map(|suffix| word.strip_suffix(suffix))
        .unwrap_or(word);
    digits.parse().ok()
}

/// The ordinal a short part marker names ("2nd stage", "part 3"):
/// every word a marker or an ordinal, exactly one ordinal, and a bare
/// number only beside a marker.
pub(crate) fn sole_ordinal(rest: &str) -> Option<u32> {
    let words: Vec<&str> = rest.split_whitespace().collect();
    if words.is_empty() || words.len() > 3 {
        return None;
    }
    let has_marker = words.iter().any(|w| PART_MARKERS.contains(w));
    let mut ordinal = None;
    for w in &words {
        if PART_MARKERS.contains(w) {
            continue;
        }
        let bare = w.chars().all(|c| c.is_ascii_digit());
        let n = word_ordinal(w).filter(|_| has_marker || !bare)?;
        if ordinal.replace(n).is_some() {
            return None;
        }
    }
    ordinal
}
