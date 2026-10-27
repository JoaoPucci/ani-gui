//! The rules that build an [`EpisodeMap`] from [`Evidence`].
//!
//! Each rule explains one way a tracker's count can differ from Kitsu's,
//! and fires only on evidence that settles it; the map falls back to
//! identity — every episode sent as it is — whenever nothing conclusive
//! explains the difference, since a wrong mapping is worse than none.
//! `docs/title-resolution.md` ("Episode numbers") tables the cases.

use super::episode_map::{EpisodeMap, Evidence, Identity, Segment, Shape};

#[path = "episode_map_chain.rs"]
mod chain_rule;
#[path = "episode_map_dates.rs"]
mod dates;
#[path = "episode_map_days.rs"]
mod days;
#[path = "episode_map_extras.rs"]
mod extras_rule;
#[path = "episode_map_order.rs"]
mod order;
#[path = "episode_map_parts.rs"]
mod parts_rule;
#[path = "episode_map_rivals.rs"]
mod rivals;

use chain_rule::chain;
use extras_rule::extras;
#[cfg(test)]
use extras_rule::listed_number;
use parts_rule::shared_entry;
use rivals::{chain_files_otherwise, split_explanations};

/// The Kitsu episodes whose own air dates could place an exact,
/// unclaimed chain the dates alone leave outside Kitsu's run — what the
/// gatherer reads Kitsu's episode list for; none for any other evidence.
#[must_use]
pub fn chain_dates_needed(evidence: &Evidence) -> std::collections::BTreeSet<u32> {
    match (
        evidence.kitsu.episodes.filter(|n| *n > 0),
        evidence.entry.episodes.filter(|n| *n > 0),
    ) {
        (Some(count), Some(total)) if total < count => {
            chain_rule::chain_dates_needed(evidence, count, total)
        }
        _ => std::collections::BTreeSet::new(),
    }
}

/// The map for `evidence`.
///
/// The counts decide which rules can apply: a tracker entry shorter
/// than Kitsu's show may be the first of a chain; a longer one may hold
/// several Kitsu parts or count episodes Kitsu does not list. Exactly
/// one rule has to fire — two explanations of the same difference are
/// no explanation — and none leaves the map identity.
#[must_use]
pub fn map_show(evidence: &Evidence) -> EpisodeMap {
    // A part that aired before the part it follows is no later part: the
    // chain of parts ends before it.
    let in_order = order::parts_in_airing_order(&evidence.parts);
    let ordered;
    let evidence = if in_order < evidence.parts.len() {
        ordered = Evidence {
            parts: evidence.parts[..in_order].to_vec(),
            ..evidence.clone()
        };
        &ordered
    } else {
        evidence
    };
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
        let shared = shared_entry(evidence, tracker_total);
        if shared.is_some() && chain_files_otherwise(evidence, tracker_total) {
            return EpisodeMap::identity(Identity::Conflict);
        }
        if let Some(segments) = &shared {
            explanations.push((Shape::SharedEntry, segments.clone()));
        }
        // The extras rule reads a surplus over the show's own count, so it
        // explains the entry only where no other Kitsu entry is filed
        // under it; beside Kitsu parts it is only ever a rival to them.
        let ours = evidence.kitsu.id.as_str();
        let sole = evidence
            .sharing
            .iter()
            .chain(&evidence.parts)
            .all(|k| k.id == ours);
        explanations.extend(split_explanations(
            evidence,
            kitsu_count,
            tracker_total,
            shared.is_some(),
        ));
        if sole {
            if let Some(segments) = extras(evidence, kitsu_count, tracker_total) {
                explanations.push((Shape::Extras, segments));
            }
        }
    }
    // Two rules filing the episodes the same way are one explanation.
    explanations.dedup_by(|b, a| a.1 == b.1);
    match explanations.len() {
        0 => EpisodeMap::identity(Identity::Unexplained),
        1 => {
            let (shape, segments) = explanations.remove(0);
            EpisodeMap { shape, segments }
        }
        _ => EpisodeMap::identity(Identity::Conflict),
    }
}

#[cfg(test)]
#[path = "episode_map_test.rs"]
mod tests;

#[cfg(test)]
#[path = "episode_map_prop_test.rs"]
mod prop_tests;

#[cfg(test)]
#[path = "episode_map_fixture_test.rs"]
mod fixture_tests;

#[cfg(test)]
#[path = "episode_map_oracle_test.rs"]
mod oracle;
