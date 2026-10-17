//! The numbering a resolve that records no watch leaves behind. Split
//! from [`super`] for the per-file complexity bar.

use crate::app::AppState;
use crate::commands::play_native_resolve::NativeResolved;
use crate::history::guard::Asked;

/// [`super::stamp_numbering`] for a resolve that records no watch — a
/// prefetch, a warm: the display stamp is the one the show's history
/// row reads through, and only a watch moves it. A former tag is still
/// put right, and the offset stamped.
pub(crate) fn stamp_numbering_unwatched(
    state: &AppState,
    native: &NativeResolved,
    asked: Asked<'_>,
) {
    crate::history::guard::hold(&state.history_path, |held| {
        if !held.removed_since(asked, &native.slug) {
            crate::commands::numbering_offset::retag_from_resolve(state, native);
            crate::commands::numbering_offset::put(state, &native.slug, native.numbering_offset);
        }
    });
}
