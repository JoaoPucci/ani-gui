//! The one way a command reads a show's external ids — the MAL and
//! AniList ids Kitsu's mappings carry for a Kitsu anime.
//!
//! Tracker id-resolution, the airing schedule, the banner and
//! episode-thumbnail backfills and aniskip all start from the same
//! `/anime/:id?include=mappings` answer. Reading it through one place
//! means one place decides what it costs.

use crate::app::AppState;
use crate::error::Result;
use crate::meta::kitsu::ExternalIds;

/// The MAL and AniList ids Kitsu maps `kitsu_id` to.
///
/// # Errors
/// Whatever [`crate::meta::kitsu::KitsuClient::external_ids_for_kitsu_id`]
/// answers with: Network / Upstream / ParseFailed.
pub(crate) async fn external_ids_for(state: &AppState, kitsu_id: &str) -> Result<ExternalIds> {
    state.kitsu.external_ids_for_kitsu_id(kitsu_id).await
}
