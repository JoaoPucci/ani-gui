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
//! it says nothing about the show, so the write it was for fails with
//! that read's error (a rate limit keeps its wait) rather than going
//! ahead as identity.
//!
//! Nothing here runs on a write whose map is cached, and a cached
//! identity map costs the write one cache read. The evidence is
//! gathered — one tracker read for the entry and one per sequel it names
//! at each hop, one Kitsu read per mapping or relationship asked, and the
//! id translations a part's resolution makes — only on a miss, once per
//! show per window ([`evidence`] has the walks).

use std::sync::Arc;

use crate::account::episode_map::{EpisodeMap, Evidence, Identity, KitsuShow, TrackerShow};
use crate::account::episode_map_rules::map_show;
use crate::account::provider::{ProviderKind, ProviderMediaId, Tokens, UserListProvider};
use crate::app::AppState;
#[cfg(test)]
use crate::cache::ttl::TRACKER_MAP_TTL;
use crate::cache::ttl::TRACKER_MAP_UNSETTLED_TTL;
use crate::error::Result;

#[path = "tracker_map_cache.rs"]
mod cache;
#[path = "tracker_map_evidence.rs"]
mod evidence;

pub(crate) use cache::map_key;
#[cfg(test)]
use cache::map_ttl;
use cache::{cached, remember, remember_for};
#[path = "tracker_map_kitsu.rs"]
mod kitsu_side;
#[path = "tracker_map_parts.rs"]
mod parts_walk;
#[path = "tracker_map_prequels.rs"]
mod prequel_walk;

/// A read the map rests on failed, logged at debug. The map is neither
/// built nor kept, and the write it was for fails with the read's error:
/// an answer that never arrived is unknown, not the absence of a
/// mapping, so the write does not go ahead as identity.
fn unread(kitsu_id: &str, err: &crate::error::AniError) {
    tracing::debug!(kitsu_id, error = ?err, "tracker map: evidence not read; write fails");
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
) -> Result<EpisodeMap> {
    let key = map_key(ask.kind, kitsu_id, Some((native.0, tracker_total)));
    if let Some(map) = cached(ask.state, &key) {
        return Ok(map);
    }
    let map = build_resolved(ask, kitsu_id, native, tracker_total)
        .await
        .inspect_err(|err| unread(kitsu_id, err))?;
    remember(ask.state, &key, &map);
    Ok(map)
}

/// The map for Kitsu's `kitsu_id` when Kitsu's mappings resolve it to
/// nothing on the tracker: a later Kitsu part filed under the entry a
/// prequel resolves to, when Kitsu's prequel chain reaches one.
pub(crate) async fn episode_map_for_unresolved(
    ask: &MapAsk<'_>,
    kitsu_id: &str,
) -> Result<EpisodeMap> {
    let key = map_key(ask.kind, kitsu_id, None);
    if let Some(map) = cached(ask.state, &key) {
        return Ok(map);
    }
    let map = build_unresolved(ask, kitsu_id)
        .await
        .inspect_err(|err| unread(kitsu_id, err))?;
    // The key names no entry for such a show, so a recount of the entry
    // it is filed under reaches no new key: it is asked for daily.
    remember_for(ask.state, &key, &map, TRACKER_MAP_UNSETTLED_TTL);
    Ok(map)
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
/// prequels to the entry it may be filed under, back to the first part
/// filed under that entry, then the parts from there. A show no
/// prequel of which resolves, or whose entry the tracker answers is
/// gone, is identity, kept for the short window.
async fn build_unresolved(ask: &MapAsk<'_>, kitsu_id: &str) -> Result<EpisodeMap> {
    let ours = kitsu_show(ask, kitsu_id).await?;
    let Some((root, native)) = prequel_walk::root_through_prequels(ask, kitsu_id).await? else {
        return Ok(EpisodeMap::identity(Identity::Unexplained));
    };
    // The nearest prequel that resolves may be a later part under the
    // entry: the parts start at the first of them.
    let root = prequel_walk::first_part_under(ask, &root, native).await?;
    let Some(gathered) = evidence::gather_under(ask, ours, root, native).await? else {
        return Ok(EpisodeMap::identity(Identity::Unexplained));
    };
    Ok(map_show(&gathered))
}

#[cfg(test)]
#[path = "tracker_map_test.rs"]
mod tests;
