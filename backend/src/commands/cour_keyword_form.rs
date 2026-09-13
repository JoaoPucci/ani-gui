//! The keyword-then-number cour form — a trailing `Part N` /
//! `Cour N` / `Season N` on a title, `-part-N` / `-cour-N` /
//! `-season-N` on a Kitsu slug — read for [`super::cour`], which
//! composes it with the ordinal form.

use super::cour::COUR_KEYWORDS;

/// The keyword-then-number title form: a trailing `Part N` / `Cour N`
/// / `Season N`, the keyword anchored to start-of-string, whitespace
/// or a colon.
pub(crate) fn in_title(trimmed: &str) -> Option<u32> {
    // Walk back from the end to read a trailing decimal number.
    let (digits_start, _) = trailing_digits(trimmed)?;
    let n: u32 = trimmed[digits_start..].parse().ok()?;
    // Skip whitespace between keyword and digits.
    let after_kw = trimmed[..digits_start].trim_end();
    let kw_end = after_kw.len();
    // `kw_start = kw_end - kw.len()` is a byte offset; non-ASCII
    // titles ("アニメ123") can land it mid-codepoint, so go through
    // `str::get` (returns `None` when the index isn't a char
    // boundary) instead of slicing directly.
    let kw_start = COUR_KEYWORDS.iter().find_map(|kw| {
        let want = kw.len();
        if kw_end < want {
            return None;
        }
        let kw_start = kw_end - want;
        if after_kw.get(kw_start..)?.eq_ignore_ascii_case(kw) {
            Some(kw_start)
        } else {
            None
        }
    })?;
    // The keyword must be preceded by start-of-string, whitespace, or
    // a colon. This is what keeps "Part 6: Stone Ocean" (mid-title)
    // from matching when the real suffix is e.g. "Stone Ocean" alone.
    // Walk back by char rather than by byte so non-ASCII prefixes
    // ("アニメ Part 2") are checked correctly.
    match after_kw[..kw_start].chars().next_back() {
        None => Some(n),
        Some(c) if c == ':' || c.is_whitespace() => Some(n),
        _ => None,
    }
}

/// The keyword-then-number slug form: a trailing `-part-N` /
/// `-cour-N` / `-season-N`, the keyword a whole segment.
pub(crate) fn in_slug(slug: &str) -> Option<u32> {
    let (digits_start, _) = trailing_digits(slug)?;
    let n: u32 = slug[digits_start..].parse().ok()?;
    // Must be preceded by `-(part|cour|season)-`.
    if digits_start == 0 {
        return None;
    }
    if slug.as_bytes()[digits_start - 1] != b'-' {
        return None;
    }
    let before_dash = &slug[..digits_start - 1];
    COUR_KEYWORDS.iter().find_map(|kw| {
        let want = kw.len();
        if before_dash.len() < want {
            return None;
        }
        let kw_start = before_dash.len() - want;
        if !before_dash[kw_start..].eq_ignore_ascii_case(kw) {
            return None;
        }
        // The keyword must be preceded by start-of-string or `-`,
        // anchoring the match as a real slug segment.
        if kw_start == 0 || before_dash.as_bytes()[kw_start - 1] == b'-' {
            Some(n)
        } else {
            None
        }
    })
}

/// Find the byte index where the trailing ASCII-digit run starts in
/// `s`, plus the digit run's length. Returns `None` when `s` has no
/// trailing digits or is empty.
fn trailing_digits(s: &str) -> Option<(usize, usize)> {
    if s.is_empty() {
        return None;
    }
    let bytes = s.as_bytes();
    let mut i = bytes.len();
    while i > 0 && bytes[i - 1].is_ascii_digit() {
        i -= 1;
    }
    if i == bytes.len() {
        None
    } else {
        Some((i, bytes.len() - i))
    }
}
