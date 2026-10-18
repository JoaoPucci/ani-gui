//! The chain rule: one Kitsu entry spans a chain of the tracker's
//! entries. Split from [`super`] so each file stays inside the CRAP
//! gate's per-file bar.

use super::dates::inside_kitsu_run;
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

/// The chain rule: `entry` and its sequels, in the order the tracker
/// chains them, sum exactly to Kitsu's count, every segment's run lies
/// inside Kitsu's run ([`inside_kitsu_run`]), and no segment past the first is a
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
    shows
        .iter()
        .zip(&segments)
        .all(|(show, segment)| inside_kitsu_run(evidence, show, segment))
        .then_some(segments)
}
