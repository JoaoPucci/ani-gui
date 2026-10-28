//! The special rule's dates: a whole day, a run by month, and the side
//! of the season's start a slot is on. Split from [`super`] so each file
//! stays inside the CRAP gate's per-file bar.

use crate::account::episode_map::TrackerShow;

/// Whether a special that started on `start` can take the slot after
/// Kitsu's episode `after` of a season that started on `season`: the
/// slot before the season's first episode is a prologue's, which a
/// special takes only where it started no more than the sources' slack
/// after the season; a slot after one of the season's episodes is a
/// recap's, which a special takes only where the season, as the dates
/// settle it, started first.
pub(super) fn dated_for_slot(start: &str, season: Option<&str>, after: u32) -> bool {
    if after == 0 {
        crate::account::episode_map_rules::within_slack(season, Some(start))
    } else {
        crate::account::episode_map_rules::started_before(season, Some(start))
    }
}

/// `date` when it names a real whole day.
pub(super) fn whole_day(date: Option<&str>) -> Option<&str> {
    date.filter(|d| {
        crate::account::episode_map_calendar::parse_date(d).is_some_and(|p| p.day.is_some())
    })
}

/// Whether `start` falls in `entry`'s run, compared by month.
pub(super) fn in_run(start: &str, entry: &TrackerShow) -> bool {
    match (
        month_of(Some(start)),
        month_of(entry.start.as_deref()),
        month_of(entry.end.as_deref()),
    ) {
        (Some(s), Some(from), Some(to)) => from <= s && s <= to,
        _ => false,
    }
}

/// `YYYY-MM` of a real date or month.
fn month_of(date: Option<&str>) -> Option<&str> {
    let date = date?;
    crate::account::episode_map_calendar::parse_date(date)?;
    date.get(..7)
}
