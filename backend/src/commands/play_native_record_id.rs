//! What the history row's Kitsu id becomes once a watch is recorded —
//! split from `play_native_record` for the per-file complexity bar.

use super::Watch;
use crate::app::AppState;
use crate::history::guard::{Epoch, Held};

/// The given id when the cache can already say the cour guard accepts
/// pairing it with the watch, so it can go on with the row's own
/// write. Otherwise none, and the id waits for the guard's verdict, so
/// the row never carries an id the guard refuses.
pub(super) fn judged_by_cache(
    state: &AppState,
    watch: &Watch,
    given: Option<&str>,
) -> Option<String> {
    given
        .filter(|k| {
            crate::commands::kitsu::cached_cour_pairing_verdict(state, &watch.title, k)
                == Some(false)
        })
        .and_then(crate::history::kitsu_id_of)
}

/// Put an id the guard accepted on the row, once the guard has read
/// Kitsu for it. An id that is not digits is not recorded.
pub(super) fn add_accepted_id(state: &AppState, watch: &Watch, accepted: &str, begun: Epoch) {
    let Some(id) = crate::history::kitsu_id_of(accepted) else {
        return;
    };
    if let Err(e) = patch_id(state, watch, Some(id), begun) {
        tracing::warn!(
            show_id = %watch.show_id,
            error = ?e,
            "history id write failed after the guard accepted the pairing",
        );
    }
}

/// The Kitsu id the show's row records now, if any, read with the
/// history held. A history that cannot be read fails the read rather
/// than answering none: the refusal would clear the id on that answer.
///
/// # Errors
/// The file exists but cannot be read.
pub(super) fn recorded_id(held: &Held<'_>, show_id: &str) -> crate::error::Result<Option<String>> {
    Ok(held
        .rows()?
        .into_iter()
        .find(|e| e.id == show_id)
        .and_then(|e| e.kitsu_id))
}

/// The row's id once the cour guard refused pairing the watch with
/// `refused`. A refused pairing is the poison the guard exists for — a
/// Part 2 stream played from the cour-1 page — and on the row it would
/// steer Continue Watching and the detail page's resume to the wrong
/// entry, so the row does not keep it. An id the row held before is
/// judged the same way, as the refused mapping write judges the
/// mapping already stored: kept when the title agrees with it, cleared
/// when the title disagrees with it too.
pub(super) async fn settle_refused_id(
    state: &AppState,
    watch: &Watch,
    refused: &str,
    previous: Option<String>,
    begun: Epoch,
) {
    let keep = match previous {
        Some(p)
            if !crate::history::same_kitsu_id(&p, refused)
                && !crate::commands::kitsu::cour_pairing_disagrees(state, &watch.title, &p)
                    .await =>
        {
            Some(p)
        }
        _ => None,
    };
    if let Err(e) = patch_id(state, watch, keep, begun) {
        tracing::warn!(
            show_id = %watch.show_id,
            error = ?e,
            "history id write failed after a refused pairing",
        );
    }
}

/// Set the id on the watch's row, unless the row changed since the
/// recording wrote it: a removed row is gone, and a row a later watch
/// wrote is that watch's to settle.
fn patch_id(
    state: &AppState,
    watch: &Watch,
    kitsu_id: Option<String>,
    begun: Epoch,
) -> crate::error::Result<()> {
    crate::history::guard::hold(&state.history_path, |held| {
        if held.show_changed_since(begun, &watch.show_id) {
            return Ok(());
        }
        held.set_kitsu_id(&watch.show_id, kitsu_id)
    })
}
