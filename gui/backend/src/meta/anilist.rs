//! AniList GraphQL client.
//!
//! Used today only for the home page's Trending Now row — AniList's
//! `TRENDING_DESC` sort is genuinely week-fresh (it weights recent
//! activity surge), unlike Kitsu's `userCount` which is cumulative
//! across all time and lets evergreens like One Piece anchor the top
//! forever. The plan-doc rationale is in `requirements.md` §7 / D2.
//!
//! Read-only public queries don't require auth. AniList rate-limits
//! all clients to 30 requests/minute (per IP). With a 30-min cache
//! on the trending fetch, we use ~2 requests/hour — well under.
//!
//! Cross-references: each `Media` entry exposes `idMal`
//! (MyAnimeList id), which the home page bridges to a Kitsu id
//! through Kitsu's `mappings` endpoint to keep nav + the rest of the
//! app on Kitsu's id space.

use serde::Deserialize;

use crate::error::{AniError, Result};

pub(crate) const ANILIST_API: &str = "https://graphql.anilist.co";

/// One trending anime as AniList serves it. Fields chosen to match
/// what the home-page bridge consumes: `id_mal` for the Kitsu lookup,
/// `title.user_preferred` for fallback display when the bridge
/// fails, the rest available for richer rendering when we want it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AniListAnimeRef {
    /// AniList's own id.
    pub id: u32,
    /// MyAnimeList id. The bridge to Kitsu — Kitsu's `mappings`
    /// endpoint accepts `filter[externalSite]=myanimelist/anime` +
    /// `filter[externalId]=<id_mal>`. May be null on shows AniList
    /// indexes but MAL doesn't (rare).
    #[serde(rename = "idMal")]
    pub id_mal: Option<u32>,
    /// Title bag — same shape as Kitsu's `titles` map but with fixed
    /// keys. `user_preferred` is the field AniList renders by default
    /// in their own UI; safe display fallback.
    pub title: AniListTitle,
    /// Cover (poster) image bag. AniList serves three pre-rendered
    /// sizes plus an extracted dominant colour for theming.
    #[serde(rename = "coverImage")]
    pub cover_image: AniListCoverImage,
    /// Single banner URL (~21:5). May be null on shows that don't
    /// have a banner uploaded; the renderer falls back to the cover
    /// in that case.
    #[serde(rename = "bannerImage")]
    pub banner_image: Option<String>,
    /// AniList airing status — `"RELEASING"`, `"FINISHED"`,
    /// `"NOT_YET_RELEASED"`, `"CANCELLED"`, `"HIATUS"`.
    pub status: Option<String>,
    /// Total announced episode count. Null on shows without a
    /// confirmed total.
    pub episodes: Option<u32>,
    /// AniList's trending score for this entry — rough surrogate for
    /// "users who interacted in the last few days." Higher = hotter.
    pub trending: Option<u32>,
    /// Mean rating × 100 (0..=100). Optional because not every show
    /// has enough scores to compute one.
    #[serde(rename = "averageScore")]
    pub average_score: Option<u32>,
}

/// AniList exposes four well-known title forms. `user_preferred` is
/// the one the AniList UI itself defaults to.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AniListTitle {
    /// Latin-script transliteration of the original title (e.g.
    /// `"Shingeki no Kyojin"`). Almost always present.
    pub romaji: Option<String>,
    /// Localized English title when one exists (`"Attack on Titan"`).
    /// Often null for less-licensed shows.
    pub english: Option<String>,
    /// Native (Japanese) title in its original script.
    pub native: Option<String>,
    /// AniList's own preferred display form — picks one of the
    /// above based on the viewer's language settings on anilist.co.
    #[serde(rename = "userPreferred")]
    pub user_preferred: Option<String>,
}

