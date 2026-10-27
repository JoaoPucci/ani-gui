//! The episode map's cache row: its key, its window, its read and
//! write. Split from [`super`] so each file stays inside the CRAP gate's
//! per-file bar.

use std::time::Duration;

use crate::account::episode_map::EpisodeMap;
use crate::account::provider::ProviderKind;
use crate::app::AppState;
use crate::cache::ttl::{TRACKER_MAP_TTL, TRACKER_MAP_UNSETTLED_TTL};
use crate::cache::{meta_cache_get, meta_cache_put};

/// Cache key for one show's map on one tracker: the Kitsu id, and what
/// the map was built against — the entry Kitsu's mappings resolve the
/// show to with the total the write read for it, or that it resolves to
/// nothing. A remapped show or a recount of the resolved entry is a new
/// key; a show resolving to nothing is kept only for the short window,
/// since its key names no entry.
///
/// v2: the chain rule places a segment outside Kitsu's start and end
/// dates by Kitsu's own episode dates. A v1 row, written before it could,
/// says identity for chains it now maps.
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
    format!("tracker-map:v2:{}:k{kitsu_id}:{place}", kind.slug())
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
pub(super) fn cached(state: &AppState, key: &str) -> Option<EpisodeMap> {
    meta_cache_get(&state.cache_pool, key)
        .ok()
        .flatten()
        .and_then(|body| serde_json::from_str(&body).ok())
}

/// Write `map` under `key` for the window its evidence warrants.
/// Best-effort: a failed write costs the next write the same reads.
pub(super) fn remember(state: &AppState, key: &str, map: &EpisodeMap) {
    remember_for(state, key, map, map_ttl(map));
}

/// Write `map` under `key` for at most `ttl` — never longer than its
/// evidence warrants.
pub(super) fn remember_for(state: &AppState, key: &str, map: &EpisodeMap, ttl: Duration) {
    if let Ok(body) = serde_json::to_string(map) {
        let ttl = ttl.min(map_ttl(map));
        let _ = meta_cache_put(&state.cache_pool, key, &body, ttl.as_secs());
    }
}

#[cfg(test)]
#[path = "tracker_map_cache_prop_test.rs"]
mod prop_tests;
