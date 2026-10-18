//! What a successful native resolve leaves behind.
//!
//! Two writes follow every resolve that a user asked for, and they
//! are the same writes whether the stream ends up in the embedded
//! player, in mpv, or in Syncplay. They live here so the paths
//! cannot drift: every one records the episode the play resolved to,
//! as the app numbers it, marked as such
//! ([`crate::history::HistoryEntry::own_number`]).

use crate::app::AppState;
use crate::commands::play_native_resolve::NativeResolved;
use crate::history::guard::{Asked, Epoch};

#[path = "play_native_record_id.rs"]
mod record_id;
use record_id::{add_accepted_id, judged_by_cache, recorded_id, settle_refused_id};

#[path = "play_native_record_matched.rs"]
mod matched;
pub(crate) use matched::matched_episode;

/// Put a stamp an earlier version wrote with a row's former tag right,
/// and stamp the show's numbering offset, for a show the sidecar has no
/// row for ([`put_numbering`]).
///
/// The history read boundary keys on it by slug for rows from before
/// rows recorded the app's own number, which hold a provider slot.
///
/// Prefetches stamp as well: their resolve is exactly as
/// authoritative as a click's.
///
/// `asked` is the request the resolve served: a show removed from
/// history since it began is not stamped, the removal having taken its
/// numbering with the row ([`crate::history::guard`]).
pub(crate) fn stamp_numbering(state: &AppState, native: &NativeResolved, asked: Asked<'_>) {
    crate::history::guard::hold(&state.history_path, |held| {
        if !held.removed_since(asked, &native.slug) {
            // The listing is at hand: a stamp an earlier version wrote
            // with a row's former tag gets the row's tag now.
            crate::commands::numbering_offset::retag_from_resolve(state, native);
            put_numbering(state, native);
        }
    });
}

/// The offset, for a show that has none yet. Rows record the app's own
/// number now; only rows from before read through the sidecar, and the
/// offset and display stamp each of those reads through were stamped
/// when it was written, so a later resolve — a re-listing's included —
/// leaves both as they are. A display stamp is no longer written.
fn put_numbering(state: &AppState, native: &NativeResolved) {
    if !crate::commands::numbering_offset::has(state, &native.slug) {
        crate::commands::numbering_offset::put(state, &native.slug, native.numbering_offset);
    }
}

/// A fresh play's numbering, then its history row — the order a
/// handoff keeps too: the offset and any former tag are put right
/// first, whatever happens. A play overtaken by a later watch records
/// no row.
pub(crate) fn record_fresh_watch(
    state: &AppState,
    native: &NativeResolved,
    requested: &str,
    asked: Asked<'_>,
) {
    stamp_numbering(state, native, asked);
    write_history(state, native, requested, asked);
}

/// The wall clock's moment, in milliseconds since the epoch.
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Stamp the watch's moment in the cache, beside the show's history
/// row — the same moment the row itself carries
/// ([`record_watch_requested_at`]).
///
/// The stamp orders Continue Watching and, when two providers have
/// each left a row for one show, picks the one to resume from. The
/// embedded player stamps on mark-watched, once playback has
/// reported progress; a handoff stamps once the player has started.
/// A failed write is logged and swallowed, like the row's; a row
/// that could not be written is not stamped at all. The row carries
/// the moment too, so a cache that refuses the stamp does not leave
/// the row ranked as unwatched below a sibling's older stamp.
fn stamp_watched_at(state: &AppState, show_id: &str, now_ms: i64) {
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
    /// The episode watched, as the app numbers it: the one the resolve
    /// matched ([`matched_episode`]) — the number the play asked for,
    /// unless that named the row by a tag it no longer carries.
    pub ep_no: String,
}

impl Watch {
    /// The watch a fresh resolve of `episode` describes.
    #[must_use]
    pub fn of(native: &NativeResolved, episode: &str) -> Self {
        Self {
            show_id: native.slug.clone(),
            title: native.title.clone(),
            ep_no: matched_episode(native, episode),
        }
    }
}

