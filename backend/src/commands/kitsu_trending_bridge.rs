//! The Trending row's bridge from AniList's entries to Kitsu's. Split
//! from [`super`] so each file stays inside the CRAP gate's per-file
//! bar.

use super::*;

/// Concurrent AniList → Kitsu lookups for a list of AniList refs.
/// Each entry tries Kitsu's `myanimelist/anime` mapping first, then
/// its `anilist/anime` mapping — Kitsu's MAL table lags brand-new
/// seasonal entries by weeks while the AniList row exists from day
/// one, and a drop here removes exactly the season's hottest shows
/// from the strip. Entries mapped on neither site are dropped.
/// Preserves the AniList ranking — order matters because the home
/// hero takes the first 5–6 entries. `FuturesOrdered` runs the
/// lookups concurrently and yields results in the input order; the
/// resulting Kitsu /mappings calls fan out at once. 25 lookups
/// against a single endpoint is fine and the cache layer absorbs
/// the cost on subsequent visits.
#[cfg(test)]
pub(in crate::commands) async fn bridge_anilist_to_kitsu(
    state: &AppState,
    anilist: Vec<crate::meta::anilist::AniListAnimeRef>,
) -> Vec<KitsuAnimeRef> {
    bridge_answered(state, anilist).await.0
}

/// [`bridge_anilist_to_kitsu`], and whether Kitsu answered every
/// lookup an entry it left out needed: `false` when an entry was
/// dropped because a lookup failed rather than because Kitsu maps it
/// on neither site.
pub(super) async fn bridge_answered(
    state: &AppState,
    anilist: Vec<crate::meta::anilist::AniListAnimeRef>,
) -> (Vec<KitsuAnimeRef>, bool) {
    use futures_util::stream::{FuturesOrdered, StreamExt};

    let mut futures = FuturesOrdered::new();
    for entry in anilist {
        let kitsu = state.kitsu.clone();
        let pool = state.cache_pool.clone();
        futures.push_back(async move {
            // AniList may still list a banner its CDN has told the
            // image proxy is gone; that one is not offered again.
            let banner = entry
                .banner_image
                .clone()
                .filter(|url| !crate::meta::images::is_dead(&pool, url));
            let by_mal = match entry.id_mal {
                Some(mal_id) => kitsu.lookup_by_mal_id(mal_id).await,
                None => Ok(None),
            };
            let mut kref = match by_mal {
                Ok(Some(k)) => k,
                // No MAL mapping, or Kitsu not saying: the AniList
                // mapping decides, and the entry is a dead end only
                // when both lookups answered.
                by_mal => match kitsu.lookup_by_anilist_id(entry.id).await {
                    Ok(Some(k)) => k,
                    Ok(None) => return Err(by_mal.is_ok()),
                    Err(_) => return Err(false),
                },
            };
            // AniList's banner rides beside Kitsu's cover, free: this
            // request already carried it.
            kref.anilist_banner.clone_from(&banner);
            // Backfill banner from AniList when Kitsu hasn't
            // catalogued one. New ongoing shows like Slime S4 land
            // on Kitsu before their cataloguers upload a banner;
            // AniList typically has it from day one. The image
            // proxy accepts any https URL so the renderer doesn't
            // need to know the source.
            if kref.cover_image.is_none() {
                if let Some(url) = banner {
                    kref.cover_image = Some(KitsuCoverImage {
                        tiny: None,
                        small: None,
                        large: Some(url.clone()),
                        original: Some(url),
                    });
                }
            }
            Ok(kref)
        });
    }
    // Each entry is its ref, or `Err(answered)`: left out, and whether
    // Kitsu answered both lookups that left it out.
    let mut out = Vec::new();
    let mut complete = true;
    while let Some(bridged) = futures.next().await {
        match bridged {
            Ok(r) => out.push(r),
            Err(answered) => complete &= answered,
        }
    }
    (out, complete)
}
