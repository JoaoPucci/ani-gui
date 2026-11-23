//! The numbering half of the retired-provider sweep — split from
//! [`super`] for the per-file complexity bar.

use std::collections::HashSet;

use crate::app::AppState;
use crate::commands::numbering_offset::Numbering;
use crate::history::HistoryEntry;

use super::walks;

/// The retired keys' numbering rows no row in `rows` reads through, as
/// the history stands once the sweep's renumbering is written. A row
/// in the app's own numbering reads through none, but a former stamp
/// is what a position kept under the row's earlier number is found by,
/// so a row's former stamp keeps its numbering.
pub(super) fn doomed(state: &AppState, store: &Numbering, rows: &[HistoryEntry]) -> Vec<String> {
    let needed: HashSet<&str> = rows
        .iter()
        .filter(|r| !r.own_number || store.former(&r.id).is_some())
        .map(|r| r.id.as_str())
        .collect();
    store
        .slugs()
        .filter(|s| !walks(state, s) && !needed.contains(s))
        .map(str::to_string)
        .collect()
}

/// How many of `store`'s rows are a retired key's.
pub(super) fn retired(state: &AppState, store: &Numbering) -> usize {
    store.slugs().filter(|s| !walks(state, s)).count()
}

/// Drop the numbering rows of `slugs`.
pub(super) fn forget(state: &AppState, slugs: &[String]) -> std::io::Result<()> {
    if slugs.is_empty() {
        return Ok(());
    }
    crate::commands::numbering_offset::forget_where(state, |slug| slugs.iter().any(|s| s == slug))
}
