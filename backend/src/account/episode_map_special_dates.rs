//! The special rule's dates: a whole day, a run by month, and the side
//! of the season's start a slot is on. Split from [`super`] so each file
//! stays inside the CRAP gate's per-file bar.

use crate::account::episode_map::{KitsuShow, TrackerShow};

/// Whether a special that started on `start` can take the slot after
/// Kitsu's episode `after` of `season`, which counts `count` episodes,
/// on the tracker's `entry`. Dates decide every slot:
/// - before the season's first episode, a prologue's: the special
///   started inside the entry's run and no more than the sources' slack
///   after the season did;
/// - after the season's last episode, an epilogue's: the special started
///   on or after the season ended, or within the slack before it, which
///   may be past the entry's run;
/// - after any other episode, a recap's: the special started inside the
///   entry's run, and the season, as the dates settle it, first.
///
/// Each compares whole days where it counts days, and a date that is not
/// one settles nothing.
pub(super) fn dated_for_slot(
    start: &str,
    season: &KitsuShow,
    entry: &TrackerShow,
    after: u32,
    count: u32,
) -> bool {
    use crate::account::episode_map_rules::{started_before, within_slack};
    if after == count && after > 0 {
        return within_slack(Some(start), season.end.as_deref());
    }
    in_run(start, entry)
        && if after == 0 {
            within_slack(season.start.as_deref(), Some(start))
        } else {
            started_before(season.start.as_deref(), Some(start))
        }
}

/// `date` when it names a real whole day.
pub(super) fn whole_day(date: Option<&str>) -> Option<&str> {
    date.filter(|d| {
        crate::account::episode_map_calendar::parse_date(d).is_some_and(|p| p.day.is_some())
    })
}

/// Whether `start` falls in `entry`'s run, compared by month.
fn in_run(start: &str, entry: &TrackerShow) -> bool {
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
