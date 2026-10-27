//! Which rows of each cache family the retired-provider sweep removes
//! — split from [`super::cache`] for the per-file complexity bar.

use std::collections::HashSet;

use crate::app::AppState;
use crate::error::Result;

use super::cache_keys::{mapping_key_id, rows, walked_provider};
use super::walks;

const STAMPS: &str = "watched-at:v1:";

/// Watched-at stamps of a retired key no history row carries.
pub(super) fn stamps(state: &AppState, ids: &HashSet<&str>) -> Result<Vec<String>> {
    Ok(rows(state, STAMPS, None)?
        .into_iter()
        .map(|(key, _)| key)
        .filter(|key| retired_rowless(state, ids, &key[STAMPS.len()..]))
        .collect())
}

/// Reverse mappings, every version and the played mark, of a retired
/// key no history row carries.
pub(super) fn mappings(state: &AppState, ids: &HashSet<&str>) -> Result<Vec<String>> {
    Ok(rows(state, "allmanga2kitsu:", None)?
        .into_iter()
        .map(|(key, _)| key)
        .filter(|key| mapping_key_id(key).is_some_and(|id| retired_rowless(state, ids, id)))
        .collect())
}

/// Resolution rows whose show key names no walked provider, an empty
/// or absent one included: they are never replayed.
pub(super) fn resolutions(state: &AppState) -> Result<Vec<String>> {
    Ok(rows(state, "play:", Some("$.show_id"))?
        .into_iter()
        .filter(|(_, show)| !show.as_deref().is_some_and(|id| walks(state, id)))
        .map(|(key, _)| key)
        .collect())
}

/// Availability verdicts naming no walked provider, or none: they are
/// never served.
pub(super) fn verdicts(state: &AppState) -> Result<Vec<String>> {
    Ok(rows(state, "availability:", Some("$.provider"))?
        .into_iter()
        .filter(|(_, p)| !p.as_deref().is_some_and(|p| walked_provider(state, p)))
        .map(|(key, _)| key)
        .collect())
}

/// Whether `id` is a retired key no history row carries.
fn retired_rowless(state: &AppState, ids: &HashSet<&str>, id: &str) -> bool {
    !walks(state, id) && !ids.contains(id)
}
