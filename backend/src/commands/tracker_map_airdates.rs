//! Kitsu's own air dates for an exact chain's segment bounds — the
//! dates that let the chain rule place a segment Kitsu's start and end
//! dates leave out. Split from `tracker_map_evidence.rs` so each file
//! stays inside the CRAP gate's per-file bar.

use std::collections::{BTreeMap, BTreeSet};

use super::*;

/// The episodes a page of Kitsu's episode list holds, as the detail
/// page reads it.
const KITSU_EPISODES_PAGE: u32 = 20;

/// The Kitsu episode numbers that open and close each segment of the
/// chain `entry_total` then `chain`, when those counts sum exactly to
/// `kitsu_count`; empty otherwise, since no chain rule reads them then.
pub(super) fn segment_bounds(
    kitsu_count: u32,
    entry_total: u32,
    chain: &[TrackerShow],
) -> BTreeSet<u32> {
    let mut bounds = BTreeSet::from([1, entry_total]);
    let mut covered = entry_total;
    for hop in chain {
        if covered >= kitsu_count {
            break;
        }
        let Some(count) = hop.episodes.filter(|n| *n > 0) else {
            return BTreeSet::new();
        };
        bounds.insert(covered + 1);
        covered += count;
        bounds.insert(covered);
    }
    if covered == kitsu_count && bounds.len() > 2 {
        bounds
    } else {
        BTreeSet::new()
    }
}

/// The air dates Kitsu's episode list gives `numbers`, read from the
/// pages that hold them — through the episode pages' cache.
pub(super) async fn kitsu_airdates(
    ask: &MapAsk<'_>,
    kitsu_id: &str,
    numbers: &BTreeSet<u32>,
) -> Result<BTreeMap<u32, String>> {
    let pages: BTreeSet<u32> = numbers
        .iter()
        .map(|n| (n.saturating_sub(1)) / KITSU_EPISODES_PAGE + 1)
        .collect();
    let mut dates = BTreeMap::new();
    for page in pages {
        for episode in crate::commands::kitsu::kitsu_episodes(ask.state, kitsu_id, page).await? {
            if let (Some(n), Some(aired)) = (episode.number, episode.airdate) {
                if numbers.contains(&n) {
                    dates.insert(n, aired);
                }
            }
        }
    }
    Ok(dates)
}
