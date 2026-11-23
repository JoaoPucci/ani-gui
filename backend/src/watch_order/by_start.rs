//! Start-date order for run members: start date ascending, unknown
//! last, then Kitsu id.

use std::cmp::Ordering;

use super::WatchEntry;

/// An id as a number, for the tie-break (Kitsu ids are numeric).
fn id_number(id: &str) -> f64 {
    id.parse().unwrap_or(f64::NAN)
}

/// Start date ascending, unknown last, then Kitsu id.
#[must_use]
pub fn by_start(a: &WatchEntry, b: &WatchEntry) -> Ordering {
    match (&a.start, &b.start) {
        (Some(x), Some(y)) if x != y => return x.cmp(y),
        (None, Some(_)) => return Ordering::Greater,
        (Some(_), None) => return Ordering::Less,
        _ => {}
    }
    id_number(&a.id)
        .partial_cmp(&id_number(&b.id))
        .unwrap_or(Ordering::Equal)
}

/// `list` sorted by [`by_start`] (a stable sort, as the renderer's was).
pub fn sort_by_start<T: std::borrow::Borrow<WatchEntry>>(list: &mut [T]) {
    list.sort_by(|a, b| by_start(a.borrow(), b.borrow()));
}