/// Record a watch: the history row, the watched-at stamp and, when
/// the caller knows the Kitsu id, the id on the row and the show's
/// reverse mapping. A handoff records once its player has started —
/// the spawn is the watch, and a player that failed to start leaves
/// nothing behind; the embedded player records on mark-watched, once
/// playback has reported progress. The mapping is what lets the row be
/// found by the Kitsu id, and what puts its stamp in the running when
/// two providers have each left a row. A watch without a show id (a
/// cached row from before the field) records nothing.
///
/// The row leads and the rest follow: a history write that fails —
/// the state directory unwritable or full while the cache is not —
/// ends the recording with a log line, since a stamp advanced for a
/// watch that never reached the file would make that row the show's
/// latest and hand the resume its stale episode over another
/// provider's real one. The row and then the stamp go down before
/// anything waits on Kitsu.
///
/// The row carries the given Kitsu id only once the cour guard accepts
/// it: in the same write when the cached detail already answers the
/// guard, otherwise after the guard's one Kitsu read, whose verdict
/// settles both the mapping and the row's id ([`settle_refused_id`],
/// [`add_accepted_id`]).
///
/// The frontend does not wait for a recording, so the show can be
/// removed from history while the guard is still reading Kitsu. What
/// the recording writes after that read — the mapping, the id on the
/// row — it writes only if the show was not removed since the row went
/// down ([`crate::history::guard`]): a removal wins over a watch begun
/// before it.
///
/// `requested` is the moment the watch's request began. A handoff
/// resolves its stream before its player starts, and the show can be
/// removed from history while it resolves, under the key the resolve
/// lands on or another: the player still opens, and the watch is not
/// recorded — the removal stands, as it does over everything else a
/// play begun before it would write. So does a later watch of the
/// show recorded while this one's request stalled: its row and stamp
/// stay, and this watch writes nothing. That is not a failure — the
/// show's latest watch is on the row — so it returns quietly, like
/// every other outcome of a recording.
pub(crate) async fn record_watch_requested_at(
    state: &AppState,
    watch: &Watch,
    kitsu_id: Option<&str>,
    requested: Epoch,
) {
    if watch.show_id.is_empty() {
        return;
    }
    // Only a Kitsu id is one: anything else the page sent is no id, and
    // reaches neither the row nor the mapping and its played mark.
    let given = kitsu_id.and_then(crate::history::kitsu_id_in);
    let judged = judged_by_cache(state, watch, given);
    // The watch's moment travels with the row, in the same write, so
    // the recency the resume and the strip rank by is not left to a
    // store the row was not written to; the cache's copy follows.
    let now = now_ms();
    let entry = crate::history::HistoryEntry {
        ep_no: watch.ep_no.clone(),
        id: watch.show_id.clone(),
        title: watch.title.clone(),
        watched_at: Some(now),
        kitsu_id: judged.clone(),
        own_number: true,
    };
    // The row, its stamp and the moment the recording begins are one
    // step with the history held: a removal runs wholly before it —
    // and the watch is new, unless its request began before that
    // removal, when nothing is recorded — or wholly after it, and
    // takes the row and the stamp together. A watch recorded since the
    // request began is newer than this one, and keeps the row too.
    let asked = Asked {
        begun: requested,
        page: given,
    };
    let recorded = crate::history::guard::hold(&state.history_path, |held| {
        if held.overtaken_since(asked, &watch.show_id) {
            return Ok(None);
        }
        // The id the row held is read in the same hold as the write:
        // a read that fails fails the write, and is never taken for a
        // row without one.
        let previous = recorded_id(held, &watch.show_id)?;
        held.upsert(entry)?;
        // The row records the page only once the guard accepts it; a
        // removal before then still has to know it.
        held.played_from(&watch.show_id, given);
        stamp_watched_at(state, &watch.show_id, now);
        Ok::<_, crate::error::AniError>(Some((held.epoch(), previous)))
    });
    let (begun, previous) = match recorded {
        Ok(Some(recorded)) => recorded,
        Ok(None) => return,
        Err(e) => {
            tracing::warn!(
                show_id = %watch.show_id,
                error = ?e,
                "history write failed after handoff; the watch is not stamped",
            );
            return;
        }
    };
    let Some(kid) = given else {
        return;
    };
    let accepted = crate::commands::kitsu::try_put_allmanga_kitsu_mapping(
        state,
        &watch.show_id,
        &watch.title,
        kid,
        begun,
    )
    .await;
    if !accepted {
        settle_refused_id(state, watch, kid, previous, begun).await;
    } else if judged.is_none() {
        add_accepted_id(state, watch, kid, begun);
    }
}

/// Record the watch.
///
/// The row records the episode the resolve matched, as the app numbers
/// it ([`matched_episode`]), marked as the app's own number, so it
/// reads back as itself whatever the provider's slots or the sidecar
/// say. A failed write is logged and swallowed: the stream is resolved
/// and the user is waiting on it.
///
/// `requested` is the episode the caller asked for, as the app numbers
/// it — what the row records when the resolve names no row.
///
/// `asked` is the play's request: a show removed from history while
/// the play resolved, or watched since it began, gets no row from it
/// ([`crate::history::guard`]).
pub(crate) fn write_history(
    state: &AppState,
    native: &NativeResolved,
    requested: &str,
    asked: Asked<'_>,
) {
    let entry = crate::history::HistoryEntry {
        ep_no: matched_episode(native, requested),
        id: native.slug.clone(),
        title: native.title.clone(),
        // A resolve is not a watch: the row keeps the moment of the
        // watch before it, if any.
        watched_at: None,
        kitsu_id: None,
        own_number: true,
    };
    let wrote = crate::history::guard::hold(&state.history_path, |held| {
        if held.overtaken_since(asked, &native.slug) {
            return Ok(());
        }
        held.upsert(entry)?;
        held.played_from(&native.slug, asked.page);
        Ok::<_, crate::error::AniError>(())
    });
    if let Err(e) = wrote {
        tracing::warn!(
            title = %native.title,
            episode = %requested,
            error = ?e,
            "history write failed after native resolve",
        );
    }
}

/// [`record_watch_requested_at`] for a watch whose request begins as it
/// is recorded — the form the tests record with. Every production
/// caller takes its request's moment first.
#[cfg(test)]
pub(crate) async fn record_watch(state: &AppState, watch: &Watch, kitsu_id: Option<&str>) {
    let now = crate::history::guard::epoch(&state.history_path);
    record_watch_requested_at(state, watch, kitsu_id, now).await;
}

#[cfg(test)]
#[path = "play_native_record_test.rs"]
mod tests;

#[cfg(test)]
#[path = "play_native_record_ids_test.rs"]
mod ids_tests;