/// AniList cover-image bag. Sizes from largest to smallest plus an
/// extracted dominant colour string (`"#1abbd6"` etc.) usable as a
/// theming accent.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AniListCoverImage {
    /// Largest cover variant — used for hero / detail-page art.
    #[serde(rename = "extraLarge")]
    pub extra_large: Option<String>,
    /// Mid-size cover — used for grid cards and trending tiles.
    pub large: Option<String>,
    /// Smallest cover — used for compact lists and search snippets.
    pub medium: Option<String>,
    /// `"#rrggbb"` — already in the format CSS expects.
    pub color: Option<String>,
}

/// GraphQL query string for the trending feed. Field set is the
/// minimum the bridge + frontend need; expand here when adding new
/// surfaces. `perPage` is bound at call time.
const TRENDING_GQL: &str = "query Trending($perPage: Int!) { \
    Page(perPage: $perPage) { \
        media(type: ANIME, sort: TRENDING_DESC) { \
            id idMal \
            title { romaji english native userPreferred } \
            coverImage { extraLarge large medium color } \
            bannerImage \
            status episodes trending averageScore \
        } \
    } \
}";

/// Lightweight by-MAL-id query — used by detail-page enrichment
/// when Kitsu's coverImage is null and we need a banner fallback.
/// Smaller projection than [`TRENDING_GQL`] since the caller only
/// needs the banner URL.
const BANNER_BY_MAL_GQL: &str = "query BannerByMal($idMal: Int!) { \
        Media(idMal: $idMal, type: ANIME) { bannerImage } \
    }";

/// User-agent for every AniList request. The proxy client mimics
/// Firefox, but AniList's Cloudflare layer 403s browser UAs that lack
/// a full fingerprint — an app-style identifier passes through.
const ANILIST_UA: &str = "ani-gui/0.1 (https://github.com/pucci/ani-gui)";

/// Shared POST to AniList's public GraphQL endpoint. The three public
/// fetchers (`trending`, `banner_for_mal_id`, `media_id_for_mal`) only
/// differ in query body + parser, so the request build + status
/// mapping live here once.
///
/// # Errors
/// [`AniError::Network`] on transport failure, [`AniError::Upstream`]
/// on non-2xx.
pub(crate) async fn post_graphql_public(
    client: &reqwest::Client,
    url: &str,
    body: &serde_json::Value,
) -> Result<bytes::Bytes> {
    let resp = client
        .post(url)
        .header("user-agent", ANILIST_UA)
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|_| AniError::Network)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AniError::Upstream {
            status: status.as_u16(),
        });
    }
    resp.bytes().await.map_err(|_| AniError::Network)
}

/// Fetch the AniList trending feed, top `limit` entries.
///
/// `base_override` mirrors the convention in `scraper::allanime` —
/// `None` in prod (hits the real GraphQL endpoint), `Some(uri)` in
/// tests pointing at wiremock.
///
/// # Errors
/// - [`AniError::Network`] on connection failure.
/// - [`AniError::Upstream`] on non-2xx HTTP.
/// - [`AniError::ParseFailed`] when the response shape is wrong.
pub async fn trending(
    client: &reqwest::Client,
    limit: u8,
    base_override: Option<&str>,
) -> Result<Vec<AniListAnimeRef>> {
    let url = base_override.unwrap_or(ANILIST_API);
    let body = serde_json::json!({
        "query": TRENDING_GQL,
        "variables": { "perPage": limit },
    });
    let bytes = post_graphql_public(client, url, &body).await?;
    parse_trending(&bytes)
}

/// Look up the AniList banner URL for a show by its MAL id.
/// Returns `None` when AniList has no media for the supplied id, or
/// when AniList has the media but no banner uploaded. Used by the
/// detail-page enrichment chain (Kitsu null cover → MAL id → here).
///
/// # Errors
/// Same as [`trending`] — Network / Upstream / ParseFailed.
pub async fn banner_for_mal_id(
    client: &reqwest::Client,
    mal_id: u32,
    base_override: Option<&str>,
) -> Result<Option<String>> {
    let url = base_override.unwrap_or(ANILIST_API);
    let body = serde_json::json!({
        "query": BANNER_BY_MAL_GQL,
        "variables": { "idMal": mal_id },
    });
    let bytes = post_graphql_public(client, url, &body).await?;
    parse_banner_response(&bytes)
}

