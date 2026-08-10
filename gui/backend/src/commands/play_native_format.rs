//! The picker's format disproof — split from `play_native` so each
//! file stays inside the complexity ratchet's per-file bar.

use crate::scraper::anidb::BrowseHit;

/// The layer the allanime picker carried as its type filter, applied
/// in BOTH directions: a card badged `Movie` cannot be the
/// multi-episode series the caller expects — nor the clicked entry
/// when Kitsu says it is anything but a movie (the special-vs-movie
/// shape: both are single videos, count-tied, and only the format
/// tells them apart) — and a card with any OTHER known badge cannot
/// be the entry Kitsu says IS a movie. Unknown badges never exclude,
/// an absent subtype keeps single-video pools permissive, and a pool
/// left empty here is disproven — the next alias may carry the real
/// show.
pub(crate) fn format_filtered<'a>(
    head: Vec<(&'a BrowseHit, bool)>,
    expected: Option<u32>,
    subtype: Option<&str>,
) -> Vec<(&'a BrowseHit, bool)> {
    let expects_movie = subtype.is_some_and(|s| s.eq_ignore_ascii_case("movie"));
    let expects_non_movie = matches!(expected, Some(n) if n > 1)
        || subtype.is_some_and(|s| !s.eq_ignore_ascii_case("movie"));
    if expects_movie {
        head.into_iter()
            .filter(|(h, _)| {
                h.kind
                    .as_deref()
                    .is_none_or(|k| k.eq_ignore_ascii_case("movie"))
            })
            .collect()
    } else if expects_non_movie {
        head.into_iter()
            .filter(|(h, _)| h.kind.as_deref() != Some("Movie"))
            .collect()
    } else {
        head
    }
}
