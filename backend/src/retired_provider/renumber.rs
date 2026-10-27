//! Writing the kept retired rows in the app's own numbering — split
//! from [`super::history`] for the per-file complexity bar.

use crate::app::AppState;
use crate::commands::numbering_offset::Numbering;
use crate::error::Result;
use crate::history::HistoryEntry;
use crate::retired_provider_plan::Plan;

/// The rows as the read boundary shows them, in file order: each
/// episode in its entry's numbering, an appended extra read back as
/// the extra.
pub(super) fn shown_rows(state: &AppState) -> Result<Vec<HistoryEntry>> {
    let numbering = Numbering::read(&state.history_path);
    crate::commands::history::history_list_through(state, &numbering)
}

/// Write each planned row's episode as the read boundary shows it,
/// marked as the app's own number. Answers the titles rewritten.
pub(super) fn renumber(state: &AppState, the_plan: &Plan) -> Result<Vec<String>> {
    crate::history::guard::hold(&state.history_path, |held| {
        let mut rows = held.rows()?;
        let shown = shown_rows(state)?;
        let mut renumbered = Vec::new();
        for (row, seen) in rows.iter_mut().zip(&shown) {
            if row.own_number || !the_plan.renumber.contains(&row.id) {
                continue;
            }
            row.ep_no.clone_from(&seen.ep_no);
            row.own_number = true;
            renumbered.push(row.title.clone());
        }
        if !renumbered.is_empty() {
            held.write(&rows)?;
        }
        Ok(renumbered)
    })
}
