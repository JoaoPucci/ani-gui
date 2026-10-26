//! Which Kitsu entries own a tracker entry, as the episode map's claim
//! and share checks read it: the one ownership test the write's own id
//! resolution answers. Split from `tracker_map_kitsu.rs` so each file
//! stays inside the CRAP gate's per-file bar.

use super::kitsu_side::{kitsu_show_of, site};
use super::*;
use crate::meta::kitsu::{KitsuAnimeRef, KitsuMapped};

/// The Kitsu entries that may own `show` on the asking tracker: those
/// mapped to it by the tracker's own id, and those mapped to it by its
/// id on the other tracker (AniList's recorded MAL id, or every AniList
/// media that records a MAL id), since the write's id resolution reaches a
/// tracker entry either way. Not complete when a page did not hold
/// every mapping Kitsu counts.
async fn candidates(ask: &MapAsk<'_>, show: &TrackerShow) -> Result<KitsuMapped> {
    let mut found = ask
        .state
        .kitsu
        .anime_by_external_id(site(ask.kind)?, show.id)
        .await?;
    let others: Vec<(&str, u32)> = match ask.kind {
        ProviderKind::AniList => show
            .id_mal
            .map(|id| ("myanimelist/anime", id))
            .into_iter()
            .collect(),
        ProviderKind::MyAnimeList => crate::commands::anilist_ids::media_ids_recording_mal(
            ask.state,
            show.id,
            ask.anilist_base,
        )
        .await?
        .into_iter()
        .map(|id| ("anilist/anime", id))
        .collect(),
        ProviderKind::InHouse => Vec::new(),
    };
    for (other_site, id) in others {
        let more = ask.state.kitsu.anime_by_external_id(other_site, id).await?;
        found.complete &= more.complete;
        for anime in more.entries {
            if !found.entries.iter().any(|a| a.id == anime.id) {
                found.entries.push(anime);
            }
        }
    }
    Ok(found)
}

/// Whether Kitsu's `kitsu_id` owns tracker entry `entry`: whether the
/// write's own id resolution
/// ([`crate::commands::account::resolve_native_media_id`]) takes it
/// there. The one ownership test the map's claims and shares read, so
/// they cannot drift from where a write would go.
pub(super) async fn owns(ask: &MapAsk<'_>, kitsu_id: &str, entry: u32) -> Result<bool> {
    let resolved = crate::commands::account::resolve_native_media_id(
        ask.state,
        ask.kind,
        kitsu_id,
        ask.anilist_base,
    )
    .await?;
    Ok(resolved == Some(ProviderMediaId(entry)))
}

/// The Kitsu entries that own `show` ([`owns`]) among its
/// [`candidates`], and whether the candidates were every mapping Kitsu
/// counts.
async fn owners(ask: &MapAsk<'_>, show: &TrackerShow) -> Result<(Vec<KitsuAnimeRef>, bool)> {
    let found = candidates(ask, show).await?;
    let mut owners = Vec::new();
    for anime in found.entries {
        if owns(ask, &anime.id, show.id).await? {
            owners.push(anime);
        }
    }
    Ok((owners, found.complete))
}

/// Whether some Kitsu entry owns `show` — or may, when Kitsu did not
/// list every mapping to it.
pub(super) async fn claimed_on_kitsu(ask: &MapAsk<'_>, show: &TrackerShow) -> Result<bool> {
    let (owners, complete) = owners(ask, show).await?;
    Ok(!owners.is_empty() || !complete)
}

/// Every Kitsu entry that owns `entry`, each once.
pub(super) async fn sharing_on_kitsu(
    ask: &MapAsk<'_>,
    entry: &TrackerShow,
) -> Result<Vec<KitsuShow>> {
    let (owners, complete) = owners(ask, entry).await?;
    // A page that did not hold every mapping Kitsu counts stands for
    // what it left out with one entry it did not list: no id, no count,
    // so nothing sums with it and the show is not the entry's only one.
    if !complete {
        return Ok(vec![KitsuShow::default()]);
    }
    Ok(owners.iter().map(kitsu_show_of).collect())
}
