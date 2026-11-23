//! The streaming-listing lookups of a lane, coalesced
//! ([`crate::meta::anilist_batch`]'s lanes). Split from [`super`] so
//! each file stays inside the CRAP gate's per-file bar.

use super::batch::Streaming;
use super::StreamingListing;
use crate::error::Result;
use crate::meta::anilist_batch::{coalesced, Want};

/// `want`'s streaming listing, asked with the other lookups of `lane`
/// that miss at the same time.
///
/// # Errors
/// As [`super::streaming_listings_batch`] for the show;
/// [`crate::error::AniError::Network`] when the batch was lost before
/// it answered.
pub(crate) async fn streaming_listing_coalesced(
    client: &reqwest::Client,
    lane: &str,
    want: Want,
    base_override: Option<&str>,
) -> Result<StreamingListing> {
    coalesced::<Streaming>(client, lane, want, base_override).await
}

#[cfg(test)]
#[path = "anilist_streaming_eps_coalesce_test.rs"]
mod tests;
