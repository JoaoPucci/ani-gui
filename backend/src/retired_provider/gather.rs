//! Reading every store the retired-provider sweep needs before it
//! writes any, then writing — split from [`super`] for the per-file
//! complexity bar.

use crate::app::AppState;
use crate::commands::numbering_offset::Numbering;
use crate::error::{AniError, Result};

use super::{cache, history, numbering, renumber, RetiredSweepReport};

/// Everything the sweep will write, decided from one read of each
/// store.
pub(super) struct Work {
    plan: history::HistoryPlan,
    numbering: Vec<String>,
    retired_numbering: usize,
    cache: Vec<String>,
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

/// Write what `work` decided. A write that fails is logged; the
/// numbering a renumbering made unneeded is dropped only once the
/// renumbering is written.
pub(super) fn apply(state: &AppState, work: Work) -> RetiredSweepReport {
    let mut report = RetiredSweepReport::default();
    for (id, title) in &work.plan.superseded {
        match crate::commands::history::history_delete(state, id) {
            Ok(true) => report.superseded.push(title.clone()),
            Ok(false) => {}
            Err(e) => tracing::warn!(error = ?e, "retired-provider sweep: a row stays"),
        }
    }
    match renumber::renumber(state, &work.plan.renumber) {
        Ok(titles) => {
            report.renumbered = titles;
            match numbering::forget(state, &work.numbering) {
                // Counted across the whole sweep: a removed row took
                // its own numbering with it.
                Ok(()) => {
                    let now = numbering::retired(state, &Numbering::read(&state.history_path));
                    report.numbering_rows = work.retired_numbering.saturating_sub(now);
                }
                Err(e) => tracing::warn!(error = ?e, "retired-provider sweep: numbering stays"),
            }
        }
        Err(e) => tracing::warn!(error = ?e, "retired-provider sweep: rows not renumbered"),
    }
    match cache::delete(state, &work.cache) {
        Ok(n) => report.cache_rows = n,
        Err(e) => tracing::warn!(error = ?e, "retired-provider sweep: cache stays"),
    }
    report
}
