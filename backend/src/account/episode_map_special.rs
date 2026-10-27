//! A special the tracker has no entry for, filed among the extras a
//! related entry counts: Unlimited Blade Works' prologue (Kitsu 9718),
//! which AniList has no entry for, is the "Episode 0" AniList's 19603
//! lists before the TV season's twelve.
//!
//! The rule reads the related entry's own map, built by the other rules
//! from that entry's evidence, so the two never disagree about where the
//! extra is.

use serde::{Deserialize, Serialize};

use super::episode_map::{EpisodeMap, Identity, KitsuShow, Segment, Shape, TrackerShow};

/// One entry Kitsu relates the special to that resolves on the tracker:
/// the Kitsu entry, the tracker entry it resolves to, that entry's map
/// for it, and whether another of its Kitsu relations could be the same
/// extras.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelatedFiling {
    /// The related Kitsu entry.
    pub related: KitsuShow,
    /// The tracker entry it resolves to.
    pub entry: TrackerShow,
    /// The related entry's map on that tracker entry.
    pub map: EpisodeMap,
    /// Whether another Kitsu entry related to it, one the tracker has no
    /// entry for either, counts the special's episodes too.
    pub contested: bool,
}

/// What the rule reads for a special the tracker has no entry for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecialEvidence {
    /// The special, as Kitsu has it.
    pub special: KitsuShow,
    /// Every related Kitsu entry that resolves on the tracker.
    pub related: Vec<RelatedFiling>,
}

/// The special's map, where the evidence files it among a related
/// entry's extras; `None` where it does not.
///
/// It is filed where exactly one related entry holds it: an entry whose
/// own map reads the related season's extras over the whole season
/// from the entry's first episode, with as many extras as the special
/// has episodes, one after another, none of them the entry's last; no
/// other Kitsu entry related to it could be the special; and the
/// special started on a whole, real day inside the entry's run, by
/// month. Its episodes are those extras, in the place the tracker lists
/// them — whatever its own date, which only has to lie in the run — and
/// it never completes the entry.
#[must_use]
pub fn special_extras(evidence: &SpecialEvidence) -> Option<EpisodeMap> {
    let count = evidence.special.episodes.filter(|n| *n > 0)?;
    let start = whole_day(evidence.special.start.as_deref())?;
    let mut held = evidence
        .related
        .iter()
        .filter(|f| !f.contested && in_run(start, &f.entry))
        .filter_map(|f| slots(f, count));
    let segment = held.next()?;
    if held.next().is_some() {
        return None;
    }
    Some(EpisodeMap {
        shape: Shape::SpecialExtras,
        segments: vec![segment],
    })
}

/// The segment filing the special's `count` episodes as the extras
/// `filing`'s map reads, where that map holds them.
fn slots(filing: &RelatedFiling, count: u32) -> Option<Segment> {
    if filing.map.shape != Shape::Extras {
        return None;
    }
    let [season] = filing.map.segments.as_slice() else {
        return None;
    };
    if season.entry != filing.entry.id || season.kitsu_from != 1 || season.lead != 0 {
        return None;
    }
    let after = season.extras_after.first().copied()?;
    if u32::try_from(season.extras_after.len()).ok()? != count
        || season.extras_after.iter().any(|w| *w != after)
    {
        return None;
    }
    // The extras after Kitsu's `after` are the entry's `after + 1` on.
    let last = after.checked_add(count)?;
    if last >= season.entry_total {
        return None;
    }
    Some(Segment {
        entry: season.entry,
        entry_total: season.entry_total,
        entry_finished: season.entry_finished,
        kitsu_from: 1,
        kitsu_to: count,
        lead: after,
        extras_after: Vec::new(),
    })
}

/// `date` when it names a real whole day.
fn whole_day(date: Option<&str>) -> Option<&str> {
    date.filter(|d| super::episode_map_calendar::parse_date(d).is_some_and(|p| p.day.is_some()))
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
    super::episode_map_calendar::parse_date(date)?;
    date.get(..7)
}

/// `existing`, the map the other rules gave the special, together with
/// the special rule's: the special rule's where the others explain
/// nothing; theirs where it explains nothing, or files the special the
/// same way; and a conflict where both explain it differently, or where
/// theirs already is one.
#[must_use]
pub fn with_special(existing: EpisodeMap, special: Option<EpisodeMap>) -> EpisodeMap {
    let Some(special) = special else {
        return existing;
    };
    match existing.shape {
        Shape::Identity(Identity::Conflict) => existing,
        Shape::Identity(_) => special,
        _ if existing.segments == special.segments => existing,
        _ => EpisodeMap::identity(Identity::Conflict),
    }
}

#[cfg(test)]
#[path = "episode_map_special_test.rs"]
mod tests;
