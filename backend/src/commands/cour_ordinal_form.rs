//! The ordinal-then-keyword cour form — a trailing `2nd Season` /
//! `Second Part` on a title, `-2nd-season` / `-2-season` /
//! `-second-season` on a Kitsu slug — read for [`super::cour`],
//! which composes it with the keyword-then-number form. The forms
//! are the page's own resolver's: its ten spelled ordinals, and a
//! number with any of the four ordinal suffixes, a bare number only
//! in a slug.

use super::cour::is_cour_keyword;

/// The suffixes a numeric ordinal carries — any of them on any
/// number, as the page reads it, so "2th" names cour 2 like "2nd".
const ORDINAL_SUFFIXES: &[&str] = &["st", "nd", "rd", "th"];

/// The spelled ordinals the page reads, cour 1 first.
const ORDINAL_WORDS: &[&str] = &[
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth",
];

/// The ordinal-then-keyword title form: a trailing `2nd Season` /
/// `Second Part`, the ordinal a whole token anchored to
/// start-of-string, whitespace or a colon, like the keyword is in
/// the other form. A bare number before the keyword is not a form
/// the page reads on a title.
pub(crate) fn in_title(trimmed: &str) -> Option<u32> {
    let (head, kw) = trimmed.rsplit_once(char::is_whitespace)?;
    if !is_cour_keyword(kw) {
        return None;
    }
    let head = head.trim_end();
    // The token before the keyword: what follows the last whitespace
    // or colon, walked by char so a non-ASCII prefix is left whole.
    let start = head
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace() || *c == ':')
        .map_or(0, |(i, c)| i + c.len_utf8());
    ordinal_value(&head[start..], false)
}

/// The ordinal-then-keyword slug form: a trailing `-2nd-season` /
/// `-2-season` / `-second-season`, the ordinal a whole segment; a
/// bare number is read here, as the page reads it in a slug.
pub(crate) fn in_slug(slug: &str) -> Option<u32> {
    let (head, kw) = slug.rsplit_once('-')?;
    if !is_cour_keyword(kw) {
        return None;
    }
    let ordinal = head.rsplit_once('-').map_or(head, |(_, segment)| segment);
    ordinal_value(ordinal, true)
}

/// The cour an ordinal token names: a spelled ordinal the page
/// reads, or a number carrying one of the ordinal suffixes; a bare
/// number only where `bare` allows it. Anything else names none.
fn ordinal_value(token: &str, bare: bool) -> Option<u32> {
    if let Some(i) = ORDINAL_WORDS
        .iter()
        .position(|word| token.eq_ignore_ascii_case(word))
    {
        return u32::try_from(i + 1).ok();
    }
    let lowered = token.to_ascii_lowercase();
    let digits = ORDINAL_SUFFIXES
        .iter()
        .find_map(|suffix| lowered.strip_suffix(suffix))
        .or(if bare { Some(lowered.as_str()) } else { None })?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}
