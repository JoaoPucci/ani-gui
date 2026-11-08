//! Reading every store the retired-provider sweep needs before it
//! writes any — split from [`super`] for the per-file complexity bar;
//! the writes are [`super::apply`].

use crate::app::AppState;
use crate::commands::numbering_offset::Numbering;
use crate::error::{AniError, Result};

use super::{cache, history, numbering};

/// Everything the sweep will write, decided from one read of each
/// store.
pub(super) struct Work {
    pub(super) plan: history::HistoryPlan,
    pub(super) numbering: Vec<String>,
    pub(super) retired_numbering: usize,
    pub(super) cache: Vec<String>,
}

/// Read the history, the numbering sidecar and the cache, and decide
/// the writes. A store that exists and cannot be read is an error.
pub(super) fn gather(state: &AppState) -> Result<Work> {
    let mut rows = crate::history::read_all(&state.history_path)?;
    let store = Numbering::try_read(&state.history_path).map_err(|e| {
        tracing::warn!(error = %e, "retired-provider sweep: the numbering sidecar could not be read");
        AniError::Io
    })?;
    let plan = history::plan_history(state, &store)?;
    // The history as it stands once the renumbering is written; a
    // removed row's own stamp, mapping, numbering and title matches go
    // with its removal, so it counts as remaining here.
    for row in &mut rows {
        if plan.renumber.iter().any(|(id, _)| id == &row.id) {
            row.own_number = true;
        }
    }
    Ok(Work {
        numbering: numbering::doomed(state, &store, &rows),
        retired_numbering: numbering::retired(state, &store),
        cache: cache::doomed(state, &rows)?,
        plan,
    })
}
