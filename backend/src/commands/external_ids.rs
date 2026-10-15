//! The one way a command reads a show's external ids — the MAL and
//! AniList ids Kitsu's mappings carry for a Kitsu anime.
//!
//! Tracker id-resolution, the airing schedule, the banner and
//! episode-thumbnail backfills and aniskip all start from the same
//! `/anime/:id?include=mappings` answer, and a detail page with two
//! trackers connected asks for it twice at once. The answer is kept
//! in `meta_cache` under `kitsu:external-ids:v1:k<kitsu_id>`: thirty
//! days when both ids are present, a day when one is missing (see
//! [`external_ids_ttl`]). A failed read is kept nowhere — it says
//! nothing about the mapping. Reads of one show in flight together
//! share one request: the second waits for the first and takes its row
//! ([`in_flight`]), so a cold open costs one Kitsu request however
//! many surfaces ask at once.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use crate::app::AppState;
use crate::cache::ttl::{EXTERNAL_IDS_PARTIAL_TTL, EXTERNAL_IDS_TTL};
use crate::cache::{meta_cache_get, meta_cache_put, SqlitePool};
use crate::error::Result;
use crate::meta::kitsu::ExternalIds;

/// One async mutex per Kitsu id being read right now. Process-wide,
/// like the account write locks, because the reads that race arrive as
/// separate HTTP requests; a static rather than a field on `AppState`,
/// which every test fixture constructs by hand. One entry per show
/// read this run — negligible for a desktop session.
type InFlight = Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>;

fn in_flight(kitsu_id: &str) -> Arc<tokio::sync::Mutex<()>> {
    static IN_FLIGHT: OnceLock<InFlight> = OnceLock::new();
    let mut map = IN_FLIGHT
        .get_or_init(InFlight::default)
        .lock()
        .expect("external-ids in-flight map poisoned");
    Arc::clone(map.entry(kitsu_id.to_owned()).or_default())
}

/// Cache key for one show's external ids. Versioned with the shape of
/// [`ExternalIds`]: a field added there moves the version so old rows
/// are not read as the new shape.
fn external_ids_key(kitsu_id: &str) -> String {
    format!("kitsu:external-ids:v1:k{kitsu_id}")
}

/// How long an answer from Kitsu's mappings is trusted. What is
/// present does not move, so a complete mapping — both trackers' ids
/// — is kept for [`EXTERNAL_IDS_TTL`]. What is missing may appear:
/// Kitsu adds the MAL mapping for a fresh seasonal show within days,
/// so an answer missing either id is re-asked after
/// [`EXTERNAL_IDS_PARTIAL_TTL`].
#[must_use]
pub(crate) fn external_ids_ttl(ids: &ExternalIds) -> Duration {
    if ids.mal.is_some() && ids.anilist.is_some() {
        EXTERNAL_IDS_TTL
    } else {
        EXTERNAL_IDS_PARTIAL_TTL
    }
}

/// The live row under `key`, when there is one this build can read. A
/// cache read that fails, or a row that does not parse, is not an
/// answer about the mapping: the caller asks Kitsu and overwrites it.
fn cached(pool: &SqlitePool, key: &str) -> Option<ExternalIds> {
    meta_cache_get(pool, key)
        .ok()
        .flatten()
        .and_then(|body| serde_json::from_str::<ExternalIds>(&body).ok())
}

/// Write what Kitsu answered. Best-effort: a failed write costs one
/// more Kitsu request.
fn remember(pool: &SqlitePool, key: &str, ids: &ExternalIds) {
    if let Ok(body) = serde_json::to_string(ids) {
        let _ = meta_cache_put(pool, key, &body, external_ids_ttl(ids).as_secs());
    }
}

/// The MAL and AniList ids Kitsu maps `kitsu_id` to, from the cache
/// when a row is live and from Kitsu otherwise — once, however many
/// readers miss at the same moment.
///
/// # Errors
/// Whatever [`crate::meta::kitsu::KitsuClient::external_ids_for_kitsu_id`]
/// answers with: Network / Upstream / ParseFailed.
pub(crate) async fn external_ids_for(state: &AppState, kitsu_id: &str) -> Result<ExternalIds> {
    let key = external_ids_key(kitsu_id);
    if let Some(ids) = cached(&state.cache_pool, &key) {
        return Ok(ids);
    }
    let lock = in_flight(kitsu_id);
    let _reading = lock.lock().await;
    // A read that finished while this one waited wrote the row.
    if let Some(ids) = cached(&state.cache_pool, &key) {
        return Ok(ids);
    }
    let ids = state.kitsu.external_ids_for_kitsu_id(kitsu_id).await?;
    remember(&state.cache_pool, &key, &ids);
    Ok(ids)
}

#[cfg(test)]
#[path = "external_ids_test.rs"]
mod tests;

#[cfg(test)]
#[path = "external_ids_prop_test.rs"]
mod prop_tests;
