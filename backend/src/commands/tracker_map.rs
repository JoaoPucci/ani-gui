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
//! per show per window ([`evidence`] has the walks).

use std::sync::Arc;
use std::time::Duration;

use crate::account::episode_map::{EpisodeMap, Evidence, Identity, KitsuShow, TrackerShow};
use crate::account::episode_map_rules::map_show;
use crate::account::provider::{ProviderKind, ProviderMediaId, Tokens, UserListProvider};
use crate::app::AppState;
use crate::cache::ttl::{TRACKER_MAP_TTL, TRACKER_MAP_UNSETTLED_TTL};
use crate::cache::{meta_cache_get, meta_cache_put};
use crate::error::Result;

#[path = "tracker_map_evidence.rs"]
mod evidence;
#[path = "tracker_map_kitsu.rs"]
mod kitsu_side;
#[path = "tracker_map_parts.rs"]
mod parts_walk;

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

/// The live row for `key`, when this build can read it.
fn cached(state: &AppState, key: &str) -> Option<EpisodeMap> {
    meta_cache_get(&state.cache_pool, key)
        .ok()
        .flatten()
        .and_then(|body| serde_json::from_str(&body).ok())
}

/// Write `map` under `key` for the window its evidence warrants.
/// Best-effort: a failed write costs the next write the same reads.
fn remember(state: &AppState, key: &str, map: &EpisodeMap) {
    if let Ok(body) = serde_json::to_string(map) {
        let _ = meta_cache_put(&state.cache_pool, key, &body, map_ttl(map).as_secs());
    }
}

/// The identity map a failed read leaves the write with, logged at
/// debug: the write proceeds as it did before maps existed.
fn unlearnt(kitsu_id: &str, err: &crate::error::AniError) -> EpisodeMap {
    tracing::debug!(kitsu_id, error = ?err, "tracker map: evidence not read; identity");
    EpisodeMap::identity(Identity::Unexplained)
}

/// Who is asking for a map, and through what: the app's state, the
/// tracker and its provider with the user's bearer, and the AniList
/// endpoint the public id translations go to (`None` in production).
pub(crate) struct MapAsk<'a> {
    /// The app's state: cache, Kitsu client.
    pub state: &'a Arc<AppState>,
    /// The tracker.
    pub kind: ProviderKind,
    /// Its provider.
    pub provider: &'a dyn UserListProvider,
    /// The user's bearer for it.
    pub tokens: &'a Tokens,
    /// AniList endpoint override for the public id translations.
    pub anilist_base: Option<&'a str>,
}

/// The map for Kitsu's `kitsu_id`, where Kitsu's mappings resolve the
/// show to `native`, whose total the write already read as
/// `tracker_total`.
pub(crate) async fn episode_map_for(
    ask: &MapAsk<'_>,
    kitsu_id: &str,
    native: ProviderMediaId,
    tracker_total: Option<u32>,
) -> EpisodeMap {
    let key = map_key(ask.kind, kitsu_id);
    if let Some(map) = cached(ask.state, &key) {
        return map;
    }
    let map = match build_resolved(ask, kitsu_id, native, tracker_total).await {
        Ok(map) => map,
        Err(err) => return unlearnt(kitsu_id, &err),
    };
    remember(ask.state, &key, &map);
    map
}

/// The map for Kitsu's `kitsu_id` when Kitsu's mappings resolve it to
/// nothing on the tracker: a later Kitsu part filed under the entry a
/// prequel resolves to, when Kitsu's prequel chain reaches one.
pub(crate) async fn episode_map_for_unresolved(ask: &MapAsk<'_>, kitsu_id: &str) -> EpisodeMap {
    let key = map_key(ask.kind, kitsu_id);
    if let Some(map) = cached(ask.state, &key) {
        return map;
    }
    let map = match build_unresolved(ask, kitsu_id).await {
        Ok(map) => map,
        Err(err) => return unlearnt(kitsu_id, &err),
    };
    remember(ask.state, &key, &map);
    map
}

/// Kitsu's entry as the map reads it, from the detail row the detail
/// page already cached (a cold read asks Kitsu once).
async fn kitsu_show(ask: &MapAsk<'_>, kitsu_id: &str) -> Result<KitsuShow> {
    let detail =
        super::kitsu::kitsu_anime_detail_with_anilist_base(ask.state, kitsu_id, ask.anilist_base)
            .await?;
    Ok(kitsu_side::kitsu_show_of(&detail))
}

/// Build the map for a show that resolves to `native`: agreeing or
/// unknown counts settle it without a tracker read; otherwise the
/// evidence the counts call for is walked.
async fn build_resolved(
    ask: &MapAsk<'_>,
    kitsu_id: &str,
    native: ProviderMediaId,
    tracker_total: Option<u32>,
) -> Result<EpisodeMap> {
    let kitsu = kitsu_show(ask, kitsu_id).await?;
    let settled = Evidence {
        kitsu: kitsu.clone(),
        entry: TrackerShow {
            id: native.0,
            episodes: tracker_total.filter(|n| *n > 0),
            ..Default::default()
        },
        ..Default::default()
    };
    let (Some(count), Some(total)) = (kitsu.episodes.filter(|n| *n > 0), settled.entry.episodes)
    else {
        return Ok(map_show(&settled));
    };
    if count == total {
        return Ok(map_show(&settled));
    }
    let gathered = evidence::gather(ask, kitsu, native).await?;
    Ok(map_show(&gathered))
}

/// Build the map for a show that resolves to nothing: through its
/// prequels to the entry it may be filed under, then the parts from
/// there. A show no prequel of which resolves is identity, kept for the
/// short window.
async fn build_unresolved(ask: &MapAsk<'_>, kitsu_id: &str) -> Result<EpisodeMap> {
    let ours = kitsu_show(ask, kitsu_id).await?;
    let Some((root, native)) = parts_walk::root_through_prequels(ask, kitsu_id).await? else {
        return Ok(EpisodeMap::identity(Identity::Unexplained));
    };
    let gathered = evidence::gather_under(ask, ours, root, native).await?;
    Ok(map_show(&gathered))
}

#[cfg(test)]
#[path = "tracker_map_test.rs"]
mod tests;
