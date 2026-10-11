//! Noting Kitsu serving an id — from the cache or from a fetch in hand
//! — and taking its gone mark in the same step; split from [`super`] so
//! each file stays inside the CRAP gate's per-file bar.

use super::*;

/// Publish what a read Kitsu served at `fetched` found: take the id's
/// mark and run `cache` (the detail row's write), as one step —
/// unless Kitsu has since answered a newer read that the id is gone.
/// That answer stands: the older success caches nothing over it and
/// reports it.
///
/// # Errors
/// - A not-found error when a newer read was answered gone: the read
///   answers as the mark does.
/// - A mark that cannot be deleted: the id is not reported served
///   while its mark still says it is gone, and no row is cached
///   beside it.
pub(crate) fn publish_served(
    state: &AppState,
    id: &str,
    fetched: Epoch,
    cache: impl FnOnce(),
) -> Result<()> {
    crate::history::guard::hold(&state.history_path, |held| {
        if held.kitsu_gone_since(fetched, id) {
            return Err(AniError::Upstream { status: 404 });
        }
        take_mark(state, id)?;
        cache();
        Ok(())
    })
}

/// A detail served from the cache under `key`: read the row, and when
/// `parse` takes it, note Kitsu serving `id` and take its mark — one
/// step, so a 404 noted between the read and the taking cannot be the
/// mark taken. `None` when there is no row `parse` takes.
///
/// # Errors
/// SQLite read failures, and a mark that cannot be deleted.
pub(crate) fn served_from_cache<T>(
    state: &AppState,
    id: &str,
    key: &str,
    parse: impl FnOnce(&str) -> Option<T>,
) -> Result<Option<(T, String)>> {
    crate::history::guard::hold(&state.history_path, |held| {
        let Some(body) = meta_cache_get(&state.cache_pool, key)? else {
            return Ok(None);
        };
        let Some(value) = parse(&body) else {
            return Ok(None);
        };
        held.kitsu_served(id);
        take_mark(state, id)?;
        Ok(Some((value, body)))
    })
}

/// Kitsu served `id` now, to a lookup that has the detail in hand:
/// note it, take the id's mark and run `cache` as one step, then run
/// `then` — the point a test fails another read of `id` at.
///
/// The served moment and the mark go in one hold: a read begun after
/// the moment is newer than this answer, and the mark its failure
/// writes must not be the one taken here.
///
/// # Errors
/// A mark that cannot be deleted; nothing is cached then.
pub(crate) fn served_now_then(
    state: &AppState,
    id: &str,
    cache: impl FnOnce(),
    then: impl FnOnce(&AppState),
) -> Result<()> {
    crate::history::guard::hold(&state.history_path, |held| -> Result<()> {
        held.kitsu_served(id);
        take_mark(state, id)?;
        cache();
        Ok(())
    })?;
    then(state);
    Ok(())
}

/// [`served_now_then`] with nothing cached — the seam a test notes a
/// success through.
#[cfg(test)]
pub(crate) fn note_served_then(state: &AppState, id: &str, served: impl FnOnce(&AppState)) {
    served_now_then(state, id, || {}, served).expect("mark taken");
}
