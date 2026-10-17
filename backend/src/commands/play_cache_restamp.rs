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
/// as a fresh play that records its watch does: history stores the
/// slot, and reads it back through the one stamp, which another watch,
/// or the prefetch that alone resolved this row, left elsewhere or
/// never wrote. The tag is the played episode in the provider's
/// numbering; an undisplaced row writes nothing. The caller holds the
/// history guard and has the show's row holding that slot.
pub(crate) fn restamp_cached(state: &AppState, cached: &play_resolution_cache::CachedResolution) {
    use crate::commands::numbering_offset::{get, put_display};
    let (Some(slot), Some(played)) = (cached.resolved_slot, cached.played.as_deref()) else {
        return;
    };
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
    let key = play_resolution_cache::cache_key(
        &args.title,
        &args.mode,
        args.quality.as_deref().unwrap_or("best"),
        &args.episode,
        args.year,
        args.episode_count,
        args.subtype.as_deref(),
    );
    if let Ok(Some(cached)) = play_resolution_cache::get(&state.cache_pool, &key) {
        restamp_recorded(state, &cached);
    }
}
