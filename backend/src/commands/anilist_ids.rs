//! The one way a command translates ids between AniList and MAL.
//!
//! AniList's `Media` carries both its own id and MAL's, so it is the
//! bridge whenever Kitsu maps a show to one tracker only: a MAL id
//! becomes AniList's `mediaId` for the AniList account, an AniList id
//! becomes the `idMal` the MAL account keys on, and the Watch Later
//! rail asks for many MAL ids at once. Every one of these is a public
//! AniList request against the 30-per-minute budget the rest of the
//! app shares, so each answer is kept in `meta_cache`, one row per
//! id in each direction, and one for every media recording a MAL id:
//!
//! - `anilist:media-id-for-mal:v1:<mal_id>` — AniList's `mediaId`
//! - `anilist:mal-id-for-media:v1:<media_id>` — the `idMal` it records
//! - `anilist:media-ids-recording-mal:v1:<mal_id>` — every media that
//!   records the MAL id, which the episode map reads for ownership
//!
//! A translation row's body is the translated id, or empty for "AniList
//! does not index this" — an id is never empty, so the two cannot be
//! confused; the listing row's is the ids joined by commas, empty for
//! none. A positive is kept for [`ANILIST_ID_MAP_TTL`], a negative for
//! [`ANILIST_ID_MAP_MISS_TTL`]; a failed request is kept nowhere. The
//! batch writes every id it learns, so the detail page's single
//! translation for a show the rail already bridged costs nothing; a MAL
//! id several media record it leaves to the single translation, whose
//! answer is the one that counts.

use std::collections::HashMap;

use crate::app::AppState;
use crate::cache::ttl::{ANILIST_ID_MAP_MISS_TTL, ANILIST_ID_MAP_TTL};
use crate::cache::{meta_cache_get, meta_cache_put, SqlitePool};
use crate::error::Result;

fn media_id_key(mal_id: u32) -> String {
    format!("anilist:media-id-for-mal:v1:{mal_id}")
}

fn recording_key(mal_id: u32) -> String {
    format!("anilist:media-ids-recording-mal:v1:{mal_id}")
}

fn mal_id_key(media_id: u32) -> String {
    format!("anilist:mal-id-for-media:v1:{media_id}")
}

/// The cached answer under `key`: `Some(Some(id))` translated,
/// `Some(None)` known not indexed, `None` never asked — or a row this
/// build cannot read, which is re-asked like one never written.
fn cached_id(pool: &SqlitePool, key: &str) -> Option<Option<u32>> {
    match meta_cache_get(pool, key) {
        Ok(Some(body)) if body.is_empty() => Some(None),
        Ok(Some(body)) => body.parse::<u32>().ok().map(Some),
        _ => None,
    }
}

/// Record an answer AniList gave. Best-effort: a failed write costs
/// one more AniList request.
fn remember_id(pool: &SqlitePool, key: &str, id: Option<u32>) {
    let (body, ttl) = match id {
        Some(id) => (id.to_string(), ANILIST_ID_MAP_TTL),
        None => (String::new(), ANILIST_ID_MAP_MISS_TTL),
    };
    let _ = meta_cache_put(pool, key, &body, ttl.as_secs());
}

/// AniList's `mediaId` for a MAL id; `None` when AniList does not
/// index it.
///
/// # Errors
/// Same as [`crate::meta::anilist::media_id_for_mal`].
pub(crate) async fn media_id_for_mal(
    state: &AppState,
    mal_id: u32,
    anilist_base: Option<&str>,
) -> Result<Option<u32>> {
    let key = media_id_key(mal_id);
    if let Some(known) = cached_id(&state.cache_pool, &key) {
        return Ok(known);
    }
    let id = crate::meta::anilist::media_id_for_mal(&state.meta_http, mal_id, anilist_base).await?;
    remember_id(&state.cache_pool, &key, id);
    Ok(id)
}

