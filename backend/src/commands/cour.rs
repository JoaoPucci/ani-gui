//! Cour-detection helpers shared by the mark-watched integrity guard.
//!
//! The reverse cache (`provider show_id → kitsu_id`) used to record
//! cross-cour mappings — e.g. Stone Ocean Part 2's provider show_id
//! paired with Part 1's Kitsu id — because the picker can pick a
//! sibling cour when ep-count and year tie. The guard reads the
//! provider show's title (cour from trailing "Part N" / "Cour N" /
//! "Season N") and compares it against the Kitsu slug's trailing
//! "-part-N" / "-cour-N" / "-season-N". Mismatch → reject the write.
//!
//! Kitsu writes the cour before the keyword as often as after it —
//! "2nd Season", "Second Season", and in slugs `2nd-season`,
//! `2-season`, `second-season` — and the page's own resolver reads
//! every one of those forms, so both parsers here read them too:
//! the ten spelled ordinals the page knows, and a number with any of
//! the four ordinal suffixes (a bare number only in a slug, where the
//! page reads it), each still trailing and anchored.
//!
//! Trailing-only matching is deliberate. "JoJo no Kimyou na Bouken
//! Part 6: Stone Ocean" has a mid-title "Part 6" that names the
//! parent series, not a cour; the matchers below anchor to end-of-
//! string so they don't trip on that.
//!
//! Written by hand rather than via the `regex` crate — `regex` isn't
//! a direct dependency and the patterns are simple enough that
//! pulling it in would be heavier than the helper itself.

const COUR_KEYWORDS: &[&str] = &["part", "cour", "season"];

/// The suffixes a numeric ordinal carries — any of them on any
/// number, as the page reads it, so "2th" names cour 2 like "2nd".
const ORDINAL_SUFFIXES: &[&str] = &["st", "nd", "rd", "th"];

/// The spelled ordinals the page reads, cour 1 first.
const ORDINAL_WORDS: &[&str] = &[
    "first", "second", "third", "fourth", "fifth", "sixth", "seventh", "eighth", "ninth", "tenth",
];

/// Extract the cour number from a trailing `Part N` / `Cour N` /
/// `Season N` suffix on a provider show name. Returns `None` for
/// bare titles or when the only "Part N" / etc. is mid-title (parent
/// series name).
#[must_use]
pub fn cour_from_title(name: &str) -> Option<u32> {
    let trimmed = name.trim_end();
    // Legacy cache rows carry show_title as "<name> (<N> episodes)";
    // the native writer stores the provider's card title verbatim and
    // appends nothing. Strip the suffix when it is there — on a row
    // that has one the trailing chars are otherwise "episodes)" and
    // the cour never parses.
    let trimmed = strip_trailing_episode_count(trimmed);
    keyword_then_number_in_title(trimmed).or_else(|| ordinal_then_keyword_in_title(trimmed))
}

/// The keyword-then-number title form: a trailing `Part N` / `Cour N`
/// / `Season N`, the keyword anchored to start-of-string, whitespace
/// or a colon.
fn keyword_then_number_in_title(trimmed: &str) -> Option<u32> {
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

/// The ordinal-then-keyword title form: a trailing `2nd Season` /
/// `Second Part`, the ordinal a whole token anchored to
/// start-of-string, whitespace or a colon, like the keyword is in
/// the other form. A bare number before the keyword is not a form
/// the page reads on a title.
fn ordinal_then_keyword_in_title(trimmed: &str) -> Option<u32> {
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

/// Whether `token` is one of the cour keywords, case-insensitively.
fn is_cour_keyword(token: &str) -> bool {
    COUR_KEYWORDS
        .iter()
        .any(|kw| token.eq_ignore_ascii_case(kw))
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

/// Extract the cour number from a trailing `-part-N` / `-cour-N` /
/// `-season-N` suffix on a Kitsu slug. Returns `None` for bare slugs.
#[must_use]
pub fn cour_from_slug(slug: &str) -> Option<u32> {
    keyword_then_number_in_slug(slug).or_else(|| ordinal_then_keyword_in_slug(slug))
}

/// The keyword-then-number slug form: a trailing `-part-N` /
/// `-cour-N` / `-season-N`, the keyword a whole segment.
fn keyword_then_number_in_slug(slug: &str) -> Option<u32> {
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

/// The ordinal-then-keyword slug form: a trailing `-2nd-season` /
/// `-2-season` / `-second-season`, the ordinal a whole segment; a
/// bare number is read here, as the page reads it in a slug.
fn ordinal_then_keyword_in_slug(slug: &str) -> Option<u32> {
    let (head, kw) = slug.rsplit_once('-')?;
    if !is_cour_keyword(kw) {
        return None;
    }
    let ordinal = head.rsplit_once('-').map_or(head, |(_, segment)| segment);
    ordinal_value(ordinal, true)
}

/// Whether the provider-derived cour and the Kitsu-slug-derived cour
/// refer to the same cour of the same franchise. `None` on either
/// side normalizes to cour 1 (the parent), so a bare provider title
/// paired with a `-part-1` Kitsu slug agrees, and a `Part 2` provider
/// title paired with a bare Kitsu slug disagrees.
#[must_use]
pub fn cours_agree(provider: Option<u32>, kitsu: Option<u32>) -> bool {
    provider.unwrap_or(1) == kitsu.unwrap_or(1)
}

/// Whether a Kitsu search hit's slug disagrees with the cour a
/// search term carries: the term's trailing `Part N` against the
/// slug's trailing `-part-N`, a slug without a suffix being the
/// parent cour. A term without cour evidence, or a hit without a
/// slug, disagrees with nothing — the same silence rule as the
/// mapping guard's, which this reads without a detail fetch since
/// the hit carries its slug.
#[must_use]
pub fn hit_cour_disagrees(term: &str, kitsu_slug: Option<&str>) -> bool {
    match (cour_from_title(term), kitsu_slug) {
        (Some(term_cour), Some(slug)) => term_cour != cour_from_slug(slug).unwrap_or(1),
        _ => false,
    }
}

/// Strip a trailing ` (<digits> episodes)` segment from a title, if
/// present. Returns the original slice when no suffix matches so the
/// caller can fall through transparently.
fn strip_trailing_episode_count(s: &str) -> &str {
    let Some(inner) = s.strip_suffix(')') else {
        return s;
    };
    let Some(inner) = inner.strip_suffix(" episodes") else {
        return s;
    };
    let Some(open) = inner.rfind('(') else {
        return s;
    };
    let digits = &inner[open + 1..];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return s;
    }
    inner[..open].trim_end()
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

#[cfg(test)]
#[path = "cour_test.rs"]
mod tests;
