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
/// reported progress; a handoff stamps once the player has started.
/// A failed write is logged and swallowed, like the row's.
pub(crate) fn stamp_watched_now(state: &AppState, show_id: &str) {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    if let Err(e) = crate::commands::kitsu::watched_at_put(state, show_id, now_ms) {
        tracing::warn!(
            show_id = %show_id,
            error = ?e,
            "watched-at stamp write failed",
        );
    }
}

/// A watch a handoff will record once its player has started: the
/// history row's three fields, from a fresh resolve or a cached one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Watch {
    /// The show key's string.
    pub show_id: String,
    /// The provider's title for the row.
    pub title: String,
    /// The row's episode number, in the provider's numbering.
    pub ep_no: String,
}

impl Watch {
    /// The watch a fresh resolve describes.
    #[must_use]
    pub fn of(native: &NativeResolved) -> Self {
        Self {
            show_id: native.slug.clone(),
            title: native.title.clone(),
            ep_no: native.resolved_slot.to_string(),
        }
    }
}

/// Record a handoff's watch: the history row, the watched-at stamp
/// and, when the caller knows the Kitsu id, the show's reverse
/// mapping — written once the player has started; the spawn is the
/// watch, and a player that failed to start leaves nothing behind.
/// The mapping is what lets the row be found by the Kitsu id, and
/// what puts its stamp in the running when two providers have each
/// left a row; the embedded player writes it on mark-watched, which
/// a handoff never reaches. A watch without a show id (a cached row
/// from before the field) records nothing.
///
/// The row leads and the rest follow: a history write that fails —
/// the state directory unwritable or full while the cache is not —
/// ends the recording with a log line, since a stamp advanced for a
/// watch that never reached the file would make that row the show's
/// latest and hand the resume its stale episode over another
/// provider's real one.
pub(crate) async fn record_watch(state: &AppState, watch: &Watch, kitsu_id: Option<&str>) {
    if watch.show_id.is_empty() {
        return;
    }
    let entry = crate::history::HistoryEntry {
        ep_no: watch.ep_no.clone(),
        id: watch.show_id.clone(),
        title: watch.title.clone(),
    };
    if let Err(e) = crate::history::upsert_and_write(&state.history_path, entry) {
        tracing::warn!(
            show_id = %watch.show_id,
            error = ?e,
            "history write failed after handoff; the watch is not stamped",
        );
        return;
    }
    stamp_watched_now(state, &watch.show_id);
    if let Some(kid) = kitsu_id.filter(|k| !k.is_empty()) {
        crate::commands::kitsu::try_put_allmanga_kitsu_mapping(
            state,
            &watch.show_id,
            &watch.title,
            kid,
        )
        .await;
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
