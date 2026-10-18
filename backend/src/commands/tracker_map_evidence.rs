//! The walks that gather an [`Evidence`] for the episode map, one
//! request per thing the rules read and none they do not.

use super::*;
use crate::account::provider::MediaEvidence;
use crate::meta::kitsu::{KitsuAnimeRef, KitsuRelation};

/// How many sequel hops a chain is followed, on the tracker or on
/// Kitsu. The longest real chain found across Kitsu's catalogue has
/// six segments; a chain still short of the count after this many is
/// not one the rules can settle.
const MAX_HOPS: usize = 6;

/// How many prequels an unresolved show is walked up through to find
/// the entry it may be filed under.
const MAX_PREQUEL_HOPS: usize = 3;

/// Kitsu's name for `kind`'s mapping site.
fn site(kind: ProviderKind) -> Result<&'static str> {
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
async fn claimed_on_kitsu(
    state: &std::sync::Arc<AppState>,
    kind: ProviderKind,
    show: &TrackerShow,
) -> Result<bool> {
    if !state
        .kitsu
        .anime_by_external_id(site(kind)?, show.id)
        .await?
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
                .is_empty());
        }
    }
    Ok(false)
}

/// Every Kitsu entry mapped to `entry` on `kind`, each once.
async fn sharing_on_kitsu(
    state: &std::sync::Arc<AppState>,
    kind: ProviderKind,
    entry: &TrackerShow,
) -> Result<Vec<KitsuShow>> {
    let mut found = state
        .kitsu
        .anime_by_external_id(site(kind)?, entry.id)
        .await?;
    if kind == ProviderKind::AniList {
        if let Some(id_mal) = entry.id_mal {
            for anime in state
                .kitsu
                .anime_by_external_id("myanimelist/anime", id_mal)
                .await?
            {
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
async fn single_relation(
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

/// Kitsu's chain of parts from `root`: each the single sequel of the
/// one before, resolving on the tracker to nothing or to `native`
/// itself — never to an entry of its own — followed until the counts
/// reach `tracker_total` or the chain ends.
async fn kitsu_parts(
    ask: &MapAsk<'_>,
    root: KitsuShow,
    native: ProviderMediaId,
    tracker_total: u32,
) -> Result<Vec<KitsuShow>> {
    let mut covered = root.episodes.unwrap_or(0);
    let mut parts = vec![root];
    for _ in 0..MAX_HOPS {
        if covered >= tracker_total {
            break;
        }
        let last = parts.last().map(|p| p.id.clone()).unwrap_or_default();
        let Some(sequel) = single_relation(ask.state, &last, "sequel").await? else {
            break;
        };
        let own_entry = crate::commands::account::resolve_native_media_id(
            ask.state,
            ask.kind,
            &sequel.id,
            ask.anilist_base,
        )
        .await?;
        if own_entry.is_some_and(|own| own != native) {
            break;
        }
        let part = sequel;
        covered = covered.saturating_add(part.episodes.unwrap_or(0));
        let more = part.episodes.is_some();
        parts.push(part);
        if !more {
            break;
        }
    }
    Ok(parts)
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

/// The first Kitsu part filed under `native`: `kitsu` itself, or the
/// earliest of its single prequels, up the chain, that resolves to
/// `native` too.
async fn first_part_under(
    ask: &MapAsk<'_>,
    kitsu: &KitsuShow,
    native: ProviderMediaId,
) -> Result<KitsuShow> {
    let mut root = kitsu.clone();
    for _ in 0..MAX_PREQUEL_HOPS {
        let Some(prequel) = single_relation(ask.state, &root.id, "prequel").await? else {
            break;
        };
        let resolves = crate::commands::account::resolve_native_media_id(
            ask.state,
            ask.kind,
            &prequel.id,
            ask.anilist_base,
        )
        .await?;
        if resolves != Some(native) {
            break;
        }
        root = prequel;
    }
    Ok(root)
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

/// The first Kitsu entry up the single-prequel chain from `kitsu_id`
/// that resolves on the tracker, with the entry it resolves to.
pub(super) async fn root_through_prequels(
    ask: &MapAsk<'_>,
    kitsu_id: &str,
) -> Result<Option<(KitsuShow, ProviderMediaId)>> {
    let mut current = kitsu_id.to_owned();
    for _ in 0..MAX_PREQUEL_HOPS {
        let Some(prequel) = single_relation(ask.state, &current, "prequel").await? else {
            return Ok(None);
        };
        let show = prequel;
        if let Some(native) = crate::commands::account::resolve_native_media_id(
            ask.state,
            ask.kind,
            &show.id,
            ask.anilist_base,
        )
        .await?
        {
            return Ok(Some((show, native)));
        }
        current = show.id;
    }
    Ok(None)
}
