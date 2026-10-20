//! Whether a chain segment's run lies inside Kitsu's: the dates the
//! chain rule compares. Split from the chain rule so each file stays
//! inside the CRAP gate's per-file bar.

use super::days::within_slack;
use crate::account::episode_map::{Evidence, Segment, TrackerShow};

/// `YYYY-MM` of a date, `None` when the date is unknown or is not a
/// real calendar date or month
/// ([`crate::account::episode_map_calendar::parse_date`]). Runs are compared by month: the sources disagree by a day
/// about the same broadcast often enough that a day would refuse
/// chains that are plainly right.
pub(super) fn month(date: Option<&str>) -> Option<&str> {
    let date = date?;
    crate::account::episode_map_calendar::parse_date(date)?;
    date.get(..7)
}

/// Whether Kitsu's own episode `number` is dated in `bound`'s month.
fn dated_in(evidence: &Evidence, number: u32, bound: Option<&str>) -> bool {
    let aired = evidence.kitsu_airdates.get(&number).map(String::as_str);
    matches!((month(aired), month(bound)), (Some(a), Some(b)) if a == b)
}

/// The bounds of `show`'s run, filed as `segment`, that the dates alone
/// leave outside Kitsu's run, each as the Kitsu episode whose own air
/// date could place it — the segment's first for its start, its last
/// for its end — and the bound's date. `None` when a date the comparison
/// needs is missing.
///
/// A start is inside when it is no earlier than Kitsu's start (by month,
/// or within [`super::days::DAYS_OF_SLACK`]) and no later than Kitsu's end; an end
/// when it is no later than Kitsu's end (by month, or within the slack)
/// and no earlier than Kitsu's start.
pub(super) fn unplaced_bounds<'a>(
    evidence: &Evidence,
    show: &'a TrackerShow,
    segment: &Segment,
) -> Option<Vec<(u32, &'a str)>> {
    let (k_start, k_end) = (
        evidence.kitsu.start.as_deref(),
        evidence.kitsu.end.as_deref(),
    );
    let (start, end) = (show.start.as_deref()?, show.end.as_deref()?);
    let (ks, ke, s, e) = (
        month(k_start)?,
        month(k_end)?,
        month(Some(start))?,
        month(Some(end))?,
    );
    let start_inside = (s >= ks || within_slack(Some(start), k_start)) && s <= ke;
    let end_inside = (e <= ke || within_slack(k_end, Some(end))) && e >= ks;
    let mut unplaced = Vec::new();
    if !start_inside {
        unplaced.push((segment.kitsu_from, start));
    }
    if !end_inside {
        unplaced.push((segment.kitsu_to, end));
    }
    Some(unplaced)
}

/// Whether `show`'s run, filed as `segment`, lies inside Kitsu's run:
/// every bound [`unplaced_bounds`] leaves outside is vouched for by
/// Kitsu's own air date for its episode falling in the bound's month.
pub(super) fn inside_kitsu_run(evidence: &Evidence, show: &TrackerShow, segment: &Segment) -> bool {
    unplaced_bounds(evidence, show, segment).is_some_and(|unplaced| {
        unplaced
            .iter()
            .all(|(n, bound)| dated_in(evidence, *n, Some(bound)))
    })
}
