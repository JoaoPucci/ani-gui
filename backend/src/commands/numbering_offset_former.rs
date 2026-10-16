//! Display stamps an earlier version wrote with a row's former tag —
//! the tag a stitched listing gave a row before it was numbered as it
//! is now — and putting them right. Split from [`super`] for the
//! per-file complexity bar.

use super::{display_stamp, get, merge_row, AppState};

/// Put a display stamp an earlier version wrote right: when the slug's
/// stamp names one of `former_tags`' slots with the tag that version
/// gave it, it names the tag the row has now. Any other stamp is left
/// alone.
pub fn retag_display(state: &AppState, slug: &str, former_tags: &[(u32, String, String)]) {
    let Some((stamped, tag)) = display_stamp(state, slug) else {
        return;
    };
    if let Some((slot, _, now)) = former_tags
        .iter()
        .find(|(slot, former, _)| *slot == stamped && *former == tag)
    {
        merge_row(state, slug, get(state, slug), Some((*slot, now.clone())));
    }
}

/// The (slot, tag) a repaired display stamp named its row by before
/// [`retag_display`] put it right.
pub fn former_stamp(_state: &AppState, _slug: &str) -> Option<(u32, String)> {
    None
}
