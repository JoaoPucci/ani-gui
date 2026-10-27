//! The rules that stand against one another where Kitsu's chain of
//! parts and the entries mapped to the tracker's entry disagree. Split
//! from [`super`] so each file stays inside the CRAP gate's per-file
//! bar.

use super::extras_rule::{extras, first_part_extras};
use super::parts_rule::split_parts;
use super::{Evidence, Segment, Shape};

/// Whether Kitsu's chain of parts reaches the entry's total, at least
/// two of them, through parts other than the entries mapped to it: the
/// chain then files the entry's episodes otherwise than the mapped set
/// does, whichever part the show is.
pub(super) fn chain_files_otherwise(evidence: &Evidence, tracker_total: u32) -> bool {
    let mut covered = 0u32;
    let mut taken = Vec::new();
    for part in &evidence.parts {
        if covered >= tracker_total {
            break;
        }
        let Some(count) = part.episodes.filter(|n| *n > 0) else {
            return false;
        };
        // A run past what u32 counts reaches no entry's total.
        let Some(next) = covered.checked_add(count) else {
            return false;
        };
        covered = next;
        taken.push(part.id.as_str());
    }
    let reaches = covered == tracker_total && taken.len() >= 2;
    reaches
        && (taken.len() != evidence.sharing.len()
            || evidence
                .sharing
                .iter()
                .any(|k| !taken.contains(&k.id.as_str())))
}

/// What the split-parts rule explains, with the extras rule as its
/// rival: the chain's placement, or none where a mapped entry the chain
/// does not take contradicts it. A contradicted chain is no explanation
/// of its own, but it still stands as a rival to a shared set that
/// places the show otherwise (`shared`).
pub(super) fn split_explanations(
    evidence: &Evidence,
    kitsu_count: u32,
    tracker_total: u32,
    shared: bool,
) -> Vec<(Shape, Vec<Segment>)> {
    let Some((segments, contradicted)) = split_parts(evidence, tracker_total) else {
        return Vec::new();
    };
    if contradicted {
        return if shared {
            vec![(Shape::SplitParts, segments)]
        } else {
            Vec::new()
        };
    }
    let mut found = vec![(Shape::SplitParts, segments)];
    // The entry's list explaining the surplus over the first part is a
    // rival explanation of the same episodes.
    let ours = evidence.kitsu.id.as_str();
    let rival = if evidence.parts.first().is_some_and(|p| p.id == ours) {
        extras(evidence, kitsu_count, tracker_total)
    } else {
        first_part_extras(evidence, tracker_total)
    };
    if let Some(rival) = rival {
        found.push((Shape::Extras, rival));
    }
    found
}
