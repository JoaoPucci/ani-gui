//! The one way the write-back learns where a Kitsu episode lands on a
//! tracker: the show's [`EpisodeMap`], built from evidence the tracker
//! and Kitsu answer and kept in `meta_cache`.
//!
//! The map is keyed per tracker, Kitsu id and what it was built against
//! — the resolved entry and its total, or no entry at all
//! ([`map_key`]) — and the key's version moves with the map's shape so
//! an older row is never read as the new one. A map that rests on
//! evidence that does not move — the counts agreeing, or a rule
//! explaining their difference over entries that have all finished
//! airing — is kept for [`TRACKER_MAP_TTL`]; one that rests on evidence
//! that was missing, inconclusive or still moving — an unknown count, a
//! difference nothing explains, an entry still airing — for
//! [`TRACKER_MAP_UNSETTLED_TTL`]. A read that fails is kept nowhere:
//! it says nothing about the show, and the write it was for proceeds as
//! if nothing had been learnt.
//!
//! Nothing here runs on a write whose map is cached, and a cached
//! identity map costs the write one cache read. The evidence is
//! gathered — one tracker read for the entry and one per sequel it names
//! at each hop, one Kitsu read per mapping or relationship asked, and the
//! id translations a part's resolution makes — only on a miss, once per
//! show per window ([`evidence`] has the walks).

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

/// Cache key for one show's map on one tracker: the Kitsu id, and what
/// the map was built against — the entry Kitsu's mappings resolve the
/// show to with the total the write read for it, or that it resolves to
/// nothing. A remapped show or a recounted entry is a new key, so a map
/// never answers for evidence it was not built from.
#[must_use]
pub(crate) fn map_key(
    kind: ProviderKind,
    kitsu_id: &str,
    resolved: Option<(u32, Option<u32>)>,
) -> String {
    let place = match resolved {
        Some((entry, total)) => format!(
            "e{entry}:t{}",
            total.map(|t| t.to_string()).unwrap_or_default()
        ),
        None => "unresolved".to_string(),
    };
    format!("tracker-map:v1:{}:k{kitsu_id}:{place}", kind.slug())
}

/// How long `map` is served from its row: the long window only for a
/// conclusive map whose every entry has finished airing, since an
/// airing entry's status — and with it each segment's completion —
/// moves.
#[must_use]
pub(crate) fn map_ttl(map: &EpisodeMap) -> Duration {
    if map.conclusive() && map.segments.iter().all(|s| s.entry_finished) {
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
    remember_for(state, key, map, map_ttl(map));
}

/// Write `map` under `key` for at most `ttl` — never longer than its
/// evidence warrants.
fn remember_for(state: &AppState, key: &str, map: &EpisodeMap, ttl: Duration) {
    if let Ok(body) = serde_json::to_string(map) {
        let ttl = ttl.min(map_ttl(map));
        let _ = meta_cache_put(&state.cache_pool, key, &body, ttl.as_secs());
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
    let key = map_key(ask.kind, kitsu_id, Some((native.0, tracker_total)));
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
    let key = map_key(ask.kind, kitsu_id, None);
    if let Some(map) = cached(ask.state, &key) {
        return map;
    }
    let map = match build_unresolved(ask, kitsu_id).await {
        Ok(map) => map,
        Err(err) => return unlearnt(kitsu_id, &err),
    };
    // The key names no entry for such a show, so a recount of the entry
    // it is filed under reaches no new key: it is asked for daily.
    remember_for(ask.state, &key, &map, TRACKER_MAP_UNSETTLED_TTL);
    map
}

/// Kitsu's entry as the map reads it, from the detail row the detail
/// page caches; a cold row is read as any detail read is — from Kitsu,
/// and AniList's banner for a show Kitsu has no cover for.
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
