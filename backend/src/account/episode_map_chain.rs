//! The chain rule: one Kitsu entry spans a chain of the tracker's
//! entries. Split from [`super`] so each file stays inside the CRAP
//! gate's per-file bar.

use crate::account::episode_map::{Evidence, Segment, TrackerShow};

/// A segment filing Kitsu's `from..=to` under the whole of `show`.
fn whole_entry(show: &TrackerShow, total: u32, from: u32, to: u32) -> Segment {
    Segment {
        entry: show.id,
        entry_total: total,
        entry_finished: show.finished,
        kitsu_from: from,
        kitsu_to: to,
        lead: 0,
        extras_after: Vec::new(),
    }
}

/// `YYYY-MM` of a date, `None` when the date is unknown or is not a
/// real calendar date or month
/// ([`crate::account::episode_map_calendar::parse_date`]). Runs are compared by month: the sources disagree by a day
/// about the same broadcast often enough that a day would refuse
/// chains that are plainly right.
fn month(date: Option<&str>) -> Option<&str> {
    let date = date?;
    crate::account::episode_map_calendar::parse_date(date)?;
    date.get(..7)
}

/// The chain rule: `entry` and its sequels, in the order the tracker
/// chains them, sum exactly to Kitsu's count, every segment's run lies
/// inside Kitsu's run by month, and no segment past the first is a
/// Kitsu entry's own. Each segment then takes the Kitsu episodes it
/// counts, in order.
pub(super) fn chain(
    evidence: &Evidence,
    kitsu_count: u32,
    tracker_total: u32,
) -> Option<Vec<Segment>> {
    let mut segments = vec![whole_entry(
        &evidence.entry,
        tracker_total,
        1,
        tracker_total,
    )];
    let mut shows = vec![&evidence.entry];
    let mut covered = tracker_total;
    for hop in &evidence.chain {
        if covered >= kitsu_count {
            break;
        }
        let count = hop.episodes.filter(|n| *n > 0)?;
        segments.push(whole_entry(hop, count, covered + 1, covered + count));
        shows.push(hop);
        covered += count;
    }
    if covered != kitsu_count || segments.len() < 2 {
        return None;
    }
    if segments[1..]
        .iter()
        .any(|s| evidence.claimed.contains(&s.entry))
    {
        return None;
    }
    let kitsu_start = month(evidence.kitsu.start.as_deref())?;
    let kitsu_end = month(evidence.kitsu.end.as_deref())?;
    for show in shows {
        let start = month(show.start.as_deref())?;
        let end = month(show.end.as_deref())?;
        if start < kitsu_start || end > kitsu_end {
            return None;
        }
    }
    Some(segments)
}
