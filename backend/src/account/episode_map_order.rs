//! Whether one run is proven to have started before another: what the
//! chain and split-parts rules read to refuse a "later" part that aired
//! first. Split from [`super`] so each file stays inside the CRAP gate's
//! per-file bar.

use super::dates::month;

/// Whether `later` started before `earlier`, as far as the dates settle
/// it: both real dates or months, a month apart with `later`'s the
/// earlier month, or real whole days in one month with `later`'s the
/// earlier day. A date missing, impossible, a year alone, or a month
/// named alone in the other's month, and the same day, settle nothing.
#[must_use]
pub(super) fn started_before(later: Option<&str>, earlier: Option<&str>) -> bool {
    let (Some(l), Some(e)) = (month(later), month(earlier)) else {
        return false;
    };
    if l != e {
        return l < e;
    }
    match (whole_day(later), whole_day(earlier)) {
        (Some(l), Some(e)) => l < e,
        _ => false,
    }
}

/// `date` when it names a real whole day.
fn whole_day(date: Option<&str>) -> Option<&str> {
    date.filter(|d| {
        crate::account::episode_map_calendar::parse_date(d).is_some_and(|p| p.day.is_some())
    })
}
