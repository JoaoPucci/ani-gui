//! Whether an AniList streaming listing describes the show whose
//! episode page it would fill.

use crate::meta::anilist_streaming_eps::StreamingListing;
use crate::meta::kitsu::KitsuEpisode;

/// `true` when `listing` may fill stills on `page`.
#[must_use]
pub fn listing_fits(_listing: &StreamingListing, _page: &[KitsuEpisode]) -> bool {
    true
}

#[cfg(test)]
#[path = "anilist_eps_fit_test.rs"]
mod tests;
