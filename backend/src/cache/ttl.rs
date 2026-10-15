//! TTL policy per cache key prefix.
//!
//! See `docs/architecture.md` §"Caching" for the table.

use std::time::Duration;

/// TTL for the AniList trending row.
pub const TRENDING_TTL: Duration = Duration::from_secs(60 * 60); // 1h

/// TTL for Kitsu seasonal / top / recent rows.
pub const DISCOVERY_TTL: Duration = Duration::from_secs(6 * 60 * 60); // 6h

/// TTL for per-anime metadata (`/anime/:id`).
pub const ANIME_DETAIL_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60); // 7d

/// TTL for episode lists. Shorter than ANIME_DETAIL_TTL because new
/// episodes appear weekly for currently-airing shows; a 1-day window
/// caps staleness at a manageable level without burning Kitsu calls.
/// For finished shows, the data is stable, but re-fetching once a day
/// is cheap.
pub const EPISODES_TTL: Duration = Duration::from_secs(24 * 60 * 60); // 1d

/// TTL for AniList `streamingEpisodes` lookups by MAL id, used to
/// backfill per-episode thumbnails Kitsu doesn't carry. Aligned with
/// [`EPISODES_TTL`] (1d) — Crunchyroll's per-episode metadata is
/// stable enough that we don't burn AniList calls re-fetching, but a
/// daily window picks up fresh stills as currently-airing shows
/// release.
pub const ANILIST_STREAMING_EPS_TTL: Duration = Duration::from_secs(24 * 60 * 60); // 1d

/// Short negative-cache TTL for AniList `streamingEpisodes` lookups
/// that error out (typically 429 in the home-page cold-load burst
/// where multiple Continue Watching rows each fire an AniList call
/// in parallel against the 30/min unauth limit). Caching empty for
/// a few minutes stops the burst from recurring on every navigation
/// while still letting retries happen after the rate-limit window
/// resets.
pub const ANILIST_STREAMING_EPS_ERROR_TTL: Duration = Duration::from_secs(5 * 60); // 5min

/// TTL for title-match cache (Kitsu/AniList → provider id).
pub const TITLE_MATCH_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60); // 30d

/// TTL for a show's external ids from Kitsu's mappings when both the
/// MAL and the AniList id are present. An id mapping does not move:
/// a Kitsu entry points at the same MAL and AniList entries for as
/// long as all three exist, so this matches [`TITLE_MATCH_TTL`].
pub const EXTERNAL_IDS_TTL: Duration = TITLE_MATCH_TTL;

/// TTL for a show's external ids when one or both are missing. What
/// is present does not move; what is missing may appear — Kitsu adds
/// the MAL mapping for a fresh seasonal show within days — so an
/// incomplete answer is re-asked daily. Kitsu is not the constrained
/// budget; AniList is, and the translations the missing id forces are
/// kept on their own under [`ANILIST_ID_MAP_TTL`].
pub const EXTERNAL_IDS_PARTIAL_TTL: Duration = Duration::from_secs(24 * 60 * 60); // 1d

/// TTL for an id AniList translated — a MAL id's AniList `mediaId`,
/// or an AniList media's `idMal`. The same reasoning as
/// [`EXTERNAL_IDS_TTL`]: the pairing is stable for as long as both
/// entries exist.
pub const ANILIST_ID_MAP_TTL: Duration = TITLE_MATCH_TTL;

/// TTL for "AniList does not index this id" — a MAL id with no media,
/// or a media with no `idMal` recorded. AniList indexes new shows and
/// links their MAL ids within days, so a month-long negative would
/// leave a new show unwritable on one tracker for a month; a day
/// keeps the detail page from re-asking on every open meanwhile.
pub const ANILIST_ID_MAP_MISS_TTL: Duration = Duration::from_secs(24 * 60 * 60); // 1d

/// TTL for cached play resolutions (canonical_title + mode + quality +
/// episode → upstream URL + referer + media_kind). 7d is
/// generous because the *real* validity gate is the per-read HEAD
/// check + an evict-on-player-failure feedback loop — a stale row
/// survives until the next access, where HEAD or playback failure
/// kicks it out. The TTL is a coarse absolute bound on top of that:
/// reads do not renew it, so every row expires seven days after it
/// was written, however often it is replayed.
pub const PLAY_RESOLUTION_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60); // 7d

/// TTL for an AniList banner remembered per Kitsu id from the trending
/// feed. Banners rarely change, and the row is only ever written from a
/// request the app made anyway, so a long life costs no traffic.
pub const ANILIST_BANNER_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60); // 30d

/// TTL for the mark on an image URL the upstream said is gone. As long
/// as a remembered banner lives, so that a banner forgotten for being
/// dead is not remembered again from a row older than the verdict.
pub const DEAD_IMAGE_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60); // 30d

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ttls_are_strictly_ordered_short_to_long() {
        // The hierarchy is fundamental to the cache discipline: trending is
        // the freshest signal, title-match is the most stable. Asserting
        // the order makes accidental swaps loud.
        assert!(TRENDING_TTL < DISCOVERY_TTL);
        assert!(DISCOVERY_TTL < EPISODES_TTL);
        assert!(EPISODES_TTL < ANIME_DETAIL_TTL);
        assert!(ANIME_DETAIL_TTL < TITLE_MATCH_TTL);
    }
}
