//! The walks that gather an [`Evidence`] for the episode map, one
//! request per thing the rules read and none they do not.

use super::airdates::{kitsu_airdates, segment_bounds};
use super::kitsu_side::{claimed_on_kitsu, sharing_on_kitsu};
use super::parts_walk::{first_part_under, kitsu_parts, MAX_HOPS};
use super::*;
use crate::account::episode_map::Shape;
use crate::account::provider::MediaEvidence;

/// The tracker's single-sequel chain from `entry`, each hop's evidence
/// read and kept only when it is the one series sequel, followed until
/// the counts reach `kitsu_count` or the chain ends.
async fn tracker_chain(
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
        for id in &current {
            let next = ask.provider.media_evidence(ask.tokens, *id).await?;
            if next.series {
                series.push(next);
            }
        }
        if series.len() != 1 {
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

/// The evidence for `kitsu`, which resolves to `native`, whose total
/// differs from Kitsu's count: the tracker's chain for a shorter entry,
/// the Kitsu parts and the tracker's list for a longer one.
pub(super) async fn gather(
    ask: &MapAsk<'_>,
    kitsu: KitsuShow,
    native: ProviderMediaId,
) -> Result<Evidence> {
    let entry = ask.provider.media_evidence(ask.tokens, native).await?;
    let (Some(count), Some(total)) = (kitsu.episodes, entry.show.episodes) else {
        return Ok(Evidence {
            kitsu,
            entry: entry.show,
            ..Default::default()
        });
    };
    if total < count {
        let chain = tracker_chain(ask, &entry, count).await?;
        let mut claimed = Vec::new();
        for show in &chain {
            if claimed_on_kitsu(ask.state, ask.kind, show).await? {
                claimed.push(show.id);
            }
        }
        let mut evidence = Evidence {
            kitsu,
            entry: entry.show,
            chain,
            claimed,
            ..Default::default()
        };
        // Kitsu's own air dates are read only for an exact chain the rule
        // cannot place by Kitsu's start and end dates alone.
        let bounds = segment_bounds(count, total, &evidence.chain);
        if !bounds.is_empty() && map_show(&evidence).shape != Shape::Chain {
            evidence.kitsu_airdates = kitsu_airdates(ask, &evidence.kitsu.id, &bounds).await?;
        }
        return Ok(evidence);
    }
    let root = first_part_under(ask, &kitsu, native).await?;
    gather_longer(ask, kitsu, root, native, entry).await
}

/// The evidence for `ours` filed under the tracker entry `native`, which
/// Kitsu's `root` resolves to: the entry itself, the Kitsu entries
/// mapped to it, Kitsu's chain of parts from `root`, and the tracker's
/// own episode list.
pub(super) async fn gather_under(
    ask: &MapAsk<'_>,
    ours: KitsuShow,
    root: KitsuShow,
    native: ProviderMediaId,
) -> Result<Evidence> {
    let entry = ask.provider.media_evidence(ask.tokens, native).await?;
    gather_longer(ask, ours, root, native, entry).await
}

/// [`gather_under`] with the entry already read.
async fn gather_longer(
    ask: &MapAsk<'_>,
    ours: KitsuShow,
    root: KitsuShow,
    native: ProviderMediaId,
    entry: MediaEvidence,
) -> Result<Evidence> {
    let sharing = sharing_on_kitsu(ask.state, ask.kind, &entry.show).await?;
    let parts = match entry.show.episodes {
        Some(total) => kitsu_parts(ask, root, native, total).await?,
        None => vec![root],
    };
    Ok(Evidence {
        kitsu: ours,
        entry: entry.show,
        listed: entry.listed,
        sharing,
        parts,
        ..Default::default()
    })
}
