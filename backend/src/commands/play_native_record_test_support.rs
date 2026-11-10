//! The forms the tests record and write with — split from
//! [`super`] so the per-file complexity bar measures the play's own
//! code.

use super::*;

/// [`record_watch_requested_at`] for a watch whose request begins as it
/// is recorded — the form the tests record with. Every production
/// caller takes its request's moment first.
pub(crate) async fn record_watch(state: &AppState, watch: &Watch, kitsu_id: Option<&str>) {
    let now = crate::history::guard::epoch(&state.history_path);
    record_watch_requested_at(state, watch, kitsu_id, now, None).await;
}

/// [`write_history_then`] with nothing settled beside the row — the
/// form the tests write with; the play's own path settles the card note.
pub(crate) fn write_history(
    state: &AppState,
    native: &NativeResolved,
    requested: &str,
    asked: Asked<'_>,
    kitsu_id: Option<&str>,
) {
    write_history_then(state, native, requested, asked, kitsu_id, || {});
}
