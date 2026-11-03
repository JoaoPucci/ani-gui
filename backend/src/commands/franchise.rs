//! The franchise around an anime: the IO loop that feeds the pure walk
//! (`commands::franchise_walk`) the listings and details it asks for.
//!
//! Each listing is the detailed relations read
//! ([`crate::meta::kitsu::KitsuClient::franchise_relations`]), cached
//! under `kitsu:franchise-rel:v1:<id>` for as long as the tracker's
//! relations read keeps its rows. An entry no listing describes — the
//! anchor, usually — takes its fields from the cached detail read
//! ([`crate::commands::kitsu::kitsu_anime_detail`]).

use std::collections::BTreeMap;

use futures_util::stream::{self, StreamExt};

use crate::app::AppState;
use crate::cache::ttl::ANIME_DETAIL_TTL;
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::commands::franchise_walk::{plan, Franchise, Plan};
use crate::error::Result;
use crate::meta::kitsu::{FranchiseEntry, FranchiseListing, KitsuAnimeRef};

/// Listings read at the same time in one round of the walk.
const READS_AT_ONCE: usize = 4;

/// The cache key of `id`'s listing.
pub(crate) fn listing_key(id: &str) -> String {
    format!("{}{id}", crate::cache::families::KITSU_FRANCHISE_RELATIONS)
}

/// `id`'s relation listing, from the cache or Kitsu.
///
/// # Errors
/// As the client's, on a miss.
pub async fn franchise_listing(state: &AppState, id: &str) -> Result<FranchiseListing> {
    let key = listing_key(id);
    if let Some(body) = meta_cache_get(&state.cache_pool, &key)? {
        if let Ok(listing) = serde_json::from_str(&body) {
            return Ok(listing);
        }
    }
    let listing = state.kitsu.franchise_relations(id).await?;
    if let Ok(body) = serde_json::to_string(&listing) {
        let _ = meta_cache_put(&state.cache_pool, &key, &body, ANIME_DETAIL_TTL.as_secs());
    }
    Ok(listing)
}

/// What the franchise surfaces show of an entry, from its detail.
#[must_use]
pub fn entry_from_detail(d: &KitsuAnimeRef) -> FranchiseEntry {
    let poster = d.poster_image.as_ref();
    let cover = d.cover_image.as_ref();
    FranchiseEntry {
        id: d.id.clone(),
        title: d.canonical_title.clone(),
        subtype: d.subtype.clone(),
        start: d.start_date.clone(),
        episodes: d.episode_count,
        status: d.status.clone(),
        poster: poster.and_then(|p| {
            p.medium
                .clone()
                .or_else(|| p.small.clone())
                .or_else(|| p.original.clone())
        }),
        cover: cover.and_then(|c| c.large.clone().or_else(|| c.original.clone())),
    }
}

/// The franchise around `anchor`.
///
/// # Errors
/// [`crate::error::AniError::InvalidKitsuId`] for a value that is not
/// an id, and the errors of the reads the walk asks for.
pub async fn franchise(state: &AppState, anchor: &str) -> Result<Franchise> {
    let anchor = crate::kitsu_id::require(anchor)?;
    let mut known = BTreeMap::new();
    let mut details = BTreeMap::new();
    loop {
        let (listings, wanted) = match plan(anchor, &known, &details) {
            Plan::Done(f) => return Ok(f),
            Plan::Need { listings, details } => (listings, details),
        };
        let read: Vec<(String, Result<FranchiseListing>)> = stream::iter(listings)
            .map(|id| async move {
                let listing = franchise_listing(state, &id).await;
                (id, listing)
            })
            .buffer_unordered(READS_AT_ONCE)
            .collect()
            .await;
        for (id, listing) in read {
            known.insert(id, listing?);
        }
        for id in wanted {
            let detail = crate::commands::kitsu::kitsu_anime_detail(state, &id).await?;
            details.insert(id, entry_from_detail(&detail));
        }
    }
}

#[cfg(test)]
#[path = "franchise_test.rs"]
mod tests;
