//! Reading what a stamp repair left: the tag it replaced, and the stamp
//! beside a history file. Split from [`super`] for the per-file
//! complexity bar.

use super::super::numbering_offset_store::{parse, store_path, Row};
use super::AppState;

/// The (slot, tag) a repaired display stamp named its row by before
/// [`super::retag_display`] put it right.
pub fn former_stamp(state: &AppState, slug: &str) -> Option<(u32, String)> {
    former_of(&row_of(state, slug)?)
}

/// The display stamp the sidecar beside the history file at
/// `history_path` keeps for the slug — [`crate::commands::numbering_offset::display_stamp`] for a
/// caller that holds the path rather than the state.
pub fn stamp_beside(history_path: &std::path::Path, slug: &str) -> Option<(u32, String)> {
    let body = std::fs::read_to_string(store_path(history_path)).unwrap_or_default();
    parse(&body).into_iter().find(|r| r.slug == slug)?.display
}

fn row_of(state: &AppState, slug: &str) -> Option<Row> {
    let body = std::fs::read_to_string(store_path(&state.history_path)).unwrap_or_default();
    parse(&body).into_iter().find(|r| r.slug == slug)
}

/// The row's former pair, while its stamp still names that slot.
fn former_of(row: &Row) -> Option<(u32, String)> {
    let (slot, _) = row.display.as_ref()?;
    row.former.clone().filter(|(was, _)| was == slot)
}
