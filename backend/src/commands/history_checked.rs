//! The history read path where a failed cache read is an error rather
//! than a row read without its extra — split from [`super`] for the
//! per-file complexity bar.

use crate::commands::numbering_offset::Numbering;
use crate::error::Result;
use crate::history::{read_all, HistoryEntry};

/// The rows as [`super::history_list_through`] reads them, where a failed
/// cache read is an error rather than a row read without its extra — for
/// a caller that writes what it reads.
///
/// # Errors
/// The history file's read failure, and the cache's.
pub(crate) fn history_list_checked(
    state: &crate::app::AppState,
    numbering: &Numbering,
) -> Result<Vec<HistoryEntry>> {
    read_all(&state.history_path)?
        .into_iter()
        .map(|e| to_kitsu_numbering_checked(state, numbering, e, None))
        .collect()
}

/// [`super::to_kitsu_numbering`] where a failed cache read is an error rather
/// than no extra reading, for a caller that writes what it reads.
pub(super) fn to_kitsu_numbering_checked(
    state: &crate::app::AppState,
    numbering: &Numbering,
    mut entry: HistoryEntry,
    claimed_for: Option<&str>,
) -> Result<HistoryEntry> {
    // A row in the app's own numbering is already the episode; only a
    // provider slot needs the sidecar.
    if !entry.own_number {
        entry.ep_no = numbering.read_ep_no(&entry.id, &entry.ep_no);
    }
    let kitsu = match entry
        .kitsu_id
        .clone()
        .or_else(|| claimed_for.map(str::to_string))
    {
        Some(id) => Some(id),
        None => crate::commands::kitsu::allmanga_kitsu_get(state, &entry.id)?,
    };
    let reading = match crate::commands::kitsu_tail::KitsuTail::for_entry(state, kitsu.as_deref()) {
        Some(tail) => tail.kept()?,
        None => None,
    };
    if let Some(extra) = reading.and_then(|r| {
        crate::commands::play_native_appended::extra_for_slot(&r, &entry.id, &entry.ep_no)
    }) {
        entry.ep_no = extra;
    }
    Ok(entry)
}
