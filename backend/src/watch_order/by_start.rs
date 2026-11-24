//! Start-date order for run members: start date ascending, unknown
//! last, then Kitsu id.

use std::cmp::Ordering;

use super::js_text::js_number;
use super::WatchEntry;

/// Two ids' tie-break: by their numbers as `Number(id)` reads them
/// (Kitsu ids are numbers), two with one number tying as they did
/// there. An id that is not one sorts after every id that is, and among
/// its kind by its text, so the order stays total.
fn by_id(a: &str, b: &str) -> Ordering {
    let (x, y) = (js_number(a), js_number(b));
    match (x.is_nan(), y.is_nan()) {
        (false, false) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        (false, true) => Ordering::Less,
        (true, false) => Ordering::Greater,
        (true, true) => a.cmp(b),
    }
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
    by_id(&a.id, &b.id)
}

/// `list` sorted by [`by_start`] (a stable sort, as the renderer's was).
pub fn sort_by_start<T: std::borrow::Borrow<WatchEntry>>(list: &mut [T]) {
    list.sort_by(|a, b| by_start(a.borrow(), b.borrow()));
}
