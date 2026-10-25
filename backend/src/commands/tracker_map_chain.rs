//! The tracker's own chain of sequels behind the episode map's
//! evidence, read hop by hop. Split from `tracker_map_evidence.rs` so
//! each file stays inside the CRAP gate's per-file bar.

use super::parts_walk::MAX_HOPS;
use super::*;
use crate::account::provider::MediaEvidence;

/// The tracker's single-sequel chain from `entry`, each hop's evidence
/// read and kept only when it is the one series sequel, followed until
/// the counts reach `kitsu_count` or the chain ends.
pub(super) async fn tracker_chain(
    ask: &MapAsk<'_>,
    entry: &MediaEvidence,
    kitsu_count: u32,
) -> Result<Vec<TrackerShow>> {
    let mut chain = Vec::new();
    let mut current = entry.sequels.clone();
    let mut covered = entry.show.episodes.unwrap_or(0);
    for _ in 0..MAX_HOPS {
        if covered >= kitsu_count {
            break;
        }
        let mut series = Vec::new();
        let mut gone = false;
        for id in &current {
            match ask.provider.media_evidence(ask.tokens, *id).await {
                Ok(next) if next.series => series.push(next),
                Ok(_) => {}
                // The tracker answering that a sequel it names is gone is
                // an answer: the hop cannot be settled, so the chain ends
                // here. Any other failure is no answer, and fails the map.
                Err(err) if err.is_not_found_shaped() => gone = true,
                Err(err) => return Err(err),
            }
        }
        if gone || series.len() != 1 {
            break;
        }
        let next = series.remove(0);
        covered = covered.saturating_add(next.show.episodes.unwrap_or(0));
        let more = next.show.episodes.is_some();
        current = next.sequels;
        chain.push(next.show);
        if !more {
            break;
        }
    }
    Ok(chain)
}
