//! History commands — `history_list`, `history_delete`, `history_clear`.
//!
//! Reads/writes the app's own history file. Its line format is
//! characterized from the script's and deliberately kept compatible;
//! the file itself is not shared with the script, which keeps its own
//! under a different path and has since it re-keyed onto provider
//! slugs.

use crate::error::Result;
use crate::history::{read_all, remove_by_id, write_atomic, HistoryEntry};

/// Translate a row's on-disk `ep_no` — the provider's numbering,
/// which the history file keys on — back to the per-entry (Kitsu) numbering
/// every GUI surface counts in, using the offset the resolver
/// stamped for the row's slug. Rows without a stamp pass through
/// unchanged: that is the no-shift case, and the pre-stamp behavior
/// for shows the GUI never resolved.
fn to_kitsu_numbering(state: &crate::app::AppState, mut entry: HistoryEntry) -> HistoryEntry {
    entry.ep_no = crate::commands::anidb_offset::read_ep_no(state, &entry.id, &entry.ep_no);
    entry
}

/// Returns every history entry as the frontend would render the
/// "Continue Watching" row. On-disk order is append-only with in-place
/// updates — the format's own rule, kept because the format is — so
/// entries come back in the order they appear on disk and the frontend
/// reverses if it wants newest-first. Episode numbers are
/// translated to the per-entry (Kitsu) numbering at this boundary.
///
/// # Errors
/// Returns [`crate::error::AniError::Io`] if the file exists but cannot
/// be read.
pub fn history_list(state: &crate::app::AppState) -> Result<Vec<HistoryEntry>> {
    Ok(read_all(&state.history_path)?
        .into_iter()
        .map(|e| to_kitsu_numbering(state, e))
        .collect())
}

/// Find the history entry (if any) whose show id maps to the supplied
/// `kitsu_id`. Walks the on-disk TSV, resolving each entry's `id` —
/// a provider slug on rows written since the migration — through the
/// `(show id → kitsu_id)` reverse cache a successful play stamps.
/// Of two rows that map to the entry, the one the user watched last
/// is returned — the latest watched-at stamp, a stamped row over an
/// unstamped one, then the further progress, then file order — the
/// rule the Continue Watching strip applies to the same rows, so
/// the two surfaces name one episode. Returns `None` when:
///   - The history file is missing or empty.
///   - No entry's show id has a cached mapping.
///   - None of the cached mappings equal `kitsu_id`.
///
/// The reverse cache is the same surface Continue Watching uses. A row
/// can be in the file without being in it: a play that had no Kitsu id
/// to record, a mapping the cross-cour guard refused, or a row older
/// than the mapping itself. Those rows are skipped here and show no
/// Resume affordance until a play stamps them — by design, since the
/// alternative is a Kitsu search per row.
///
/// # Errors
/// Returns [`crate::error::AniError::Io`] when the history file
/// exists but cannot be read; SQLite errors propagate from the
/// reverse-cache lookup and from the watched-at stamp's, since a
/// read that fails says nothing about the row it was for.
pub fn history_by_kitsu(
    state: &crate::app::AppState,
    kitsu_id: &str,
) -> Result<Option<HistoryEntry>> {
    if kitsu_id.is_empty() {
        return Ok(None);
    }
    let entries = read_all(&state.history_path)?;
    // Two providers can leave two rows for one show, one under each
    // provider's id. The one the user watched last is the one to
    // resume from, by the rule the Continue Watching strip applies to
    // the same rows ([`super::history_resume::resumes_over`]): the
    // latest watched-at stamp wins, a stamped row beats an unstamped
    // one, the further progress decides when the stamps do not, and
    // file order stands only when the rows are equal on every count.
    // A read the cache cannot serve is the caller's error, never a
    // row without a mapping or a stamp: taken as one, the row watched
    // last would lose to its sibling's older stamp, or be skipped for
    // it, and the page would resume the stale episode. The stamp is
    // the later of the row's own moment and the cache's
    // ([`super::history_resume::latest_of`]): a watch writes its
    // moment beside the row, so a cache that refused the stamp still
    // leaves the row ranked as the watch it was.
    let mut best: Option<(HistoryEntry, Option<i64>)> = None;
    for entry in entries {
        let Some(mapped) = crate::commands::kitsu::allmanga_kitsu_get(state, &entry.id)? else {
            continue;
        };
        if mapped != kitsu_id {
            continue;
        }
        let stamp = super::history_resume::latest_of(
            entry.watched_at,
            crate::commands::kitsu::watched_at_get(state, &entry.id)?,
        );
        let newer = match &best {
            None => true,
            Some((current, current_stamp)) => super::history_resume::resumes_over(
                (stamp, &entry.ep_no),
                (*current_stamp, &current.ep_no),
            ),
        };
        if newer {
            best = Some((entry, stamp));
        }
    }
    Ok(best.map(|(entry, _)| to_kitsu_numbering(state, entry)))
}

/// Every show's watched-at moment, for the Continue Watching strip
/// to sort and dedupe by: the cache's stamps, and for each history
/// row the later of its own moment and the cache's, so a row whose
/// stamp the cache refused still sorts as the watch it was.
///
/// # Errors
/// Returns [`crate::error::AniError::Io`] when the history file
/// exists but cannot be read; SQLite errors propagate from the
/// cache's listing.
pub fn watched_at_all(
    state: &crate::app::AppState,
) -> Result<std::collections::HashMap<String, i64>> {
    let mut stamps = crate::commands::kitsu::watched_at_all(state)?;
    for entry in read_all(&state.history_path)? {
        let cached = stamps.get(&entry.id).copied();
        if let Some(at) = super::history_resume::latest_of(entry.watched_at, cached) {
            stamps.insert(entry.id, at);
        }
    }
    Ok(stamps)
}

/// Remove the history row matching `id`. Returns `true` when a row
/// was removed, `false` for a no-op (id not in file, file missing,
/// empty id). The rewrite is atomic (`.new` + rename) so a concurrent
/// reader sees either the full pre-state or the full post-state,
/// never a half-written file.
///
/// # Errors
/// Returns [`crate::error::AniError::Io`] when the file exists and
/// cannot be read or written.
pub fn history_delete(state: &crate::app::AppState, id: &str) -> Result<bool> {
    if id.is_empty() {
        return Ok(false);
    }
    let mut entries = read_all(&state.history_path)?;
    if !remove_by_id(&mut entries, id) {
        return Ok(false);
    }
    write_atomic(&state.history_path, &entries)?;
    Ok(true)
}

/// Truncate the history file to zero length. Mirrors the script's `-D`.
///
/// # Errors
/// Returns [`crate::error::AniError::Io`] if the file cannot be written.
pub fn history_clear(state: &crate::app::AppState) -> Result<()> {
    write_atomic(&state.history_path, &[])
}

#[cfg(test)]
#[path = "history_selection_test.rs"]
mod selection_tests;

#[cfg(test)]
#[path = "history_test.rs"]
mod tests;
