//! The evidence the special rule reads
//! ([`crate::account::episode_map_special`]) for a show Kitsu resolves
//! to nothing on the tracker: each entry Kitsu relates it to that
//! resolves there, the tracker's entry, and that entry's own map for it.
//! Split from `tracker_map.rs` so each file stays inside the CRAP gate's
//! per-file bar.

use super::kitsu_side::kitsu_show_of_relation;
use super::*;
use crate::account::episode_map::Shape;
use crate::account::episode_map_special::{RelatedFiling, SpecialEvidence};

/// The special rule's evidence for `ours`, or `None` where the rule has
/// none to read: a tracker that lists no episodes the rule could read
/// (MyAnimeList), a count Kitsu does not give, or a page of relations
/// that did not hold them all. A read that fails fails the map.
pub(super) async fn special_evidence(
    ask: &MapAsk<'_>,
    ours: &KitsuShow,
) -> Result<Option<SpecialEvidence>> {
    if ask.kind != ProviderKind::AniList {
        return Ok(None);
    }
    let Some(count) = ours.episodes.filter(|n| *n > 0) else {
        return Ok(None);
    };
    let relations = crate::commands::kitsu::kitsu_relationships(ask.state, &ours.id).await?;
    if !relations.complete {
        return Ok(None);
    }
    let mut related = Vec::new();
    for relation in relations.entries.iter().filter(|r| r.kind == "anime") {
        let Some(native) = resolves(ask, &relation.id).await? else {
            continue;
        };
        let entry = match ask.provider.media_evidence(ask.tokens, native).await {
            Ok(entry) => entry.show,
            // The tracker answering that the entry is gone is an answer:
            // it holds nothing.
            Err(err) if err.is_not_found_shaped() => continue,
            Err(err) => return Err(err),
        };
        let map = episode_map_for(ask, &relation.id, native, entry.episodes).await?;
        let contested = reads_extras(&map) && contested(ask, &relation.id, ours, count).await?;
        related.push(RelatedFiling {
            related: kitsu_show_of_relation(relation),
            entry,
            map,
            contested,
        });
    }
    Ok(Some(SpecialEvidence {
        special: ours.clone(),
        related,
    }))
}

/// The tracker entry Kitsu's `kitsu_id` resolves to, as the write
/// resolves it.
async fn resolves(ask: &MapAsk<'_>, kitsu_id: &str) -> Result<Option<ProviderMediaId>> {
    crate::commands::account::resolve_native_media_id(
        ask.state,
        ask.kind,
        kitsu_id,
        ask.anilist_base,
    )
    .await
}

/// Whether `map` reads extras the special could be.
fn reads_extras(map: &EpisodeMap) -> bool {
    map.shape == Shape::Extras
}

/// Whether another Kitsu entry related to `related`, of the special's
/// count and with no entry on the tracker either, could be the extras
/// as well — or whether Kitsu's page of `related`'s relations did not
/// hold them all, so one could be past it.
async fn contested(ask: &MapAsk<'_>, related: &str, ours: &KitsuShow, count: u32) -> Result<bool> {
    let relations = crate::commands::kitsu::kitsu_relationships(ask.state, related).await?;
    if !relations.complete {
        return Ok(true);
    }
    for other in relations
        .entries
        .iter()
        .filter(|r| r.kind == "anime" && r.id != ours.id && r.episode_count == Some(count))
    {
        if resolves(ask, &other.id).await?.is_none() {
            return Ok(true);
        }
    }
    Ok(false)
}
