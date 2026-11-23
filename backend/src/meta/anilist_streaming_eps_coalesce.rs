//! Coalescing concurrent streaming-listing lookups (stub).

use super::{streaming_listing_for_ids, StreamingListing, Want};
use crate::error::Result;

/// Stub: the single lookup.
pub(crate) async fn streaming_listing_coalesced(
    client: &reqwest::Client,
    _lane: &str,
    want: Want,
    base_override: Option<&str>,
) -> Result<StreamingListing> {
    streaming_listing_for_ids(client, want.mal, want.anilist, base_override).await
}
