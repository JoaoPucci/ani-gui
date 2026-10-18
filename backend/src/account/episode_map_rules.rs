//! The rules that build an [`EpisodeMap`] from [`Evidence`].
//!
//! Each rule explains one way a tracker's count can differ from Kitsu's,
//! and fires only on evidence that settles it; the map falls back to
//! identity — every episode sent as it is — whenever nothing conclusive
//! explains the difference, since a wrong mapping is worse than none.
//! `docs/title-resolution.md` ("Episode numbers") tables the cases.

use super::episode_map::{EpisodeMap, Evidence, Identity, KitsuShow, Segment, Shape, TrackerShow};

/// The map for `evidence`.
///
/// The counts decide which rules can apply: a tracker entry shorter
/// than Kitsu's show may be the first of a chain; a longer one may hold
/// several Kitsu parts or count episodes Kitsu does not list. Exactly
/// one rule has to fire — two explanations of the same difference are
/// no explanation — and none leaves the map identity.
#[must_use]
pub fn map_show(evidence: &Evidence) -> EpisodeMap {
    let (Some(kitsu_count), Some(tracker_total)) = (
        evidence.kitsu.episodes.filter(|n| *n > 0),
        evidence.entry.episodes.filter(|n| *n > 0),
    ) else {
        return EpisodeMap::identity(Identity::CountUnknown);
    };
    if kitsu_count == tracker_total {
        return EpisodeMap::identity(Identity::CountsAgree);
    }
    let mut explanations: Vec<(Shape, Vec<Segment>)> = Vec::new();
    if tracker_total < kitsu_count {
        if let Some(segments) = chain(evidence, kitsu_count, tracker_total) {
            explanations.push((Shape::Chain, segments));
        }
    } else {
        if let Some(segments) = shared_entry(evidence, tracker_total) {
            explanations.push((Shape::SharedEntry, segments));
        }
        if let Some(segments) = split_parts(evidence, tracker_total) {
            explanations.push((Shape::SplitParts, segments));
        }
    }
    match explanations.len() {
        0 => EpisodeMap::identity(Identity::Unexplained),
        1 => {
            let (shape, segments) = explanations.remove(0);
            EpisodeMap { shape, segments }
        }
        _ => EpisodeMap::identity(Identity::Conflict),
    }
}

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

/// `YYYY-MM` of a date, `None` when the date is unknown or names no
/// month. Runs are compared by month: the sources disagree by a day
/// about the same broadcast often enough that a day would refuse
/// chains that are plainly right.
fn month(date: Option<&str>) -> Option<&str> {
    let date = date?;
    let month = date.get(..7)?;
    let (year, mm) = month.split_at(4);
    (year.chars().all(|c| c.is_ascii_digit())
        && mm.len() == 3
        && mm.starts_with('-')
        && mm[1..].chars().all(|c| c.is_ascii_digit()))
    .then_some(month)
}

/// The chain rule: `entry` and its sequels, in the order the tracker
/// chains them, sum exactly to Kitsu's count, every segment's run lies
/// inside Kitsu's run by month, and no segment past the first is a
/// Kitsu entry's own. Each segment then takes the Kitsu episodes it
/// counts, in order.
fn chain(evidence: &Evidence, kitsu_count: u32, tracker_total: u32) -> Option<Vec<Segment>> {
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

/// The segment filing the whole of `part` — Kitsu's `1..=count` — under
/// `entry`, after `lead` of the entry's episodes.
fn part_of_entry(entry: &TrackerShow, total: u32, count: u32, lead: u32) -> Segment {
    Segment {
        entry: entry.id,
        entry_total: total,
        entry_finished: entry.finished,
        kitsu_from: 1,
        kitsu_to: count,
        lead,
        extras_after: Vec::new(),
    }
}

/// Where the show being mapped sits in `parts`, given in order, when
/// the parts up to some one of them sum exactly to the entry's total
/// and the show is among those: the episodes before its, and its count.
fn place_among(parts: &[&KitsuShow], ours: &str, tracker_total: u32) -> Option<(u32, u32)> {
    let mut covered = 0u32;
    let mut place = None;
    let mut taken = 0usize;
    for part in parts {
        if covered >= tracker_total {
            break;
        }
        let count = part.episodes.filter(|n| *n > 0)?;
        if part.id == ours {
            place = Some((covered, count));
        }
        covered = covered.checked_add(count)?;
        taken += 1;
    }
    (covered == tracker_total && taken >= 2)
        .then_some(place)
        .flatten()
}

/// The shared-entry rule: every Kitsu entry mapped to the tracker's
/// entry, in the order of their distinct start dates, sums exactly to
/// its total, the show among them. The show's episodes are the entry's
/// after the parts before it; only the last part's last episode is the
/// entry's.
fn shared_entry(evidence: &Evidence, tracker_total: u32) -> Option<Vec<Segment>> {
    let mut parts: Vec<&KitsuShow> = evidence.sharing.iter().collect();
    if parts.iter().any(|p| p.start.is_none()) {
        return None;
    }
    parts.sort_by(|a, b| a.start.cmp(&b.start));
    if parts.windows(2).any(|w| w[0].start == w[1].start) {
        return None;
    }
    let (lead, count) = place_among(&parts, &evidence.kitsu.id, tracker_total)?;
    Some(vec![part_of_entry(
        &evidence.entry,
        tracker_total,
        count,
        lead,
    )])
}

/// The split-parts rule: Kitsu's chain of parts from the entry the
/// tracker knows — each the single sequel of the one before, with no
/// entry of its own on the tracker — sums exactly to the entry's total
/// as far as the sum reaches, the show among those parts. The chain's
/// order is Kitsu's own.
fn split_parts(evidence: &Evidence, tracker_total: u32) -> Option<Vec<Segment>> {
    let parts: Vec<&KitsuShow> = evidence.parts.iter().collect();
    let (lead, count) = place_among(&parts, &evidence.kitsu.id, tracker_total)?;
    Some(vec![part_of_entry(
        &evidence.entry,
        tracker_total,
        count,
        lead,
    )])
}

#[cfg(test)]
#[path = "episode_map_test.rs"]
mod tests;