/// By-MAL-id query resolving AniList's numeric `mediaId`. The
/// write-back path needs it: mark-watched knows the show's MAL id
/// (via Kitsu mappings) but `SaveMediaListEntry` keys on AniList's
/// own id. Green commit fills the body in.
const MEDIA_ID_BY_MAL_GQL: &str = "query MediaIdByMal($idMal: Int!) { \
        Media(idMal: $idMal, type: ANIME) { id } \
    }";

/// Resolve a MAL id → AniList numeric `mediaId`. `None` when AniList
/// doesn't index the supplied MAL id. Mirrors [`banner_for_mal_id`]'s
/// network shape; `base_override` points at wiremock in tests.
///
/// # Errors
/// Network / Upstream / ParseFailed — same as [`banner_for_mal_id`].
pub async fn media_id_for_mal(
    client: &reqwest::Client,
    mal_id: u32,
    base_override: Option<&str>,
) -> Result<Option<u32>> {
    let url = base_override.unwrap_or(ANILIST_API);
    let body = serde_json::json!({
        "query": MEDIA_ID_BY_MAL_GQL,
        "variables": { "idMal": mal_id },
    });
    let resp = client
        .post(url)
        .header(
            "user-agent",
            "ani-gui/0.1 (https://github.com/pucci/ani-gui)",
        )
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|_| AniError::Network)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AniError::Upstream {
            status: status.as_u16(),
        });
    }
    let bytes = resp.bytes().await.map_err(|_| AniError::Network)?;
    parse_media_id_response(&bytes)
}

/// By-AniList-id query resolving the show's MAL id — the inverse of
/// [`MEDIA_ID_BY_MAL_GQL`]. Tracker id-resolution falls back to it
/// when Kitsu carries only the `anilist/anime` mapping (fresh
/// seasonal shows): the direct mapping reaches AniList, and this
/// query bridges the rest of the way to MAL.
const MAL_ID_BY_MEDIA_GQL: &str = "query MalIdByMedia($id: Int!) { \
        Media(id: $id, type: ANIME) { idMal } \
    }";

/// Resolve an AniList numeric `mediaId` → the show's MAL id. `None`
/// when AniList has no MAL link recorded. Mirrors
/// [`media_id_for_mal`]'s network shape; `base_override` points at
/// wiremock in tests.
///
/// # Errors
/// Network / Upstream / ParseFailed — same as [`media_id_for_mal`].
pub async fn mal_id_for_media_id(
    client: &reqwest::Client,
    media_id: u32,
    base_override: Option<&str>,
) -> Result<Option<u32>> {
    let url = base_override.unwrap_or(ANILIST_API);
    let body = serde_json::json!({
        "query": MAL_ID_BY_MEDIA_GQL,
        "variables": { "id": media_id },
    });
    let bytes = post_graphql_public(client, url, &body).await?;
    parse_mal_id_response(&bytes)
}

/// Pure parser for the by-media `idMal` response.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the expected
/// `{ data: { Media: { idMal } } }` envelope. Both `Media: null`
/// (unknown media id) and `idMal: null` (no MAL link) map to
/// `Ok(None)`, not an error.
pub fn parse_mal_id_response(body: &[u8]) -> Result<Option<u32>> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "Media")]
        media: Option<Media>,
    }
    #[derive(Deserialize)]
    struct Media {
        #[serde(rename = "idMal")]
        id_mal: Option<u32>,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist idMal response: {e}"),
    })?;
    Ok(parsed.data.media.and_then(|m| m.id_mal))
}

/// Batched inverse of [`MEDIA_ID_BY_MAL_GQL`]: many MAL ids → their
/// AniList `mediaId`s in ONE request per [`MAL_BATCH_PAGE_SIZE`]-sized
/// chunk. The Watch-Later bridge uses it so a rail load with many
/// Kitsu-unmapped titles costs O(n/50) AniList calls instead of O(n)
/// against the public 30 req/min limit (Codex P2 #3565216298).
const MEDIA_IDS_BY_MALS_GQL: &str = "query MediaIdsByMals($idMals: [Int]) { \
        Page(perPage: 50) { media(type: ANIME, idMal_in: $idMals) { id idMal } } \
    }";

