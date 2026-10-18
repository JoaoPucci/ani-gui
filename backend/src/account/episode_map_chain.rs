//! The chain rule: one Kitsu entry spans a chain of the tracker's
//! entries. Split from [`super`] so each file stays inside the CRAP
//! gate's per-file bar.

use std::collections::BTreeSet;

use super::dates::{inside_kitsu_run, unplaced_bounds};
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
    let (segments, shows) = exact_chain(evidence, kitsu_count, tracker_total)?;
    shows
        .iter()
        .zip(&segments)
        .all(|(show, segment)| inside_kitsu_run(evidence, show, segment))
        .then_some(segments)
}

/// The chain's segments with the entry each files, when the counts sum
/// exactly to Kitsu's over at least one hop and no segment past the
/// first is a Kitsu entry's own — the chain rule short of its dates.
fn exact_chain(
    evidence: &Evidence,
    kitsu_count: u32,
    tracker_total: u32,
) -> Option<(Vec<Segment>, Vec<&TrackerShow>)> {
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
    Some((segments, shows))
}

/// The Kitsu episodes whose own air dates could place an exact,
/// unclaimed chain whose every date is known but which the dates alone
/// leave outside Kitsu's run; none otherwise.
pub(super) fn chain_dates_needed(
    evidence: &Evidence,
    kitsu_count: u32,
    tracker_total: u32,
) -> BTreeSet<u32> {
    let Some((segments, shows)) = exact_chain(evidence, kitsu_count, tracker_total) else {
        return BTreeSet::new();
    };
    let mut needed = BTreeSet::new();
    for (show, segment) in shows.iter().zip(&segments) {
        let Some(unplaced) = unplaced_bounds(evidence, show, segment) else {
            return BTreeSet::new();
        };
        needed.extend(unplaced.iter().map(|(n, _)| *n));
    }
    needed
}
