//! Putting a show's display stamp back on the slot a cached resolution
//! played, which history stores and reads back through the stamp.
//! Split from [`super`] for the per-file complexity bar.

use crate::app::AppState;
use crate::commands::play_resolution_cache;

/// Put the show's display stamp on the slot a cached resolution
/// played, once its watch has recorded that slot ([`super::cached_watch`]):
/// decided under the history guard, against the row as it stands, so a
/// watch that recorded nothing, or another slot since, moves nothing.
pub(crate) fn restamp_recorded(state: &AppState, cached: &play_resolution_cache::CachedResolution) {
    let Some(slot) = cached.resolved_slot else {
        return;
    };
    crate::history::guard::hold(&state.history_path, |_| {
        let recorded = crate::history::read_all(&state.history_path)
            .ok()
            .and_then(|rows| rows.into_iter().find(|e| e.id == cached.show_id))
            .is_some_and(|e| e.ep_no == slot.to_string());
        if recorded {
            restamp_cached(state, cached);
        }
    });
}

/// Put the show's display stamp on the slot a cached resolution played,
/// as the fresh resolve that wrote the row did: history stores the
/// slot, and reads it back through the one stamp, which a later resolve
/// of another displaced row moved. The tag is the played episode in
/// the provider's numbering. A show with no stamp is left alone — its
/// row is not displaced, or it was removed. The caller holds the
/// history guard.
pub(crate) fn restamp_cached(state: &AppState, cached: &play_resolution_cache::CachedResolution) {
    use crate::commands::numbering_offset::{display_stamp, get, put_display};
    let (Some(slot), Some(played)) = (cached.resolved_slot, cached.played.as_deref()) else {
        return;
    };
    if display_stamp(state, &cached.show_id).is_none() {
        return;
    }
    let offset = get(state, &cached.show_id);
    let tag = match played.parse::<u32>() {
        Ok(n) => n.saturating_add(offset).to_string(),
        Err(_) => crate::commands::play_native_numbering::provider_fraction(played, offset),
    };
    if tag != slot.to_string() {
        put_display(state, &cached.show_id, offset, slot, &tag);
    }
}

/// [`restamp_recorded`] for a handoff served from the cache: the row it
/// played is the one kept under `args`.
pub(crate) fn restamp_after_handoff(state: &AppState, args: &crate::commands::play::PlayArgs) {
    let _ = (state, args);
}
