//! Writing the kept retired rows in the app's own numbering — split
//! from [`super::history`] for the per-file complexity bar.

use crate::app::AppState;
use crate::error::Result;

/// Write each `(id, episode)` row's episode, as the read boundary
/// showed it when the sweep read the stores, marked as the app's own
/// number. Answers the titles rewritten.
pub(super) fn renumber(state: &AppState, planned: &[(String, String)]) -> Result<Vec<String>> {
    if planned.is_empty() {
        return Ok(Vec::new());
    }
    #[cfg(test)]
    if FAIL_NEXT.with(|f| f.replace(false)) {
        return Err(crate::error::AniError::Io);
    }
    crate::history::guard::hold(&state.history_path, |held| {
        let mut rows = held.rows()?;
        let mut renumbered = Vec::new();
        for row in rows.iter_mut().filter(|r| !r.own_number) {
            if let Some((_, ep)) = planned.iter().find(|(id, _)| id == &row.id) {
                row.ep_no.clone_from(ep);
                row.own_number = true;
                renumbered.push(row.title.clone());
            }
        }
        if !renumbered.is_empty() {
            held.write(&rows)?;
        }
        Ok(renumbered)
    })
}

#[cfg(test)]
thread_local! {
    static FAIL_NEXT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Make this thread's next renumbering fail as an unwritable history
/// would, for a case about what the sweep reports when a write fails.
#[cfg(test)]
pub(super) fn fail_next_for_tests() {
    FAIL_NEXT.with(|f| f.set(true));
}
