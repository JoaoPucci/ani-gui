//! The shared-entry and split-parts rules: several Kitsu parts filed
//! under one tracker entry. Split from [`super`] so each file stays
//! inside the CRAP gate's per-file bar.

use crate::account::episode_map::{Evidence, KitsuShow, Segment, TrackerShow};

/// The segment filing the whole of `part` — Kitsu's `1..=count` — under
/// `entry`, after `lead` of the entry's episodes.
pub(super) fn part_of_entry(entry: &TrackerShow, total: u32, count: u32, lead: u32) -> Segment {
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
/// entry, in the order of their distinct start dates (each a real,
/// full calendar date), sums exactly to
/// its total, the show among them. The show's episodes are the entry's
/// after the parts before it; only the last part's last episode is the
/// entry's.
pub(super) fn shared_entry(evidence: &Evidence, tracker_total: u32) -> Option<Vec<Segment>> {
    let mut parts: Vec<&KitsuShow> = evidence.sharing.iter().collect();
    // Only a real, full calendar date orders a part: `YYYY-MM-DD` strings
    // that `parse_date` accepts sort as the days they name.
    let full_date = |p: &&KitsuShow| {
        p.start
            .as_deref()
            .and_then(crate::account::episode_map_calendar::parse_date)
            .is_some_and(|d| d.day.is_some())
    };
    if !parts.iter().all(full_date) {
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
pub(super) fn split_parts(evidence: &Evidence, tracker_total: u32) -> Option<Vec<Segment>> {
    let parts: Vec<&KitsuShow> = evidence.parts.iter().collect();
    let (lead, count) = place_among(&parts, &evidence.kitsu.id, tracker_total)?;
    Some(vec![part_of_entry(
        &evidence.entry,
        tracker_total,
        count,
        lead,
    )])
}
