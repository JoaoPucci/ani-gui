//! The streaming-listing kind of the batched lookup
//! ([`crate::meta::anilist_batch`]): episode stills for the Continue
//! Watching strip and any rail filling several shows at once. Split
//! from [`super`] so each file stays inside the CRAP gate's per-file
//! bar.

use super::parse::{listing_of, Media};
use super::{streaming_listing_for_ids, StreamingListing};
#[cfg(test)]
use crate::error::AniError;
use crate::error::Result;
#[cfg(test)]
use crate::meta::anilist_batch::parse_page;
use crate::meta::anilist_batch::{batch, Kind, Lanes, Want};
#[cfg(test)]
use crate::meta::anilist_batch::{resolve, Space};
use crate::meta::anilist_gate::Purpose;
#[cfg(test)]
use crate::meta::anilist_pages::{Page, PAGE_CAP};

/// The streaming-listing lookup, batched.
pub(crate) struct Streaming;

impl Kind for Streaming {
    const PURPOSE: Purpose = Purpose::Stills;
    type Out = StreamingListing;
    /// [`super::STREAMING_EPS_BY_MAL_GQL`] for a page of MAL ids.
    const BY_MAL_GQL: &'static str = "query StreamingEpsBatchByMal($malIds: [Int]) { \
        Page(page: 1, perPage: 50) { pageInfo { hasNextPage } \
            media(idMal_in: $malIds, type: ANIME) { \
                idMal episodes status streamingEpisodes { title thumbnail } \
            } \
        } \
    }";
    const BY_ID_GQL: &'static str = "query StreamingEpsBatchById($ids: [Int]) { \
        Page(page: 1, perPage: 50) { pageInfo { hasNextPage } \
            media(id_in: $ids, type: ANIME) { \
                id episodes status streamingEpisodes { title thumbnail } \
            } \
        } \
    }";
    const FIELDS: &'static [&'static str] = &["episodes", "status", "streamingEpisodes"];

    fn lanes() -> &'static Lanes<StreamingListing> {
        static LANES: std::sync::LazyLock<Lanes<StreamingListing>> =
            std::sync::LazyLock::new(Default::default);
        &LANES
    }

    fn read(node: serde_json::Value) -> Option<StreamingListing> {
        serde_json::from_value::<Media>(node).ok().map(listing_of)
    }

    fn single(
        client: &reqwest::Client,
        want: Want,
        base_override: Option<&str>,
    ) -> impl std::future::Future<Output = Result<StreamingListing>> + Send {
        streaming_listing_for_ids(client, want.mal, want.anilist, base_override)
    }
}

/// The streaming listing of each show in `wants`, in order — see
/// [`batch`].
pub async fn streaming_listings_batch(
    client: &reqwest::Client,
    wants: &[Want],
    base_override: Option<&str>,
) -> Vec<Result<StreamingListing>> {
    batch::<Streaming>(client, wants, base_override).await
}

/// One page of a streaming-listing batch, as [`parse_page`] reads it.
#[cfg(test)]
pub(crate) fn parse_streaming_page(
    body: &[u8],
    space: Space,
) -> Result<Page<crate::meta::anilist_batch::Keyed<StreamingListing>>> {
    parse_page(body, space, Streaming::FIELDS, Streaming::read)
}

#[cfg(test)]
#[path = "anilist_streaming_eps_batch_test.rs"]
mod tests;

#[cfg(test)]
#[path = "anilist_streaming_eps_batch_prop_test.rs"]
mod prop_tests;
