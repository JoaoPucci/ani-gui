//! The marks a title is written with and the release year it is
//! tagged with — what tells two titles of one name apart where their
//! words do not: "Gintama'" from "Gintama", "Film (2020)" from "Film
//! (2019)". Split from `play_native_title_own` for the per-file
//! complexity bar.

use super::play_native_title_grammar::split_tags;

/// Marks that separate words rather than belong to them — spacing
/// punctuation, the brackets around a tag or a division, and the `+` a
/// span of parts is written with — in ASCII and full-width forms.
const NEUTRAL: &str = ":-–—~,・、：－～，()[]（）【】「」『』+＋";

/// The marks `title` is written with, in order, and the release years
/// it is tagged with, sorted. A mark is any character that is neither a
/// letter, a digit, a space nor [`NEUTRAL`]; full-width marks read as
/// their ASCII forms and every prime or apostrophe as `'`. A format tag
/// ("(TV)", "[OVA]") is no mark — the format badge speaks for it — while
/// a year tag ("(2011)") is the release a remake is told by.
pub(crate) fn title_marks(title: &str) -> (String, Vec<String>) {
    let (text, tags) = split_tags(title);
    let marks = text
        .chars()
        .map(ascii_mark)
        .filter(|c| !c.is_alphanumeric() && !c.is_whitespace() && !NEUTRAL.contains(*c))
        .collect();
    let years = tags
        .into_iter()
        .filter(|t| t.chars().all(|c| c.is_ascii_digit()))
        .collect();
    (marks, years)
}

/// A full-width mark's ASCII form, and every prime or apostrophe as
/// `'`; any other character as it is.
fn ascii_mark(c: char) -> char {
    match c {
        '\u{2019}' | '\u{2018}' | '\u{2032}' | '\u{00B4}' | '`' | '\u{02B9}' | '\u{FF07}' => '\'',
        '！'..='～' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
        _ => c,
    }
}
