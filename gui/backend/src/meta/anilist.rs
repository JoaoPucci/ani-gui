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

const ANILIST_API: &str = "https://graphql.anilist.co";

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
async fn post_graphql_public(
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
mod tests {
    use super::*;

    /// Real shape lifted from a live AniList trending probe. Three
    /// entries; covers the optional fields (idMal, episodes,
    /// bannerImage) hitting both Some and null variants so the
    /// derive's `Option` handling is exercised.
    fn fixture_body() -> &'static [u8] {
        // Regular raw string (not byte raw) because the JSON contains
        // Japanese characters; `br#"…"#` rejects non-ASCII.
        FIXTURE_JSON.as_bytes()
    }

    const FIXTURE_JSON: &str = r##"{
            "data": {
                "Page": {
                    "media": [
                        {
                            "id": 182205,
                            "idMal": 59970,
                            "title": {
                                "romaji": "Tensei Shitara Slime Datta Ken 4th Season",
                                "english": "That Time I Got Reincarnated as a Slime Season 4",
                                "native": "転生したらスライムだった件 第4期",
                                "userPreferred": "Tensei Shitara Slime Datta Ken 4th Season"
                            },
                            "coverImage": {
                                "extraLarge": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/bx182205-q.jpg",
                                "large": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/medium/bx182205-q.jpg",
                                "medium": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/small/bx182205-q.jpg",
                                "color": "#1abbd6"
                            },
                            "bannerImage": "https://s4.anilist.co/file/anilistcdn/media/anime/banner/182205-f.jpg",
                            "status": "RELEASING",
                            "episodes": null,
                            "trending": 273,
                            "averageScore": 80
                        },
                        {
                            "id": 21,
                            "idMal": 21,
                            "title": {
                                "romaji": "ONE PIECE",
                                "english": "ONE PIECE",
                                "native": "ONE PIECE",
                                "userPreferred": "ONE PIECE"
                            },
                            "coverImage": {
                                "extraLarge": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/large/bx21-E.jpg",
                                "large": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/medium/bx21-E.jpg",
                                "medium": "https://s4.anilist.co/file/anilistcdn/media/anime/cover/small/bx21-E.jpg",
                                "color": "#e49335"
                            },
                            "bannerImage": "https://s4.anilist.co/file/anilistcdn/media/anime/banner/21-w.jpg",
                            "status": "RELEASING",
                            "episodes": null,
                            "trending": 167,
                            "averageScore": 87
                        },
                        {
                            "id": 999999,
                            "idMal": null,
                            "title": {
                                "romaji": "Hypothetical Show With No MAL Id",
                                "english": null,
                                "native": null,
                                "userPreferred": "Hypothetical Show With No MAL Id"
                            },
                            "coverImage": {
                                "extraLarge": null,
                                "large": null,
                                "medium": null,
                                "color": null
                            },
                            "bannerImage": null,
                            "status": null,
                            "episodes": null,
                            "trending": null,
                            "averageScore": null
                        }
                    ]
                }
            }
        }"##;

    #[test]
    fn parse_trending_yields_expected_count_and_first_entry() {
        let v = parse_trending(fixture_body()).expect("parses");
        assert_eq!(v.len(), 3);
        let first = &v[0];
        assert_eq!(first.id, 182205);
        assert_eq!(first.id_mal, Some(59970));
        assert_eq!(
            first.title.user_preferred.as_deref(),
            Some("Tensei Shitara Slime Datta Ken 4th Season")
        );
        assert_eq!(first.cover_image.color.as_deref(), Some("#1abbd6"));
        assert!(first.banner_image.is_some());
        assert_eq!(first.status.as_deref(), Some("RELEASING"));
        assert_eq!(first.trending, Some(273));
    }

    #[test]
    fn parse_trending_handles_missing_optionals() {
        let v = parse_trending(fixture_body()).expect("parses");
        let third = &v[2];
        assert_eq!(third.id, 999999);
        assert_eq!(third.id_mal, None);
        assert_eq!(third.title.english, None);
        assert_eq!(third.cover_image.extra_large, None);
        assert_eq!(third.banner_image, None);
        assert_eq!(third.status, None);
        assert_eq!(third.episodes, None);
        assert_eq!(third.trending, None);
        assert_eq!(third.average_score, None);
    }

    #[test]
    fn parse_trending_rejects_html_or_garbage() {
        let r = parse_trending(b"<html>not json</html>");
        assert!(matches!(r, Err(AniError::ParseFailed { .. })));
    }

    #[tokio::test]
    async fn trending_makes_correct_post_request() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/"))
            .and(wiremock::matchers::header(
                "content-type",
                "application/json",
            ))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "query": TRENDING_GQL,
                "variables": { "perPage": 5 },
            })))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_bytes(fixture_body()))
            .mount(&server)
            .await;

        let client = reqwest::Client::new();
        let v = trending(&client, 5, Some(&server.uri())).await.expect("ok");
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].id, 182205);
    }

    #[tokio::test]
    async fn trending_propagates_5xx_as_upstream() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(503))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = trending(&client, 5, Some(&server.uri())).await.unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 503 }));
    }

    #[tokio::test]
    async fn trending_surfaces_429_for_rate_limit() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(429))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = trending(&client, 5, Some(&server.uri())).await.unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 429 }));
    }

    /// `parse_banner_response` is the pure half of the banner-
    /// backfill flow. Three branches matter: media present + banner
    /// present, media present + banner null, and media itself null
    /// (AniList didn't index this MAL id at all).
    #[test]
    fn parse_banner_response_returns_url_when_present() {
        let body = br#"{"data":{"Media":{"bannerImage":"https://example.com/b.jpg"}}}"#;
        let got = parse_banner_response(body).expect("ok");
        assert_eq!(got.as_deref(), Some("https://example.com/b.jpg"));
    }

    #[test]
    fn parse_banner_response_returns_none_when_banner_is_null() {
        let body = br#"{"data":{"Media":{"bannerImage":null}}}"#;
        let got = parse_banner_response(body).expect("ok");
        assert!(got.is_none());
    }

    #[test]
    fn parse_banner_response_returns_none_when_media_is_null() {
        // AniList answers with `Media: null` when the MAL id isn't
        // in their catalogue. Treating that as a hard error would
        // break the detail page for any show AniList missed.
        let body = br#"{"data":{"Media":null}}"#;
        let got = parse_banner_response(body).expect("ok");
        assert!(got.is_none());
    }

    #[test]
    fn parse_banner_response_rejects_non_envelope_payload() {
        let body = br#"<html>error</html>"#;
        let err = parse_banner_response(body).unwrap_err();
        assert!(matches!(err, AniError::ParseFailed { .. }));
    }

    #[tokio::test]
    async fn banner_for_mal_id_posts_correct_query() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "query": BANNER_BY_MAL_GQL,
                "variables": { "idMal": 21 },
            })))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
                r#"{"data":{"Media":{"bannerImage":"https://example.com/op.jpg"}}}"#,
            ))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let got = banner_for_mal_id(&client, 21, Some(&server.uri()))
            .await
            .expect("ok");
        assert_eq!(got.as_deref(), Some("https://example.com/op.jpg"));
    }

    #[tokio::test]
    async fn banner_for_mal_id_propagates_upstream_5xx() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(502))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = banner_for_mal_id(&client, 21, Some(&server.uri()))
            .await
            .unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 502 }));
    }

    #[tokio::test]
    async fn banner_for_mal_id_returns_none_when_media_unmapped() {
        // End-to-end through the network layer for the "AniList
        // doesn't have this MAL id" case — the most common failure
        // mode in production for niche shows.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":null}}"#),
            )
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let got = banner_for_mal_id(&client, 99_999_999, Some(&server.uri()))
            .await
            .expect("ok");
        assert!(got.is_none());
    }

    // `media_id_for_mal` resolves a MAL id → AniList numeric mediaId,
    // the keystone the write-back path needs: mark-watched knows the
    // Kitsu id → mal_id, but SaveMediaListEntry wants AniList's own id.
    #[test]
    fn parse_media_id_response_returns_id_when_present() {
        let body = br#"{"data":{"Media":{"id":154587}}}"#;
        let got = parse_media_id_response(body).expect("ok");
        assert_eq!(got, Some(154587));
    }

    #[test]
    fn parse_media_id_response_returns_none_when_media_is_null() {
        // AniList answers Media: null when the MAL id isn't indexed.
        let body = br#"{"data":{"Media":null}}"#;
        let got = parse_media_id_response(body).expect("ok");
        assert!(got.is_none());
    }

    #[test]
    fn parse_media_id_response_rejects_non_envelope_payload() {
        let err = parse_media_id_response(br#"<html>nope</html>"#).unwrap_err();
        assert!(matches!(err, AniError::ParseFailed { .. }));
    }

    #[tokio::test]
    async fn media_id_for_mal_posts_idmal_query_and_returns_media_id() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "query": MEDIA_ID_BY_MAL_GQL,
                "variables": { "idMal": 52991 },
            })))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_string(r#"{"data":{"Media":{"id":154587}}}"#),
            )
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let got = media_id_for_mal(&client, 52991, Some(&server.uri()))
            .await
            .expect("ok");
        assert_eq!(got, Some(154587));
    }

    #[tokio::test]
    async fn media_id_for_mal_returns_none_when_unmapped() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":null}}"#),
            )
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let got = media_id_for_mal(&client, 99_999_999, Some(&server.uri()))
            .await
            .expect("ok");
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn media_id_for_mal_propagates_upstream_5xx() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(502))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = media_id_for_mal(&client, 52991, Some(&server.uri()))
            .await
            .unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 502 }));
    }

    // `mal_id_for_media_id` is the inverse bridge: Kitsu shows that
    // carry only the anilist/anime mapping (Yani Neko, AniList 207141)
    // still need a MAL id to write to MyAnimeList.
    #[test]
    fn parse_mal_id_response_returns_idmal_when_present() {
        let body = br#"{"data":{"Media":{"idMal":63403}}}"#;
        let got = parse_mal_id_response(body).expect("ok");
        assert_eq!(got, Some(63403));
    }

    #[test]
    fn parse_mal_id_response_returns_none_when_idmal_is_null() {
        // AniList-only originals carry Media.idMal: null.
        let body = br#"{"data":{"Media":{"idMal":null}}}"#;
        let got = parse_mal_id_response(body).expect("ok");
        assert!(got.is_none());
    }

    #[test]
    fn parse_mal_id_response_returns_none_when_media_is_null() {
        let body = br#"{"data":{"Media":null}}"#;
        let got = parse_mal_id_response(body).expect("ok");
        assert!(got.is_none());
    }

    #[test]
    fn parse_mal_id_response_rejects_non_envelope_payload() {
        let err = parse_mal_id_response(br#"<html>nope</html>"#).unwrap_err();
        assert!(matches!(err, AniError::ParseFailed { .. }));
    }

    #[tokio::test]
    async fn mal_id_for_media_id_posts_media_query_and_returns_idmal() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::body_json(serde_json::json!({
                "query": MAL_ID_BY_MEDIA_GQL,
                "variables": { "id": 207141 },
            })))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .set_body_string(r#"{"data":{"Media":{"idMal":63403}}}"#),
            )
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let got = mal_id_for_media_id(&client, 207141, Some(&server.uri()))
            .await
            .expect("ok");
        assert_eq!(got, Some(63403));
    }

    #[tokio::test]
    async fn mal_id_for_media_id_propagates_upstream_5xx() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .respond_with(wiremock::ResponseTemplate::new(502))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let err = mal_id_for_media_id(&client, 207141, Some(&server.uri()))
            .await
            .unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 502 }));
    }
}
