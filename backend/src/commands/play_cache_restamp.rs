//! The display pair a cached resolution's row reads through — what a
//! watch recorded from the row puts in with it. Split from [`super`]
//! for the per-file complexity bar.

use crate::app::AppState;
use crate::commands::play_native_record::WatchedDisplay;
use crate::commands::play_resolution_cache;

/// The (slot, tag) pair the row `cached` played reads through, as the
/// fresh resolve that wrote the row would have put it: the played
/// episode in the provider's numbering, under the show's offset; none
/// for an undisplaced row, or a row from before the slot was cached.
pub(crate) fn cached_display(
    state: &AppState,
    cached: &play_resolution_cache::CachedResolution,
) -> Option<WatchedDisplay> {
    let (slot, played) = (cached.resolved_slot?, cached.played.as_deref()?);
    let offset = crate::commands::numbering_offset::get(state, &cached.show_id);
    let tag = match played.parse::<u32>() {
        Ok(n) => n.saturating_add(offset).to_string(),
        Err(_) => crate::commands::play_native_numbering::provider_fraction(played, offset),
    };
    (tag != slot.to_string()).then(|| WatchedDisplay {
        slug: cached.show_id.clone(),
        offset,
        slot,
        tag,
    })
}
