//! Cour-detection helpers shared by the mark-watched integrity guard.
//!
//! The reverse cache (`allmanga show_id → kitsu_id`) used to record
//! cross-cour mappings — e.g. Stone Ocean Part 2's allmanga show_id
//! paired with Part 1's Kitsu id — because the picker can pick a
//! sibling cour when ep-count and year tie. The guard reads the
//! allmanga show's title (cour from trailing "Part N" / "Cour N" /
//! "Season N") and compares it against the Kitsu slug's trailing
//! "-part-N" / "-cour-N" / "-season-N". Mismatch → reject the write.
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

/// Extract the cour number from a trailing `Part N` / `Cour N` /
/// `Season N` suffix on an allmanga show name. Returns `None` for
/// bare titles or when the only "Part N" / etc. is mid-title (parent
/// series name).
#[must_use]
pub fn cour_from_title(name: &str) -> Option<u32> {
    let trimmed = name.trim_end();
    // `play_resolution_cache::put` writes show_title as
    // "<name> (<N> episodes)" (see commands/play.rs). Strip that
    // bookkeeping suffix first; otherwise the trailing chars are
    // "episodes)" and every production cache row returns None.
    let trimmed = strip_trailing_episode_count(trimmed);
    // Walk back from the end to read a trailing decimal number.
    let (digits_start, _) = trailing_digits(trimmed)?;
    let n: u32 = trimmed[digits_start..].parse().ok()?;
    // Skip whitespace between keyword and digits.
    let after_kw = trimmed[..digits_start].trim_end();
    let kw_end = after_kw.len();
    let kw_start = COUR_KEYWORDS.iter().find_map(|kw| {
        let want = kw.len();
        if kw_end < want {
            return None;
        }
        let kw_start = kw_end - want;
        if after_kw[kw_start..].eq_ignore_ascii_case(kw) {
            Some(kw_start)
        } else {
            None
        }
    })?;
    // The keyword must be preceded by start-of-string, whitespace, or
    // a colon. This is what keeps "Part 6: Stone Ocean" (mid-title)
    // from matching when the real suffix is e.g. "Stone Ocean" alone.
    if kw_start == 0 {
        return Some(n);
    }
    let prev_byte = after_kw.as_bytes()[kw_start - 1];
    if prev_byte == b':' || (prev_byte as char).is_whitespace() {
        Some(n)
    } else {
        None
    }
}

/// Extract the cour number from a trailing `-part-N` / `-cour-N` /
/// `-season-N` suffix on a Kitsu slug. Returns `None` for bare slugs.
#[must_use]
pub fn cour_from_slug(slug: &str) -> Option<u32> {
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

/// Whether the allmanga-derived cour and the Kitsu-slug-derived cour
/// refer to the same cour of the same franchise. `None` on either
/// side normalizes to cour 1 (the parent), so a bare allmanga title
/// paired with a `-part-1` Kitsu slug agrees, and a `Part 2` allmanga
/// title paired with a bare Kitsu slug disagrees.
#[must_use]
pub fn cours_agree(allmanga: Option<u32>, kitsu: Option<u32>) -> bool {
    allmanga.unwrap_or(1) == kitsu.unwrap_or(1)
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
