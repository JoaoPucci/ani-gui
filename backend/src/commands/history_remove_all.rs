//! Clearing the whole history, with everything its rows left behind;
//! split from [`super`] so each file stays inside the CRAP gate's
//! per-file bar.

use super::*;

/// Truncate the history file to zero length. Mirrors the script's `-D`.
/// What the rows left in the cache goes with them
/// ([`crate::commands::history_forget::forget_all`]).
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
        // numbering of the keys its pages resolved under, whatever else
        // names them, since every resolution row goes below.
        let mut known_by = crate::commands::history_forget_skips::claimed_ids(state, &cleared)?;
        for entry in &cleared {
            known_by.extend(held.pages_of(&entry.id));
        }
        let known_by: Vec<String> = known_by.into_iter().filter(|k| !k.is_empty()).collect();
        let found =
            crate::commands::history_forget_resolutions::find_resolutions(state, &ids, &known_by)?;
        let rowless: Vec<&str> = found.named.iter().map(String::as_str).collect();
        crate::commands::history_forget::sweep_offsets(state, &rowless);
        crate::commands::history_forget::forget_all(state)?;
        held.write(&[])?;
        crate::commands::history_forget::sweep_offsets(state, &ids);
        held.removed_all();
        Ok(())
    })
}
