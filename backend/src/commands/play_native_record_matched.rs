//! The episode a resolve matched, as the app numbers it — what a
//! watch records ([`super::Watch::of`]).

use crate::commands::play_native_resolve::NativeResolved;

/// The app's own number for the row a resolve matched: its tag, or its
/// slot where it has none, in the entry's numbering. That is the
/// request whenever the request named the row by its tag now; a request
/// that found the row by a tag an earlier version gave it records the
/// row's tag instead, so the row reads back as the episode it is.
pub(crate) fn matched_episode(native: &NativeResolved, requested: &str) -> String {
    let shown = native
        .resolved_tag
        .clone()
        .unwrap_or_else(|| native.resolved_slot.to_string());
    // An integer-valued tag ("41.0") is the whole number it names.
    let shown = match shown.trim().parse::<f64>() {
        Ok(v) if v.fract() == 0.0 && v >= 0.0 && v <= f64::from(u32::MAX) => (v as u32).to_string(),
        _ => shown,
    };
    let own = crate::commands::numbering_offset::kitsu_ep_no(&shown, native.numbering_offset);
    if own.trim().is_empty() {
        requested.trim().to_string()
    } else {
        own
    }
}
