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
pub fn history_number(state: &AppState, id: &str) -> Result<Option<HistoryNumber>> {
    let Some(row) = super::history::history_list(state)?
        .into_iter()
        .find(|e| e.id == id)
    else {
        return Ok(None);
    };
    Ok(Some(HistoryNumber {
        former_ep_no: former_number(state, id, &row.ep_no),
        ep_no: row.ep_no,
    }))
}

/// The number a row reading `ep_no` read as before its show's stamp
/// was put right — while the stamp's row is the episode it reads.
fn former_number(state: &AppState, slug: &str, ep_no: &str) -> Option<String> {
    use crate::commands::numbering_offset::{display_stamp, former_stamp, get, kitsu_ep_no};
    let (_, now) = display_stamp(state, slug)?;
    let (_, former) = former_stamp(state, slug)?;
    let offset = get(state, slug);
    let same = |a: &str, b: &str| match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    };
    same(&kitsu_ep_no(&now, offset), ep_no).then(|| kitsu_ep_no(&former, offset))
}

#[cfg(test)]
#[path = "history_number_test.rs"]
mod tests;
