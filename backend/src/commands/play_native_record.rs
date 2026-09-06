//! What a successful native resolve leaves behind.
//!
//! Two writes follow every resolve that a user asked for, and they
//! are the same writes whether the stream ends up in the embedded
//! player, in mpv, or in Syncplay. They live here so the paths
//! cannot drift: the handoff once wrote the display number the UI
//! had shown instead of the slot, which pointed a row at whichever
//! episode happened to occupy that slot in the provider's listing.

use crate::app::AppState;
use crate::commands::play_native_resolve::NativeResolved;

/// Stamp the show's numbering offset while it is known.
///
/// The cache-hit and mark-watched writers and the history read
/// boundary all key on it by slug, and none of them has a listing to
/// derive it from. When the matched row's display tag differs from
/// its slot, the pair goes in too so the boundary can translate
/// between the number the provider stores and the one the UI shows.
///
/// Prefetches stamp as well: their resolve is exactly as
/// authoritative as a click's.
pub(crate) fn stamp_numbering(state: &AppState, native: &NativeResolved) {
    match &native.resolved_tag {
        Some(tag)
            if !crate::commands::play_native_episode::tag_matches(
                tag,
                &native.resolved_slot.to_string(),
            ) =>
        {
            crate::commands::anidb_offset::put_display(
                state,
                &native.slug,
                native.numbering_offset,
                native.resolved_slot,
                tag,
            );
        }
        _ => crate::commands::anidb_offset::put(state, &native.slug, native.numbering_offset),
    }
}

/// Stamp the watch's moment beside the show's history row.
///
/// The stamp orders Continue Watching and, when two providers have
/// each left a row for one show, picks the one to resume from. The
/// embedded player stamps on mark-watched, once playback has
/// reported progress; a handoff has no progress to wait for — the
/// launch is the watch — so it stamps as it writes the row. A failed
/// write is logged and swallowed, like the row's.
pub(crate) fn stamp_watched_now(state: &AppState, native: &NativeResolved) {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    if let Err(e) = crate::commands::kitsu::watched_at_put(state, &native.slug, now_ms) {
        tracing::warn!(
            show_id = %native.slug,
            error = ?e,
            "watched-at stamp write failed after handoff",
        );
    }
}

/// Record the watch.
///
/// `ep_no` is the matched row's own slot — exactly what a resume
/// looks up, whatever space the display tags live in. A failed
/// write is logged and swallowed: the stream is resolved and the
/// user is waiting on it.
///
/// `requested` is the episode the caller asked for, for the log line
/// only; it is the display number and must never reach the file.
pub(crate) fn write_history(state: &AppState, native: &NativeResolved, requested: &str) {
    let entry = crate::history::HistoryEntry {
        ep_no: native.resolved_slot.to_string(),
        id: native.slug.clone(),
        title: native.title.clone(),
    };
    if let Err(e) = crate::history::upsert_and_write(&state.history_path, entry) {
        tracing::warn!(
            title = %native.title,
            episode = %requested,
            error = ?e,
            "history write failed after native resolve",
        );
    }
}
