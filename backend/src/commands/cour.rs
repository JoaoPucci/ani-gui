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
//! pulling it in would be heavier than the helper itself. The two
//! forms live in sibling modules — [`super::cour_keyword_form`] for
//! the keyword-then-number form, [`super::cour_ordinal_form`] for
//! the ordinal-then-keyword form — and this module composes them.

use super::{cour_keyword_form, cour_ordinal_form};

pub(crate) const COUR_KEYWORDS: &[&str] = &["part", "cour", "season"];

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
    cour_keyword_form::in_title(trimmed).or_else(|| cour_ordinal_form::in_title(trimmed))
}

/// Whether `token` is one of the cour keywords, case-insensitively.
pub(crate) fn is_cour_keyword(token: &str) -> bool {
    COUR_KEYWORDS
        .iter()
        .any(|kw| token.eq_ignore_ascii_case(kw))
}

/// Extract the cour number from a trailing `-part-N` / `-cour-N` /
/// `-season-N` suffix on a Kitsu slug. Returns `None` for bare slugs.
#[must_use]
pub fn cour_from_slug(slug: &str) -> Option<u32> {
    cour_keyword_form::in_slug(slug).or_else(|| cour_ordinal_form::in_slug(slug))
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

#[cfg(test)]
#[path = "cour_test.rs"]
mod tests;
