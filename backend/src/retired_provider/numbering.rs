//! The numbering half of the retired-provider sweep — split from
//! [`super`] for the per-file complexity bar.

use std::collections::HashSet;

use crate::app::AppState;
use crate::commands::numbering_offset::Numbering;
use crate::history::HistoryEntry;

use super::walks;

/// The numbering store's slugs that name no walked provider.
pub(super) fn retired(state: &AppState) -> Vec<String> {
    Numbering::read(&state.history_path)
        .slugs()
        .filter(|s| !walks(state, s))
        .map(str::to_string)
        .collect()
}

/// Drop the numbering of retired keys no remaining row reads through.
/// A row in the app's own numbering reads through none, but a former
/// stamp is what a position kept under the row's earlier number is
/// found by, so a remaining row's former stamp keeps its numbering.
pub(super) fn sweep(state: &AppState, remaining: &[HistoryEntry]) -> std::io::Result<()> {
    let store = Numbering::read(&state.history_path);
    let needed: HashSet<&str> = remaining
        .iter()
        .filter(|r| !r.own_number || store.former(&r.id).is_some())
        .map(|r| r.id.as_str())
        .collect();
    if !retired(state).iter().any(|s| !needed.contains(s.as_str())) {
        return Ok(());
    }
    crate::commands::numbering_offset::forget_where(state, |slug| {
        !walks(state, slug) && !needed.contains(slug)
    })
}
