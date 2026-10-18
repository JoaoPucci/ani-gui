//! The Kitsu reads behind the episode map's evidence: which Kitsu
//! entries map to a tracker entry, and the one sequel or prequel Kitsu
//! relates an entry to. Split from `tracker_map_evidence.rs` so each
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

/// Whether some Kitsu entry maps to `show` on `kind` — by the tracker's
/// own id, and for AniList by the MAL id it records as well, since
/// Kitsu reaches AniList through MAL's id for most shows.
pub(super) async fn claimed_on_kitsu(
    state: &std::sync::Arc<AppState>,
    kind: ProviderKind,
    show: &TrackerShow,
) -> Result<bool> {
    if !state
        .kitsu
        .anime_by_external_id(site(kind)?, show.id)
        .await?
        .entries
        .is_empty()
    {
        return Ok(true);
    }
    if kind == ProviderKind::AniList {
        if let Some(id_mal) = show.id_mal {
            return Ok(!state
                .kitsu
                .anime_by_external_id("myanimelist/anime", id_mal)
                .await?
                .entries
                .is_empty());
        }
    }
    Ok(false)
}

/// Every Kitsu entry mapped to `entry` on `kind`, each once.
pub(super) async fn sharing_on_kitsu(
    state: &std::sync::Arc<AppState>,
    kind: ProviderKind,
    entry: &TrackerShow,
) -> Result<Vec<KitsuShow>> {
    let first = state
        .kitsu
        .anime_by_external_id(site(kind)?, entry.id)
        .await?;
    // A page that did not hold every mapping Kitsu counts says nothing
    // the shared-entry rule can sum.
    if !first.complete {
        return Ok(Vec::new());
    }
    let mut found = first.entries;
    if kind == ProviderKind::AniList {
        if let Some(id_mal) = entry.id_mal {
            let by_mal = state
                .kitsu
                .anime_by_external_id("myanimelist/anime", id_mal)
                .await?;
            if !by_mal.complete {
                return Ok(Vec::new());
            }
            for anime in by_mal.entries {
                if !found.iter().any(|a| a.id == anime.id) {
                    found.push(anime);
                }
            }
        }
    }
    Ok(found.iter().map(kitsu_show_of).collect())
}

/// A related Kitsu anime as the map reads it.
fn kitsu_show_of_relation(relation: &KitsuRelation) -> KitsuShow {
    KitsuShow {
        id: relation.id.clone(),
        episodes: relation.episode_count,
        start: None,
        end: None,
    }
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
