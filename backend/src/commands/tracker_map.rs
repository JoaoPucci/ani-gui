//! The one way the write-back learns where a Kitsu episode lands on a
//! tracker: the show's [`EpisodeMap`], built from evidence the tracker
//! and Kitsu answer and kept in `meta_cache`.
//!
//! The map is keyed per tracker and Kitsu id,
//! `tracker-map:v1:<tracker>:k<kitsu id>`, and the key's version moves
//! with the map's shape so an older row is never read as the new one.
//! A map that rests on evidence that does not move — the counts
//! agreeing, or a rule explaining their difference — is kept for
//! [`TRACKER_MAP_TTL`]; one that rests on evidence that was missing or
//! inconclusive, an unknown count or a difference nothing explains, for
//! [`TRACKER_MAP_UNSETTLED_TTL`], since the count may be announced or
//! the tracker's relations filled in. A read that fails is kept nowhere:
//! it says nothing about the show, and the write it was for proceeds as
//! if nothing had been learnt.
//!
//! Nothing here runs on a write whose map is cached, and a cached
//! identity map costs the write one cache read. The evidence is
//! gathered — one tracker read for the entry, one per sequel hop, one
//! Kitsu read per mapping or relationship asked — only on a miss, once
//! per show per window.

use std::time::Duration;

use crate::account::episode_map::{EpisodeMap, Identity};
use crate::account::provider::{ProviderKind, ProviderMediaId, Tokens, UserListProvider};
use crate::app::AppState;
use crate::cache::ttl::{TRACKER_MAP_TTL, TRACKER_MAP_UNSETTLED_TTL};

/// Cache key for one show's map on one tracker.
#[must_use]
pub(crate) fn map_key(kind: ProviderKind, kitsu_id: &str) -> String {
    format!("tracker-map:v1:{}:k{kitsu_id}", kind.slug())
}

/// How long `map` is served from its row.
#[must_use]
pub(crate) fn map_ttl(map: &EpisodeMap) -> Duration {
    if map.conclusive() {
        TRACKER_MAP_TTL
    } else {
        TRACKER_MAP_UNSETTLED_TTL
    }
}

/// The map for Kitsu's `kitsu_id` on `kind`, where Kitsu's mappings
/// resolve the show to `native`, whose total the write already read as
/// `tracker_total`.
pub(crate) async fn episode_map_for(
    _state: &AppState,
    _kind: ProviderKind,
    _provider: &dyn UserListProvider,
    _tokens: &Tokens,
    _kitsu_id: &str,
    _native: ProviderMediaId,
    _tracker_total: Option<u32>,
    _anilist_base: Option<&str>,
) -> EpisodeMap {
    EpisodeMap::identity(Identity::Unexplained)
}

/// The map for Kitsu's `kitsu_id` on `kind` when Kitsu's mappings
/// resolve it to nothing there: a later Kitsu part filed under the
/// entry a prequel resolves to, when Kitsu's prequel chain reaches one.
pub(crate) async fn episode_map_for_unresolved(
    _state: &AppState,
    _kind: ProviderKind,
    _provider: &dyn UserListProvider,
    _tokens: &Tokens,
    _kitsu_id: &str,
    _anilist_base: Option<&str>,
) -> EpisodeMap {
    EpisodeMap::identity(Identity::Unexplained)
}

#[cfg(test)]
#[path = "tracker_map_test.rs"]
mod tests;