/// Max MAL ids per [`media_ids_for_mals`] request — AniList's `Page`
/// caps `perPage` at 50, so larger chunks would silently truncate.
pub const MAL_BATCH_PAGE_SIZE: usize = 50;

/// Resolve many MAL ids → AniList `mediaId`s, chunked at
/// [`MAL_BATCH_PAGE_SIZE`] per request. Ids AniList doesn't index are
/// simply absent from the returned map; empty input makes no request.
///
/// # Errors
/// Network / Upstream / ParseFailed — same as [`media_id_for_mal`].
pub async fn media_ids_for_mals(
    client: &reqwest::Client,
    mal_ids: &[u32],
    base_override: Option<&str>,
) -> Result<std::collections::HashMap<u32, u32>> {
    let url = base_override.unwrap_or(ANILIST_API);
    let mut map = std::collections::HashMap::new();
    for chunk in mal_ids.chunks(MAL_BATCH_PAGE_SIZE) {
        let body = serde_json::json!({
            "query": MEDIA_IDS_BY_MALS_GQL,
            "variables": { "idMals": chunk },
        });
        let bytes = post_graphql_public(client, url, &body).await?;
        map.extend(parse_media_ids_by_mal_response(&bytes)?);
    }
    Ok(map)
}

/// Pure parser for the batched by-MAL response: `{ data: { Page: {
/// media: [{ id, idMal }] } } }` → `idMal → id` map. Entries without
/// an `idMal` are skipped.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the expected
/// envelope.
pub fn parse_media_ids_by_mal_response(body: &[u8]) -> Result<std::collections::HashMap<u32, u32>> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "Page")]
        page: Page,
    }
    #[derive(Deserialize)]
    struct Page {
        media: Vec<Media>,
    }
    #[derive(Deserialize)]
    struct Media {
        id: u32,
        #[serde(rename = "idMal")]
        id_mal: Option<u32>,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist batched idMal response: {e}"),
    })?;
    Ok(parsed
        .data
        .page
        .media
        .into_iter()
        .filter_map(|m| m.id_mal.map(|mal| (mal, m.id)))
        .collect())
}

/// Pure parser for the by-MAL `mediaId` response.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the expected
/// `{ data: { Media: { id } } }` envelope. `Media: null` (MAL id not
/// indexed) maps to `Ok(None)`, not an error.
pub fn parse_media_id_response(body: &[u8]) -> Result<Option<u32>> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "Media")]
        media: Option<Media>,
    }
    #[derive(Deserialize)]
    struct Media {
        id: u32,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist media-id response: {e}"),
    })?;
    Ok(parsed.data.media.map(|m| m.id))
}

/// Pure parser for the by-MAL banner response.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the
/// expected `{ data: { Media: { bannerImage } } }` envelope.
pub fn parse_banner_response(body: &[u8]) -> Result<Option<String>> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "Media")]
        media: Option<Media>,
    }
    #[derive(Deserialize)]
    struct Media {
        #[serde(rename = "bannerImage")]
        banner_image: Option<String>,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist banner response: {e}"),
    })?;
    Ok(parsed.data.media.and_then(|m| m.banner_image))
}

/// Pure parser for the trending response body.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the JSON doesn't shape
/// into `{ data: { Page: { media: [...] } } }`.
pub fn parse_trending(body: &[u8]) -> Result<Vec<AniListAnimeRef>> {
    #[derive(Deserialize)]
    struct Wrap {
        data: Data,
    }
    #[derive(Deserialize)]
    struct Data {
        #[serde(rename = "Page")]
        page: Page,
    }
    #[derive(Deserialize)]
    struct Page {
        media: Vec<AniListAnimeRef>,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist trending response: {e}"),
    })?;
    Ok(parsed.data.page.media)
}

#[cfg(test)]
#[path = "anilist_test.rs"]
mod tests;
