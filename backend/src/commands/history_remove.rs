//! Removing history: one row, or all of it, with what the rows left
//! beside them — split from `history` for the per-file complexity bar.
//!
//! A removal runs with the history held ([`crate::history::guard`]),
//! so no other writer is half-way through a write of the file or of
//! what its rows leave behind, and it records what it removed, so work
//! begun before it — a watch still waiting on Kitsu, a play still
//! resolving, a skip-time lookup still out — writes nothing of the
//! removed show afterwards.

use crate::error::Result;
use crate::history::guard::hold;
use crate::history::remove_by_id;

/// Remove the history row matching `id`. Returns `true` when a row
/// was removed, `false` for a no-op (id not in file, file missing,
/// empty id). The rewrite is atomic (`.new` + rename) so a concurrent
/// reader sees either the full pre-state or the full post-state,
/// never a half-written file. What the row left in the cache goes with
/// it ([`super::history_forget::forget_show`]).
///
/// # Errors
/// Returns [`crate::error::AniError::Io`] when the file exists and
/// cannot be read or written, and [`crate::error::AniError::Cache`]
/// when the cache cannot be.
pub fn history_delete(state: &crate::app::AppState, id: &str) -> Result<bool> {
    if id.is_empty() {
        return Ok(false);
    }
    hold(&state.history_path, |held| {
        let mut entries = held.rows()?;
        let removed: Vec<(String, Option<String>)> = entries
            .iter()
            .filter(|e| e.id == id)
            .map(|e| (e.title.clone(), e.kitsu_id.clone()))
            .collect();
        if !remove_by_id(&mut entries, id) {
            return Ok(false);
        }
        // The cache first: a cache that cannot forget fails the delete
        // with the row still there to retry, rather than reporting a
        // failure for a row already gone. The offsets last, once the
        // row is gone, so no failure leaves a row without its offset.
        let claimed = super::history_forget_skips::claimed_ids(state, &entries)?;
        let mut skip_ids = Vec::new();
        for (title, recorded) in &removed {
            skip_ids.extend(super::history_forget::forget_show(
                state,
                id,
                title,
                recorded.as_deref(),
                &claimed,
            )?);
        }
        held.write(&entries)?;
        super::history_forget::sweep_offsets(state, &[id]);
        held.removed_show(id, &skip_ids);
        Ok(true)
    })
}

/// Truncate the history file to zero length. Mirrors the script's `-D`.
/// What the rows left in the cache goes with them
/// ([`super::history_forget::forget_all`]).
///
/// # Errors
/// Returns [`crate::error::AniError::Io`] if the file exists but cannot
/// be read, or cannot be written, and [`crate::error::AniError::Cache`]
/// if the cache cannot be.
pub fn history_clear(state: &crate::app::AppState) -> Result<()> {
    hold(&state.history_path, |held| {
        // In history_delete's order, for its reasons.
        // A history that exists but cannot be read fails the clear: the
        // rows it holds name the offsets that go with them.
        let cleared = held.rows()?;
        super::history_forget::forget_all(state)?;
        held.write(&[])?;
        let ids: Vec<&str> = cleared.iter().map(|e| e.id.as_str()).collect();
        super::history_forget::sweep_offsets(state, &ids);
        held.removed_all();
        Ok(())
    })
}
