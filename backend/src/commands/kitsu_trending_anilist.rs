//! The home page's AniList-backed trending row and its bridge from
//! AniList's entries to Kitsu's. Split from [`super`] so each file
//! stays inside the CRAP gate's per-file bar.

use super::*;

/// AniList-backed trending — the hero / Trending strip on home use
/// this in preference to [`kitsu_trending`] because AniList's
/// `TRENDING_DESC` is recency-weighted (its `trending` value counts
/// viewer activity over the past hour), which keeps evergreens like
/// One Piece from anchoring the top of the row regardless of what is
/// hot now.
///
/// Pipeline:
///   1. Fetch AniList trending (top N).
///   2. For each entry, bridge MAL id → Kitsu anime via Kitsu's
///      /mappings endpoint, concurrent with bounded parallelism.
///   3. Drop entries that don't bridge (rare — AniList show without
///      a Kitsu mapping). Preserve the AniList ranking for the rest.
///   4. Cache the resulting `Vec<KitsuAnimeRef>` for [`TRENDING_TTL`]
///      (1h). AniList trending updates faster than that, but home
///      doesn't need sub-hour freshness.
///
/// When AniList errors or nothing it returns can be bridged, the last
/// list AniList gave is served, however stale, and left as it is. Only
/// when no AniList list was ever kept does [`kitsu_trending`]
/// (currently-airing-by-user-count) stand in, for that load.
///
/// # Errors
/// Bubbles AniList / Kitsu transport failures only when the fallback
/// also fails. Cache write failures are silently ignored.
pub async fn kitsu_trending_anilist(state: &AppState) -> Result<Vec<KitsuAnimeRef>> {
    kitsu_trending_anilist_with_base(state, None).await
}

/// [`kitsu_trending_anilist`] with the AniList endpoint overridable,
/// so tests can point the trending fetch at wiremock. Production
/// passes `None`.
pub(crate) async fn kitsu_trending_anilist_with_base(
    state: &AppState,
    anilist_base: Option<&str>,
) -> Result<Vec<KitsuAnimeRef>> {
    // v5: the bridge gained the anilist/anime mapping fallback for
    //     entries Kitsu hasn't MAL-mapped yet. v4 rows were built
    //     without it and silently omit exactly the newest seasonal
    //     shows (TYBW's fourth cour was missing from the strip);
    //     bumping refreshes them.
    // v4: bridge backfills cover_image from AniList's bannerImage
    //     when Kitsu's is null (new ongoing shows). v3 rows have
    //     null covers for those entries; bumping forces a refresh.
    // v6: each ref keeps AniList's bannerImage as `anilist_banner`
    //     beside Kitsu's cover; v5 rows were built without it.
    const KEY: &str = "kitsu:v6:trending_anilist";
    if let Some(body) = meta_cache_get(&state.cache_pool, KEY)? {
        if let Ok(hits) = serde_json::from_str::<Vec<KitsuAnimeRef>>(&body) {
            warm_signed_image_urls(state, &body);
            return Ok(drop_music(hits));
        }
    }

    // AniList → MAL ids → bridge to Kitsu refs.
    let anilist =
        crate::meta::anilist::trending(&state.meta_http, ANILIST_TRENDING_LIMIT, anilist_base)
            .await;
    if let Err(ref e) = anilist {
        tracing::warn!(error = ?e, "anilist trending failed; falling back to kitsu_trending");
    }
    // The row is kept only when it is AniList's answer, bridged in
    // full: Kitsu's list stands in for this load only, and a row
    // missing an entry Kitsu failed to bridge is asked for again.
    let (bridged, complete) = match anilist {
        Ok(v) if !v.is_empty() => bridge_answered(state, v).await,
        Ok(_) => (Vec::new(), true),
        Err(_) => (Vec::new(), false),
    };
    let keep = complete && !bridged.is_empty();
    // Only AniList's own answer says anything about a banner; Kitsu's
    // fallback list below never carries one.
    crate::commands::anilist_banner_memo::remember(&state.cache_pool, &bridged);

    // Nothing bridged — AniList did not answer, answered nothing, or
    // Kitsu could not bridge it. The last list AniList gave stands,
    // however stale: it is AniList's order, which Kitsu's airing list
    // is not. Only with no AniList list kept at all does Kitsu's list
    // stand in, for this load. Neither is written: the row still
    // holds AniList's last answer, and the next load asks again.
    if bridged.is_empty() {
        if let Some((hits, body)) = last_anilist_list(state, KEY) {
            warm_signed_image_urls(state, &body);
            return Ok(drop_music(hits));
        }
    }
    let hits = if bridged.is_empty() {
        kitsu_trending(state).await?
    } else {
        bridged
    };

    // Bridged refs come from lookup_by_mal_id (single-resource parse), not
    // the list parser, so sanitize music here too.
    let hits = drop_music(hits);
    if let Ok(body) = serde_json::to_string(&hits) {
        if keep {
            let _ = meta_cache_put(&state.cache_pool, KEY, &body, TRENDING_TTL.as_secs());
        }
        warm_signed_image_urls(state, &body);
    }
    Ok(hits)
}

/// The list under `key` as AniList last gave it, expired or not — an
/// expired row is kept until a new answer replaces it. `None` when no
/// AniList list was ever kept, or the row cannot be read.
fn last_anilist_list(state: &AppState, key: &str) -> Option<(Vec<KitsuAnimeRef>, String)> {
    let row = crate::cache::meta_cache_row(&state.cache_pool, key).ok()??;
    let hits = serde_json::from_str::<Vec<KitsuAnimeRef>>(&row.body).ok()?;
    Some((hits, row.body))
}

/// AniList trending page size — N entries fetched, then bridged to
/// Kitsu. We render the hero from items 0..5 and the Trending strip
/// from the rest, so 25 leaves comfortable headroom for shows that
/// fail to bridge (rare; a couple per page at most).
const ANILIST_TRENDING_LIMIT: u8 = 25;

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
async fn bridge_answered(
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
