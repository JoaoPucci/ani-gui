//! The episode a history row reads as now, for a Continue card about
//! to resume from it: the card read the row when the page loaded, and
//! a probe since may have put the row's numbering right.

use serde::Serialize;

use crate::app::AppState;
use crate::error::Result;

/// A history row's episode as the read boundary presents it now, and
/// the number it read as before a stamp repair put it right.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HistoryNumber {
    /// The row's episode, as [`super::history::history_list`] reads it.
    pub ep_no: String,
    /// The number the row read as before its show's display stamp was
    /// put right — what a position the player kept then is under.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub former_ep_no: Option<String>,
}

/// The number the history row for show `id` reads as now; `None` when
/// no row is the show's.
///
/// # Errors
/// As [`super::history::history_list`].
pub fn history_number(_state: &AppState, _id: &str) -> Result<Option<HistoryNumber>> {
    Ok(None)
}

#[cfg(test)]
#[path = "history_number_test.rs"]
mod tests;
