//! The display stamp a watch leaves: put only once the watch recorded
//! the slot it names. Split from [`super`] for the per-file complexity
//! bar.

use crate::app::AppState;
use crate::commands::play_native_resolve::NativeResolved;
use crate::history::guard::Asked;

/// The (slot, display tag) pair a resolve's row reads through, for
/// the show it resolved — `None` where the tag is the slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchedDisplay {
    pub(crate) slug: String,
    pub(crate) offset: u32,
    pub(crate) slot: u32,
    pub(crate) tag: String,
}

/// The display pair `native`'s row reads through, where its tag is not
/// its slot.
pub(crate) fn watched_display(native: &NativeResolved) -> Option<WatchedDisplay> {
    let tag = native.resolved_tag.as_deref()?;
    if crate::commands::play_native_episode::tag_matches(tag, &native.resolved_slot.to_string()) {
        return None;
    }
    Some(WatchedDisplay {
        slug: native.slug.clone(),
        offset: native.numbering_offset,
        slot: native.resolved_slot,
        tag: tag.to_string(),
    })
}

/// Put `display` once the show's history row holds its slot.
pub(crate) fn put_if_recorded(state: &AppState, display: &WatchedDisplay) {
    let _ = (state, display);
}

/// A fresh play's numbering and history row.
pub(crate) fn record_fresh_watch(
    state: &AppState,
    native: &NativeResolved,
    requested: &str,
    asked: Asked<'_>,
) {
    super::stamp_numbering(state, native, asked);
    super::write_history(state, native, requested, asked);
}
