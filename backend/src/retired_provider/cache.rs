//! The cache half of the retired-provider sweep — split from
//! [`super`] for the per-file complexity bar.
//!
//! Each family is read as a range of the key index, with the one JSON
//! field a guard reads where one decides, never the whole body, so a
//! cache with nothing to reclaim costs a few indexed range reads.

use std::collections::HashSet;

use rusqlite::params;

use crate::app::AppState;
use crate::error::{AniError, Result};
use crate::history::HistoryEntry;

use super::cache_families as family;

/// The rows only a retired provider's key or verdict could be read by.
/// `remaining` is the history: a retired key a row still carries keeps
/// its stamp, mapping and title matches, which order and resolve that
/// row; a row the sweep removes takes its own with it.
pub(super) fn doomed(state: &AppState, remaining: &[HistoryEntry]) -> Result<Vec<String>> {
    let ids: HashSet<&str> = remaining.iter().map(|e| e.id.as_str()).collect();
    let mut doomed = family::stamps(state, &ids)?;
    doomed.extend(family::mappings(state, &ids)?);
    doomed.extend(family::resolutions(state)?);
    doomed.extend(family::verdicts(state)?);
    doomed.extend(super::cache_titles::title_matches(state, remaining)?);
    Ok(doomed)
}

/// Delete `doomed` in one write, and answer how many rows went.
pub(super) fn delete(state: &AppState, doomed: &[String]) -> Result<usize> {
    if doomed.is_empty() {
        return Ok(0);
    }
    let mut conn = state.cache_pool.get().map_err(|_| AniError::Cache)?;
    let tx = conn.transaction().map_err(|_| AniError::Cache)?;
    let mut deleted = 0;
    for key in doomed {
        deleted += tx
            .execute("DELETE FROM meta_cache WHERE key = ?1", params![key])
            .map_err(|_| AniError::Cache)?;
    }
    tx.commit().map_err(|_| AniError::Cache)?;
    Ok(deleted)
}
