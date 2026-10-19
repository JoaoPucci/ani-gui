//! The words a title is read in and the numbers its words write —
//! split from `play_native_title_grammar` for the per-file complexity
//! bar.

use super::*;

/// A title's words, lowercased and without its catalogue tags
/// ([`untagged`]), split at anything that is neither a
/// letter, a digit, nor the `+` a span is written with. Full-width
/// digits read as their ASCII forms.
pub(crate) fn words(title: &str) -> Vec<String> {
    untagged(title)
        .chars()
        .map(half_width)
        .collect::<String>()
        .split(|c: char| !(c.is_alphanumeric() || c == '+'))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// A full-width digit's ASCII form; any other character as it is.
pub(crate) fn half_width(c: char) -> char {
    match c {
        '０'..='９' => char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap_or(c),
        _ => c,
    }
}

pub(crate) fn kind_of(word: &str) -> Option<Kind> {
    match word {
        "season" => Some(Kind::Season),
        "part" | "cour" | "stage" => Some(Kind::Part),
        _ => None,
    }
}

/// The ordinal a word spells as an ordinal: "2nd", "second" to
/// "twentieth", "II" to "IX" — never "X", a word of its own in titles
/// like "Spy x Family". A bare number is not one ("100" in "Mob Psycho
/// 100").
pub(crate) fn spelled_ordinal(word: &str) -> Option<u32> {
    const SPELLED: &[(&str, u32)] = &[
        ("first", 1),
        ("second", 2),
        ("third", 3),
        ("fourth", 4),
        ("fifth", 5),
        ("sixth", 6),
        ("seventh", 7),
        ("eighth", 8),
        ("ninth", 9),
        ("tenth", 10),
        ("eleventh", 11),
        ("twelfth", 12),
        ("thirteenth", 13),
        ("fourteenth", 14),
        ("fifteenth", 15),
        ("sixteenth", 16),
        ("seventeenth", 17),
        ("eighteenth", 18),
        ("nineteenth", 19),
        ("twentieth", 20),
        ("ii", 2),
        ("iii", 3),
        ("iv", 4),
        ("v", 5),
        ("vi", 6),
        ("vii", 7),
        ("viii", 8),
        ("ix", 9),
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
pub(crate) fn number(word: &str) -> Option<u32> {
    (!word.is_empty() && word.len() <= 3 && word.chars().all(|c| c.is_ascii_digit()))
        .then(|| word.parse().ok())
        .flatten()
}

/// A one- or two-digit number: one a title names without a marker.
pub(crate) fn small_number(word: &str) -> Option<u32> {
    number(word).filter(|_| word.len() <= 2)
}

/// A cardinal spelled out, "one" to "twenty", read only after a marker
/// word ("Season Two").
pub(crate) fn cardinal(word: &str) -> Option<u32> {
    const CARDINALS: &[&str] = &[
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
        "twenty",
    ];
    CARDINALS
        .iter()
        .position(|w| *w == word)
        .map(|i| i as u32 + 1)
}

/// A one- or two-digit number glued to the end of a Japanese word
/// ("ショー2", "怪獣8号" aside — that one ends on a kanji). After any
/// other letter the digits are the word's own: "Show2", "ＳＨＯＷ２",
/// "Café2".
pub(crate) fn glued_number(word: &str) -> Option<u32> {
    let digits = word.len() - word.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    let (head, tail) = word.split_at(word.len() - digits);
    head.chars()
        .last()
        .is_some_and(is_japanese)
        .then(|| small_number(tail))
        .flatten()
}

/// Whether `c` is a Japanese letter: a letter of the Hiragana and
/// Katakana blocks, of the CJK ideographs' main block or its first
/// extension, the iteration and closing marks (々, 〆, 〇), or a letter
/// of half-width katakana. The long-vowel marks ー and ｰ and the
/// half-width sound marks ﾞ and ﾟ are letters; ・, ゠ and the spacing
/// sound marks ゛ and ゜ are none.
pub(crate) fn is_japanese(c: char) -> bool {
    matches!(
        c,
        '\u{3005}'..='\u{3007}'
            | '\u{3040}'..='\u{30FF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{FF66}'..='\u{FF9F}'
    ) && c.is_alphanumeric()
}
