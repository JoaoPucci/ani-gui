//! The episode a history row reads as now, for a Continue card about
//! to resume from it: the card read the row when the page loaded, and
//! a probe since may have put the row's numbering right.

use serde::Serialize;

use crate::app::AppState;
use crate::commands::numbering_offset::Numbering;
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
    // One hold for the row, the pair it reads through and the former
    // tag kept beside the pair: what a writer puts together is read
    // together. The numbering store is read once, and the row's
    // number and its former one both come from that read: a repair
    // takes no history guard, and one landing between two reads would
    // answer the row's old number as its own former one.
    crate::history::guard::hold(&state.history_path, |_| {
        let numbering = Numbering::read(&state.history_path);
        let Some(row) = super::history::history_list_through(state, &numbering)?
            .into_iter()
            .find(|e| e.id == id)
        else {
            return Ok(None);
        };
        Ok(Some(HistoryNumber {
            former_ep_no: former_number(&numbering, id, &row.ep_no),
            ep_no: row.ep_no,
        }))
    })
}

/// The number a row reading `ep_no` read as before its show's stamp
/// was put right — while the stamp's row is the episode it reads.
fn former_number(numbering: &Numbering, slug: &str, ep_no: &str) -> Option<String> {
    use crate::commands::numbering_offset::kitsu_ep_no;
    let (_, now) = numbering.display(slug)?;
    let (_, former) = numbering.former(slug)?;
    let offset = numbering.offset(slug);
    let same = |a: &str, b: &str| match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    };
    same(&kitsu_ep_no(&now, offset), ep_no).then(|| kitsu_ep_no(&former, offset))
}

#[cfg(test)]
#[path = "history_number_test.rs"]
mod tests;
