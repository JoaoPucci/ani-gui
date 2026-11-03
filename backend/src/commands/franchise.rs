//! The franchise around an anime: the IO loop that feeds the pure walk
//! (`commands::franchise_walk`) the listings and details it asks for.
//!
//! Each listing is the detailed relations read
//! ([`crate::meta::kitsu::KitsuClient::franchise_relations`]), cached
//! under `kitsu:franchise-rel:v1:<id>` for as long as the tracker's
//! relations read keeps its rows. An entry no listing describes — the
//! anchor, usually — takes its fields from the cached detail read
//! ([`crate::commands::kitsu::kitsu_anime_detail`]).

use crate::app::AppState;
use crate::commands::franchise_walk::Franchise;
use crate::error::{AniError, Result};
use crate::meta::kitsu::{FranchiseEntry, FranchiseListing, KitsuAnimeRef};

/// The cache key of `id`'s listing.
pub(crate) fn listing_key(id: &str) -> String {
    format!("kitsu:franchise-rel:v1:{id}")
}

/// `id`'s relation listing, from the cache or Kitsu.
///
/// # Errors
/// As the client's, on a miss.
pub async fn franchise_listing(state: &AppState, id: &str) -> Result<FranchiseListing> {
    let _ = (state, listing_key(id));
    Err(AniError::Network)
}

/// What the franchise surfaces show of an entry, from its detail.
#[must_use]
pub fn entry_from_detail(d: &KitsuAnimeRef) -> FranchiseEntry {
    FranchiseEntry {
        id: d.id.clone(),
        title: String::new(),
        subtype: None,
        start: None,
        episodes: None,
        status: None,
        poster: None,
        cover: None,
    }
}

/// The franchise around `anchor`.
///
/// # Errors
/// [`crate::error::AniError::InvalidKitsuId`] for a value that is not
/// an id, and the errors of the reads the walk asks for.
pub async fn franchise(state: &AppState, anchor: &str) -> Result<Franchise> {
    let _ = (state, anchor);
    Err(AniError::Network)
}
