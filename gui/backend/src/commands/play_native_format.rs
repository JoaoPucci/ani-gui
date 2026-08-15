//! The picker's format disproof — split from `play_native` so each
//! file stays inside the complexity ratchet's per-file bar.

use crate::scraper::anidb::BrowseHit;

/// The layer the provider picker carried as its type filter, applied
/// in BOTH directions: a card badged `Movie` cannot be the
/// multi-episode series the caller expects — nor the clicked entry
/// when Kitsu says it is anything but a movie (the special-vs-movie
/// shape: both are single videos, count-tied, and only the format
/// tells them apart) — and a card with any OTHER known badge cannot
/// be the entry Kitsu says IS a movie. Unknown badges never exclude,
/// an absent subtype keeps single-video pools permissive, and a pool
/// left empty here is disproven — the next alias may carry the real
/// show.
///
/// Runs over the RAW hit list, before the bounded probe head is
/// taken: the badge costs nothing to read, and filtered after the
/// truncation five format-incompatible decoys ranked first would
/// crowd the real show out of the probe slots entirely.
pub(crate) fn format_survivors(
    hits: &[BrowseHit],
    expected: Option<u32>,
    subtype: Option<&str>,
) -> Vec<BrowseHit> {
    hits.iter()
        .filter(|h| format_compatible(h.kind.as_deref(), expected, subtype))
        .cloned()
        .collect()
}

/// Whether one format tag is compatible with the caller's
/// expectation — the single predicate behind [`format_survivors`]
/// and the subprocess picker's pool filter, so the native and
/// script-driven paths disprove formats identically.
pub(crate) fn format_compatible(
    kind: Option<&str>,
    expected: Option<u32>,
    subtype: Option<&str>,
) -> bool {
    let want = subtype.and_then(format_category);
    let have = kind.and_then(format_category);
    if let (Some(w), Some(h)) = (want, have) {
        // Both sides known: the categories must agree — a special is
        // not a TV entry is not an OVA, and for one-video same-title
        // same-year entries the tag is the only separating signal.
        return w == h;
    }
    // No category verdict (either side unknown or subtype absent):
    // the count-derived movie exclusion still applies — a card
    // badged movie-shaped cannot be the multi-episode series.
    let expects_non_movie = want.is_none() && matches!(expected, Some(n) if n > 1);
    !(expects_non_movie && have == Some("movie"))
}

/// The category a format tag names, unifying Kitsu subtypes and the
/// provider's badges case-insensitively. `None` for tags neither
/// side is known to emit — unknown never excludes.
fn format_category(tag: &str) -> Option<&'static str> {
    match tag.to_ascii_lowercase().as_str() {
        "movie" => Some("movie"),
        "tv" => Some("tv"),
        "ova" => Some("ova"),
        "special" => Some("special"),
        // Kitsu says ONA where the provider badges Web.
        "ona" | "web" => Some("ona"),
        _ => None,
    }
}

#[cfg(test)]
#[path = "play_native_format_test.rs"]
mod tests;