/// The MAL id AniList records for one of its media; `None` when it
/// records none.
///
/// # Errors
/// Same as [`crate::meta::anilist::mal_id_for_media_id`].
pub(crate) async fn mal_id_for_media_id(
    state: &AppState,
    media_id: u32,
    anilist_base: Option<&str>,
) -> Result<Option<u32>> {
    let key = mal_id_key(media_id);
    if let Some(known) = cached_id(&state.cache_pool, &key) {
        return Ok(known);
    }
    let id =
        crate::meta::anilist::mal_id_for_media_id(&state.meta_http, media_id, anilist_base).await?;
    remember_id(&state.cache_pool, &key, id);
    Ok(id)
}

/// AniList's `mediaId` for each of many MAL ids, as the single
/// translation ([`media_id_for_mal`]) answers it; ids AniList does not
/// index are absent from the map. A MAL id several media record is
/// translated singly, since the batch lists them all and only the
/// single query says which one the id translates to; one whose single
/// translation fails is left out, and kept nowhere. Only the ids with no live row reach
/// AniList, and every id the request answers for — by a media, or by
/// its absence from a page AniList did return — is remembered.
///
/// # Errors
/// Same as [`crate::meta::anilist::media_listed_for_mals`]. Nothing is
/// remembered from a failed request.
pub(crate) async fn media_ids_for_mals(
    state: &AppState,
    mal_ids: &[u32],
    anilist_base: Option<&str>,
) -> Result<HashMap<u32, u32>> {
    let mut map = HashMap::new();
    let mut unknown = Vec::new();
    for &mal_id in mal_ids {
        match cached_id(&state.cache_pool, &media_id_key(mal_id)) {
            Some(Some(media_id)) => {
                map.insert(mal_id, media_id);
            }
            Some(None) => {}
            None => unknown.push(mal_id),
        }
    }
    if unknown.is_empty() {
        return Ok(map);
    }
    let listed =
        crate::meta::anilist::media_listed_for_mals(&state.meta_http, &unknown, anilist_base)
            .await?;
    for mal_id in unknown {
        let media_id = match listed.get(&mal_id).map(Vec::as_slice) {
            None | Some([]) => None,
            Some([one]) => Some(*one),
            // Several media record the id: which one it translates to
            // is the single translation's answer, never the batch's
            // order. A single translation that fails leaves the id out,
            // and kept nowhere, for the next load to ask again.
            Some(_) => match media_id_for_mal(state, mal_id, anilist_base).await {
                Ok(media_id) => media_id,
                Err(err) => {
                    tracing::debug!(mal_id, error = ?err, "anilist: single translation failed");
                    continue;
                }
            },
        };
        if listed.get(&mal_id).is_none_or(|ids| ids.len() < 2) {
            remember_id(&state.cache_pool, &media_id_key(mal_id), media_id);
        }
        if let Some(media_id) = media_id {
            map.insert(mal_id, media_id);
        }
    }
    Ok(map)
}

/// Every AniList media that records MAL's `mal_id`, in AniList id
/// order; empty when none does. Kept under its own row, its ids joined
/// by commas (empty for none), for as long as the single translations.
///
/// # Errors
/// Same as [`crate::meta::anilist::media_ids_recording_mal`]. Nothing is
/// remembered from a failed request.
pub(crate) async fn media_ids_recording_mal(
    state: &AppState,
    mal_id: u32,
    anilist_base: Option<&str>,
) -> Result<Vec<u32>> {
    let key = recording_key(mal_id);
    if let Ok(Some(body)) = meta_cache_get(&state.cache_pool, &key) {
        let ids: Option<Vec<u32>> = body
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|s| s.parse().ok())
            .collect();
        if let Some(ids) = ids {
            return Ok(ids);
        }
    }
    let ids = crate::meta::anilist::media_ids_recording_mal(&state.meta_http, mal_id, anilist_base)
        .await?;
    let body: Vec<String> = ids.iter().map(u32::to_string).collect();
    let ttl = if ids.is_empty() {
        ANILIST_ID_MAP_MISS_TTL
    } else {
        ANILIST_ID_MAP_TTL
    };
    let _ = meta_cache_put(&state.cache_pool, &key, &body.join(","), ttl.as_secs());
    Ok(ids)
}

#[cfg(test)]
#[path = "anilist_ids_test.rs"]
mod tests;
