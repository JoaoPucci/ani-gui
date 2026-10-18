//! Whether a chain segment's run lies inside Kitsu's: the dates the
//! chain rule compares. Split from the chain rule so each file stays
//! inside the CRAP gate's per-file bar.

use crate::account::episode_map::{Evidence, Segment, TrackerShow};

/// `YYYY-MM` of a date, `None` when the date is unknown or names no
/// month. Runs are compared by month: the sources disagree by a day
/// about the same broadcast often enough that a day would refuse
/// chains that are plainly right.
pub(super) fn month(date: Option<&str>) -> Option<&str> {
    let date = date?;
    let month = date.get(..7)?;
    let (year, mm) = month.split_at(4);
    (year.chars().all(|c| c.is_ascii_digit())
        && mm.len() == 3
        && mm.starts_with('-')
        && mm[1..].chars().all(|c| c.is_ascii_digit()))
    .then_some(month)
}

/// Days since 1970-01-01 of a full `YYYY-MM-DD` date, `None` for
/// anything less.
fn days(date: Option<&str>) -> Option<i64> {
    let date = date?;
    if date.len() != 10 || date.as_bytes()[4] != b'-' || date.as_bytes()[7] != b'-' {
        return None;
    }
    let y: i64 = date[..4].parse().ok()?;
    let m: i64 = date[5..7].parse().ok()?;
    let d: i64 = date[8..].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Days from civil (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// How far the sources may straddle a month boundary about one
/// broadcast and still mean the same day.
const DAYS_OF_SLACK: i64 = 2;

/// Whether `later` is no more than [`DAYS_OF_SLACK`] after `earlier`.
fn within_slack(earlier: Option<&str>, later: Option<&str>) -> bool {
    matches!((days(earlier), days(later)), (Some(a), Some(b)) if b - a <= DAYS_OF_SLACK)
}

/// Whether Kitsu's own episode `number` is dated in `bound`'s month.
fn dated_in(evidence: &Evidence, number: u32, bound: Option<&str>) -> bool {
    let aired = evidence.kitsu_airdates.get(&number).map(String::as_str);
    matches!((month(aired), month(bound)), (Some(a), Some(b)) if a == b)
}

/// Whether `show`'s run, filed as `segment`, lies inside Kitsu's run.
/// Each bound is inside by month, within [`DAYS_OF_SLACK`] of Kitsu's,
/// or vouched for by Kitsu's own air date for the segment's first
/// (start) or last (end) episode falling in the bound's month.
pub(super) fn inside_kitsu_run(evidence: &Evidence, show: &TrackerShow, segment: &Segment) -> bool {
    let (k_start, k_end) = (
        evidence.kitsu.start.as_deref(),
        evidence.kitsu.end.as_deref(),
    );
    let (start, end) = (show.start.as_deref(), show.end.as_deref());
    let (Some(ks), Some(ke), Some(s), Some(e)) =
        (month(k_start), month(k_end), month(start), month(end))
    else {
        return false;
    };
    let start_ok =
        s >= ks || within_slack(start, k_start) || dated_in(evidence, segment.kitsu_from, start);
    let end_ok = e <= ke || within_slack(k_end, end) || dated_in(evidence, segment.kitsu_to, end);
    start_ok && end_ok
}
