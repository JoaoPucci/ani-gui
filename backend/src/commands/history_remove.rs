//! Removing history: one row, or all of it, with what the rows left
//! beside them — split from `history` for the per-file complexity bar.
//!
//! A removal runs with the history held ([`crate::history::guard`]),
//! so no other writer is half-way through a write of the file or of
//! what its rows leave behind, and it records what it removed, so work
//! begun before it — a watch still waiting on Kitsu, a play still
//! resolving, a skip-time lookup still out — writes nothing of the
//! removed show afterwards: nothing under its row's key, and nothing
//! under the Kitsu ids the history or this process knew it by.

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
        // The Kitsu ids the remaining rows claim, and the ones this row
        // was known by: what the rows and the cache say, and the pages
        // this process saw each played from — a row records its page
        // only once its watch's verdict is in.
        let mut claimed = super::history_forget_skips::claimed_ids(state, &entries)?;
        for entry in &entries {
            claimed.extend(held.pages_of(&entry.id));
        }
        let pages = held.pages_of(id);
        let (mut known_by, mut other_keys) = (Vec::new(), Vec::new());
        for (title, recorded) in &removed {
            let forgotten = super::history_forget::forget_show(
                state,
                id,
                title,
                recorded.as_deref(),
                &pages,
                &claimed,
            )?;
            known_by.extend(forgotten.known_by);
            other_keys.extend(forgotten.other_keys);
        }
        // The numbering of the keys a page of the show resolved under
        // goes with the resolution rows that named them, before the
        // history write: a write that fails would otherwise leave the
        // retry nothing to find them by. A key a remaining row has
        // keeps the numbering that row is read through.
        other_keys.retain(|key| !entries.iter().any(|e| e.id == *key));
        let rowless: Vec<&str> = other_keys.iter().map(String::as_str).collect();
        super::history_forget::sweep_offsets(state, &rowless);
        held.write(&entries)?;
        super::history_forget::sweep_offsets(state, &[id]);
        held.removed_show(id, &known_by);
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
        let ids: Vec<&str> = cleared.iter().map(|e| e.id.as_str()).collect();
        // What a delete of each show would take under another key: the
        // numbering of the keys its pages resolved under. The rows
        // themselves go with every other resolution row below.
        let mut known_by = super::history_forget_skips::claimed_ids(state, &cleared)?;
        for entry in &cleared {
            known_by.extend(held.pages_of(&entry.id));
        }
        let known_by: Vec<String> = known_by.into_iter().filter(|k| !k.is_empty()).collect();
        let other_keys =
            super::history_forget_resolutions::forget_resolutions(state, &ids, &known_by)?;
        super::history_forget::forget_all(state)?;
        let rowless: Vec<&str> = other_keys.iter().map(String::as_str).collect();
        super::history_forget::sweep_offsets(state, &rowless);
        held.write(&[])?;
        super::history_forget::sweep_offsets(state, &ids);
        held.removed_all();
        Ok(())
    })
}
