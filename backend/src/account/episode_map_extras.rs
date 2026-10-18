//! The extras rule: episodes the tracker counts that Kitsu does not
//! list. Split from [`super`] so each file stays inside the CRAP gate's
//! per-file bar.

use super::parts_rule::part_of_entry;
use crate::account::episode_map::{Evidence, Segment};

/// A number from the tracker's episode list as its whole part and its
/// fraction's digits: `"7"` is `(7, None)`, `"6.5"` is `(6, Some("5"))`,
/// `"0"` is `(0, None)`. Anything else — a title with no number, an
/// empty string, letters — is `None` and ignored.
pub(super) fn listed_number(text: &str) -> Option<(u32, Option<&str>)> {
    let (whole, fraction) = match text.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (text, None),
    };
    if whole.is_empty() || !whole.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if fraction.is_some_and(|f| f.is_empty() || !f.chars().all(|c| c.is_ascii_digit())) {
        return None;
    }
    Some((whole.parse().ok()?, fraction))
}

/// The extras rule: the tracker's own episode list carries exactly as
/// many extras — episodes numbered 0, or with a fraction, each listed
/// once however many tracks carry it — as the entry's surplus over
/// Kitsu's count, every one of them inside Kitsu's run. Each extra then
/// sits after the whole Kitsu episode its number names (before the
/// first, for 0), and the episodes after it are counted one further on.
pub(super) fn extras(
    evidence: &Evidence,
    kitsu_count: u32,
    tracker_total: u32,
) -> Option<Vec<Segment>> {
    let listed = evidence.listed.as_ref()?;
    let mut extras: Vec<(u32, Option<&str>)> = listed
        .iter()
        .filter_map(|n| listed_number(n))
        .filter(|(whole, fraction)| *whole == 0 || fraction.is_some())
        .collect();
    extras.sort_unstable();
    extras.dedup();
    let surplus = tracker_total - kitsu_count;
    if u32::try_from(extras.len()).ok()? != surplus {
        return None;
    }
    if extras.iter().any(|(whole, _)| *whole > kitsu_count) {
        return None;
    }
    Some(vec![Segment {
        extras_after: extras.iter().map(|(whole, _)| *whole).collect(),
        ..part_of_entry(&evidence.entry, tracker_total, kitsu_count, 0)
    }])
}

/// The extras rule read from the first of the Kitsu parts, when the
/// show being mapped is a later one.
pub(super) fn first_part_extras(evidence: &Evidence, tracker_total: u32) -> Option<Vec<Segment>> {
    let first = evidence.parts.first()?;
    if first.id == evidence.kitsu.id {
        return None;
    }
    let count = first.episodes.filter(|n| *n > 0 && *n < tracker_total)?;
    extras(evidence, count, tracker_total)
}
