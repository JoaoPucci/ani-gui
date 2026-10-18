//! Reading what a stamp repair left: the tag it replaced, and the stamp
//! beside a history file. Split from [`super`] for the per-file
//! complexity bar.

use super::super::Numbering;
use super::AppState;

/// The (slot, tag) a repaired display stamp named its row by before
/// [`super::retag_display`] put it right.
pub fn former_stamp(state: &AppState, slug: &str) -> Option<(u32, String)> {
    Numbering::read(&state.history_path).former(slug)
}

/// The display stamp the sidecar beside the history file at
/// `history_path` keeps for the slug — [`crate::commands::numbering_offset::display_stamp`] for a
/// caller that holds the path rather than the state.
pub fn stamp_beside(history_path: &std::path::Path, slug: &str) -> Option<(u32, String)> {
    Numbering::read(history_path).display(slug)
}
