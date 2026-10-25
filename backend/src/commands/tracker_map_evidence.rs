//! The walks that gather an [`Evidence`] for the episode map, one
//! request per thing the rules read and none they do not.

use super::chain_walk::tracker_chain;
use super::kitsu_side::{claimed_on_kitsu, sharing_on_kitsu};
use super::parts_walk::{file_mapped_past, kitsu_parts};
use super::prequel_walk::first_part_under;
use super::*;
use crate::account::provider::MediaEvidence;

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
        return Ok(Evidence {
            kitsu,
            entry: entry.show,
            chain,
            claimed,
            ..Default::default()
        });
    }
    let root = first_part_under(ask, &kitsu, native).await?;
    gather_longer(ask, kitsu, root, native, entry).await
}

/// The evidence for `ours` filed under the tracker entry `native`, which
/// Kitsu's `root` resolves to: the entry itself, the Kitsu entries
/// mapped to it, Kitsu's chain of parts from `root`, and the tracker's
/// own episode list. `None` when the tracker answers that the entry is
/// gone: Kitsu can map a prequel to an id the tracker no longer serves,
/// and that answer files the show under nothing.
pub(super) async fn gather_under(
    ask: &MapAsk<'_>,
    ours: KitsuShow,
    root: KitsuShow,
    native: ProviderMediaId,
) -> Result<Option<Evidence>> {
    let entry = match ask.provider.media_evidence(ask.tokens, native).await {
        Ok(entry) => entry,
        Err(err) if err.is_not_found_shaped() => return Ok(None),
        Err(err) => return Err(err),
    };
    gather_longer(ask, ours, root, native, entry)
        .await
        .map(Some)
}

/// [`gather_under`] with the entry already read.
async fn gather_longer(
    ask: &MapAsk<'_>,
    ours: KitsuShow,
    root: KitsuShow,
    native: ProviderMediaId,
    entry: MediaEvidence,
) -> Result<Evidence> {
    let mut sharing = sharing_on_kitsu(ask.state, ask.kind, &entry.show).await?;
    let parts = match entry.show.episodes {
        Some(total) => {
            let parts = kitsu_parts(ask, root, native, total).await?;
            file_mapped_past(ask, &parts, native, total, &mut sharing).await?;
            parts
        }
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
