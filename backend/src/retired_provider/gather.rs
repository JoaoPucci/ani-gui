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

/// How many of the `before` retired numbering rows are gone now. When
/// the store cannot be read to tell, the `forgotten` rows the sweep's
/// own removal asked for, all of which were in the store the sweep
/// read: a removed history row may already have taken some of them,
/// and what such a removal took is left out, but the count never falls
/// below what this sweep's writes removed of them.
fn numbering_gone(state: &AppState, before: usize, forgotten: usize) -> usize {
    #[cfg(test)]
    let read = if FAIL_COUNT_READ.with(|f| f.replace(false)) {
        Err(std::io::Error::other("unreadable for the case"))
    } else {
        Numbering::try_read(&state.history_path)
    };
    #[cfg(not(test))]
    let read = Numbering::try_read(&state.history_path);
    match read {
        Ok(store) => before.saturating_sub(numbering::retired(state, &store)),
        Err(e) => {
            tracing::warn!(error = %e, "retired-provider sweep: numbering count unread");
            forgotten
        }
    }
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
    let mut forgotten = 0;
    match renumber::renumber(state, &work.plan.renumber) {
        Ok(titles) => {
            report.renumbered = titles;
            match numbering::forget(state, &work.numbering) {
                Ok(()) => forgotten = work.numbering.len(),
                Err(e) => tracing::warn!(error = ?e, "retired-provider sweep: numbering stays"),
            }
        }
        Err(e) => tracing::warn!(error = ?e, "retired-provider sweep: rows not renumbered"),
    }
    // Counted from the sidecar as it stands once every write has run:
    // a removed row took its own numbering with it, and a removal that
    // failed part way is counted for what it took before failing.
    report.numbering_rows = numbering_gone(state, work.retired_numbering, forgotten);
    match cache::delete(state, &work.cache) {
        Ok(n) => report.cache_rows = n,
        Err(e) => tracing::warn!(error = ?e, "retired-provider sweep: cache stays"),
    }
    report
}

#[cfg(test)]
thread_local! {
    static FAIL_COUNT_READ: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Make this thread's next count of the numbering that went fail its
/// read, as a sidecar unreadable once the writes are done would.
#[cfg(test)]
pub(super) fn fail_count_read_for_tests() {
    FAIL_COUNT_READ.with(|f| f.set(true));
}
