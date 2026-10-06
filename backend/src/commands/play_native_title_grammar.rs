//! The grammar of the season and part markers titles carry — one
//! parser, read by the title rules (`play_native_title_marker`) and by
//! the split detection's part reading (`play_native_part_title`).

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

impl Marker {
    /// Whether `named` ordinals cover the marker: a first part, or a
    /// span through it, always; anything else only when every ordinal
    /// it names is among them — "Part 0+2" is not covered by 2 alone.
    pub(super) fn named_by(&self, named: &BTreeSet<u32>) -> bool {
        self.ordinals.contains(&1) || self.ordinals.iter().all(|n| named.contains(n))
    }
}

/// A title's words, lowercased, split at anything that is neither a
/// letter, a digit, nor the `+` a span is written with. Full-width
/// digits read as their ASCII forms.
fn words(title: &str) -> Vec<String> {
    title
        .to_lowercase()
        .chars()
        .map(half_width)
        .collect::<String>()
        .split(|c: char| !(c.is_alphanumeric() || c == '+'))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// A full-width digit's ASCII form; any other character as it is.
fn half_width(c: char) -> char {
    match c {
        '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap_or(c),
        _ => c,
    }
}

fn kind_of(word: &str) -> Option<Kind> {
    match word {
        "season" => Some(Kind::Season),
        "part" | "cour" | "stage" => Some(Kind::Part),
        _ => None,
    }
}

/// The ordinal a word spells as an ordinal: "2nd", "second", "II". A
/// bare number is not one ("100" in "Mob Psycho 100").
fn spelled_ordinal(word: &str) -> Option<u32> {
    const SPELLED: &[(&str, u32)] = &[
        ("first", 1),
        ("second", 2),
        ("third", 3),
        ("fourth", 4),
        ("fifth", 5),
        ("sixth", 6),
        ("ii", 2),
        ("iii", 3),
        ("iv", 4),
        ("v", 5),
        ("vi", 6),
    ];
    if let Some((_, n)) = SPELLED.iter().find(|(w, _)| *w == word) {
        return Some(*n);
    }
    let digits = ["st", "nd", "rd", "th"]
        .iter()
        .find_map(|suffix| word.strip_suffix(suffix))?;
    number(digits)
}

/// A number of up to three digits — an ordinal beside a marker word.
fn number(word: &str) -> Option<u32> {
    (!word.is_empty() && word.len() <= 3 && word.chars().all(|c| c.is_ascii_digit()))
        .then(|| word.parse().ok())
        .flatten()
}

/// A one- or two-digit number: one a title names without a marker.
fn small_number(word: &str) -> Option<u32> {
    number(word).filter(|_| word.len() <= 2)
}

/// A cardinal spelled out, read only after a marker word ("Season
/// Two").
fn cardinal(word: &str) -> Option<u32> {
    const CARDINALS: &[&str] = &["one", "two", "three", "four", "five", "six"];
    CARDINALS
        .iter()
        .position(|w| *w == word)
        .map(|i| i as u32 + 1)
}

/// The ordinals the word after a marker names: "2", "2nd", "ii",
/// "two", or a span of plain numbers ("1+2").
fn ordinals_after_marker(word: &str) -> Option<Vec<u32>> {
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

/// The marker the words end on and how many words it takes: a pair
/// ([`marker_of`]), or an ordinal standing alone that is spelled as
/// one ("Overlord II", "X 2nd") — a sequel's number, read as its
/// season. A bare number alone is no marker.
fn last_marker(words: &[String]) -> Option<(Marker, usize)> {
    let n = words.len();
    if n >= 2 {
        if let Some(m) = marker_of(&words[n - 2], &words[n - 1]) {
            return Some((m, 2));
        }
    }
    let n = spelled_ordinal(words.last()?)?;
    Some((
        Marker {
            kind: Kind::Season,
            ordinals: vec![n],
        },
        1,
    ))
}

/// The markers `words` end on, last first, and the words before them.
fn split_markers(mut words: Vec<String>) -> (Vec<String>, Vec<Marker>) {
    let mut out = Vec::new();
    while let Some((marker, taken)) = last_marker(&words) {
        out.push(marker);
        words.truncate(words.len() - taken);
    }
    (words, out)
}

/// A title's tail parsed: the words before the markers it ends on,
/// and those markers, last first — a Japanese division (第N期, 第N部,
/// 第Nクール) and then the English markers before it. The one parse
/// every reader of a title's tail goes through.
fn parse_tail(title: &str) -> (Vec<String>, Vec<Marker>) {
    let (text, japanese) = match japanese_trailing(title) {
        Some((marker, before)) => (before, Some(marker)),
        None => (title, None),
    };
    let (before, english) = split_markers(words(text));
    (before, japanese.into_iter().chain(english).collect())
}

/// The markers a title ends on, last first: "Season 3 Part 2" ends on
/// both, "Part 4: Diamond is Unbreakable" on none, "進撃の巨人 第3期"
/// on its season.
pub(super) fn trailing_markers(title: &str) -> Vec<Marker> {
    parse_tail(title).1
}

/// The ordinal `rest` names when it is nothing but one marker naming
/// one ordinal — "2nd Stage", "Part 3", ": Season 2", "II", "第2期".
/// `None` for anything else, a span or two markers included.
pub(crate) fn sole_ordinal(rest: &str) -> Option<u32> {
    let (before, markers) = parse_tail(rest);
    match (before.is_empty(), markers.as_slice()) {
        (true, [only]) if only.ordinals.len() == 1 => Some(only.ordinals[0]),
        _ => None,
    }
}

/// The 第N期 / 第N部 / 第Nクール a title ends on, closing brackets
/// aside, with the text before its 第.
fn japanese_trailing(title: &str) -> Option<(Marker, &str)> {
    let trimmed = title.trim_end_matches(|c: char| c.is_whitespace() || ")）]】".contains(c));
    let (body, kind) = [
        ("期", Kind::Season),
        ("部", Kind::Part),
        ("クール", Kind::Part),
    ]
    .iter()
    .find_map(|(suffix, kind)| trimmed.strip_suffix(suffix).map(|b| (b, *kind)))?;
    let (before, number) = body.rsplit_once('第')?;
    let number: String = number.chars().map(half_width).collect();
    let marker = Marker {
        kind,
        ordinals: vec![japanese_number(&number)?],
    };
    Some((marker, before))
}

fn kanji_or_digit(c: char) -> Option<u32> {
    const KANJI: &str = "一二三四五六七八九";
    c.to_digit(10)
        .or_else(|| KANJI.chars().position(|k| k == c).map(|i| i as u32 + 1))
}

/// The number written after 第: ASCII digits, or one kanji digit.
fn japanese_number(text: &str) -> Option<u32> {
    if let Some(n) = number(text) {
        return Some(n);
    }
    let mut chars = text.chars();
    let n = kanji_or_digit(chars.next()?)?;
    chars.next().is_none().then_some(n)
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
            .filter_map(|w| small_number(w).or_else(|| spelled_ordinal(w))),
    );
    let half: String = title.chars().map(half_width).collect();
    // Japanese writes a season's number straight after the title, with
    // no space to make it a word of its own ("怪獣８号", "…生活２").
    let tail: String = half
        .chars()
        .rev()
        .take_while(char::is_ascii_digit)
        .collect();
    out.extend(small_number(&tail.chars().rev().collect::<String>()));
    for (_, after) in half
        .match_indices('第')
        .map(|(i, m)| half.split_at(i + m.len()))
    {
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        out.extend(number(&digits).or_else(|| after.chars().next().and_then(kanji_or_digit)));
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

/// The markers `words` open with, in order: marker pairs ("season 3
/// part 2", "2nd season"), ordinals standing alone ("second half war"
/// opens with its second) and Japanese divisions written as one word
/// ("第2期"), up to the first word that is none of them.
fn leading_markers(words: &[String]) -> Vec<Marker> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < words.len() {
        if let Some(m) = words.get(i + 1).and_then(|next| marker_of(&words[i], next)) {
            out.push(m);
            i += 2;
        } else if let Some((m, "")) = japanese_trailing(&words[i]) {
            out.push(m);
            i += 1;
        } else if let Some(n) = spelled_ordinal(&words[i]) {
            out.push(Marker {
                kind: Kind::Season,
                ordinals: vec![n],
            });
            i += 1;
        } else {
            break;
        }
    }
    out
}

/// The ordinals `sibling` names as divisions of its own right after
/// `wide`'s stem, leaving out any division `wide` itself ends on:
/// "Attack on Titan Season 3 Part 2" beside "Attack on Titan Season
/// 3" names part 2; "Gintama.: Silver Soul Arc - Second Half War"
/// beside "Gintama.: Silver Soul Arc" names 2; "Show Season 3 Recap"
/// beside "Show Season 3" and "Show Side Story 2" beside "Show" name
/// nothing. Empty when `sibling` does not start with the stem.
pub(super) fn later_divisions(sibling: &str, wide: &str) -> Vec<u32> {
    later_division_markers(sibling, wide)
        .into_iter()
        .flat_map(|m| m.ordinals)
        .collect()
}

/// The divisions behind [`later_divisions`], with their kinds.
pub(super) fn later_division_markers(sibling: &str, wide: &str) -> Vec<Marker> {
    let own = stem(wide);
    let words = words(sibling);
    if words.len() < own.len() || words[..own.len()] != *own {
        return Vec::new();
    }
    let wide_markers = trailing_markers(wide);
    leading_markers(&words[own.len()..])
        .into_iter()
        .filter(|m| !wide_markers.contains(m))
        .collect()
}

/// The ordinals a title names other than as a part: every ordinal its
/// stem names ([`named_ordinals`] — "Show 2", "ショー２", "Overlord
/// II") and each season marker it ends on ("Show 2nd Season").
pub(super) fn season_ordinals(title: &str) -> BTreeSet<u32> {
    let (before, markers) = parse_tail(title);
    let mut out = named_ordinals(&before.join(" "));
    out.extend(
        markers
            .into_iter()
            .filter(|m| m.kind == Kind::Season)
            .flat_map(|m| m.ordinals),
    );
    out
}

/// A title's words with the markers it ends on removed: the name the
/// show's seasons and parts share ("Attack on Titan" for "Attack on
/// Titan Season 3 Part 2").
pub(crate) fn stem(title: &str) -> Vec<String> {
    parse_tail(title).0
}

/// A title with its whitespace runs collapsed, lowercased.
pub(crate) fn normalized(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
