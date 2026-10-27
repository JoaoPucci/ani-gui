//! The Kitsu reads behind the episode map's evidence: how a Kitsu entry
//! reads as the map's, and the one sequel or prequel Kitsu relates an
//! entry to. Which Kitsu entries own a tracker entry is in
//! `tracker_map_owners.rs`. Split from `tracker_map_evidence.rs` so each
//! file stays inside the CRAP gate's per-file bar.

use super::*;
use crate::meta::kitsu::{KitsuAnimeRef, KitsuRelation};

/// Kitsu's name for `kind`'s mapping site.
pub(super) fn site(kind: ProviderKind) -> Result<&'static str> {
    match kind {
        ProviderKind::MyAnimeList => Ok("myanimelist/anime"),
        ProviderKind::AniList => Ok("anilist/anime"),
        ProviderKind::InHouse => Err(crate::error::AniError::Metadata),
    }
}

/// A Kitsu anime as the map reads it.
pub(super) fn kitsu_show_of(anime: &KitsuAnimeRef) -> KitsuShow {
    KitsuShow {
        id: anime.id.clone(),
        episodes: anime.episode_count,
        start: anime.start_date.clone(),
        end: anime.end_date.clone(),
    }
}

/// A related Kitsu anime as the map reads it: its count and the day it
/// started, which orders it among Kitsu's parts.
pub(super) fn kitsu_show_of_relation(relation: &KitsuRelation) -> KitsuShow {
    KitsuShow {
        id: relation.id.clone(),
        episodes: relation.episode_count,
        start: relation.start.clone(),
        end: None,
    }
}

/// A related season as the special rule reads it. Where its map reads
/// extras the special could take (`holds_slot`), the season as its own
/// Kitsu entry has it, end included, since an epilogue's slot is dated
/// by the season's last episode; its start is the relation's where the
/// entry gives none. Elsewhere the relation's own fields, which say all
/// the rule reads of a season holding no slot, at no read.
pub(super) async fn related_season(
    ask: &MapAsk<'_>,
    relation: &KitsuRelation,
    holds_slot: bool,
) -> Result<KitsuShow> {
    if !holds_slot {
        return Ok(kitsu_show_of_relation(relation));
    }
    let season = kitsu_show(ask, &relation.id).await?;
    Ok(KitsuShow {
        start: season.start.or_else(|| relation.start.clone()),
        ..season
    })
}

/// The one anime Kitsu relates `kitsu_id` to in `role`, or `None` when
/// it relates none or several — or when the page read did not hold all
/// of its relations, since a second one could be past it.
pub(super) async fn single_relation(
    state: &std::sync::Arc<AppState>,
    kitsu_id: &str,
    role: &str,
) -> Result<Option<KitsuShow>> {
    let relations = crate::commands::kitsu::kitsu_relationships(state, kitsu_id).await?;
    if !relations.complete {
        return Ok(None);
    }
    let found: Vec<&KitsuRelation> = relations
        .entries
        .iter()
        .filter(|r| r.role == role && r.kind == "anime")
        .collect();
    Ok(match found.as_slice() {
        [one] => Some(kitsu_show_of_relation(one)),
        _ => None,
    })
}
