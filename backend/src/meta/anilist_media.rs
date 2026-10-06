//! [`MediaRef`] — which id an AniList query is keyed by — and the
//! by-`MediaRef` banner lookup. Kept beside `meta::anilist` rather than
//! inside it so that file's ccn stays under the CRAP ceiling, the same
//! reason `anilist_streaming_eps` lives apart.

use crate::error::{AniError, Result};
use crate::meta::anilist::{
    parse_banner_response, post_graphql_public, ANILIST_API, BANNER_BY_MAL_GQL,
};

/// The banner query keyed by AniList's own id, for shows Kitsu maps to
/// AniList but not yet to MAL.
const BANNER_BY_ID_GQL: &str = "query BannerById($id: Int!) { \
        Media(id: $id, type: ANIME) { bannerImage } \
    }";

/// How a show is identified to AniList: by its MyAnimeList id
/// (`Media(idMal:)`) or by AniList's own id (`Media(id:)`). Kitsu
/// maps most shows to both; fresh seasonal titles often carry only
/// the `anilist/anime` mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaRef {
    /// A MyAnimeList id, queried through `idMal`.
    Mal(u32),
    /// AniList's own numeric media id, queried through `id`.
    AniList(u32),
}

impl MediaRef {
    /// The MAL id when there is one, else AniList's own id; `None`
    /// when neither is known. Preferring MAL keeps every show that
    /// already resolved on the exact query it used before.
    #[must_use]
    pub fn preferring_mal(mal: Option<u32>, anilist: Option<u32>) -> Option<Self> {
        mal.map(Self::Mal).or(anilist.map(Self::AniList))
    }

    /// The one retry a lookup gets after AniList answered `Media:
    /// null` for `self`: a MAL id AniList does not index, when Kitsu's
    /// mappings also carry AniList's own id. `None` otherwise — an
    /// AniList-id lookup has nothing left to try.
    #[must_use]
    pub fn retry_after_missing(self, anilist: Option<u32>) -> Option<Self> {
        match self {
            Self::Mal(_) => anilist.map(Self::AniList),
            Self::AniList(_) => None,
        }
    }
}

/// Whether an AniList `Media(...)` response says AniList has no such
/// media (`{"data":{"Media":null}}`), as opposed to a media whose
/// fields happen to be empty. An unreadable body is not "absent"; the
/// caller's parser reports it.
pub(crate) fn media_is_absent(body: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(body).is_ok_and(|v| {
        v.pointer("/data/Media")
            .is_some_and(serde_json::Value::is_null)
    })
}

/// The body of an AniList GraphQL response, or the failure it stands
/// for. A 2xx is the body. A 404 whose body says `data.Media: null` is
/// also the body: that is how AniList answers a single-`Media` query
/// for an id it does not index, and every parser reads it as absence.
/// Any other status, a 404 without that shape included, is
/// [`AniError::Upstream`].
///
/// # Errors
/// [`AniError::Upstream`] as above; [`AniError::Network`] when the
/// body cannot be read.
pub(crate) async fn graphql_body(resp: reqwest::Response) -> Result<bytes::Bytes> {
    let status = resp.status();
    if !status.is_success() && status != reqwest::StatusCode::NOT_FOUND {
        return Err(AniError::Upstream {
            status: status.as_u16(),
        });
    }
    let bytes = resp.bytes().await.map_err(|_| AniError::Network)?;
    if status.is_success() || media_is_absent(&bytes) {
        Ok(bytes)
    } else {
        Err(AniError::Upstream {
            status: status.as_u16(),
        })
    }
}

/// Runs `lookup` on the preferred id and, only when AniList answered
/// `Media: null` there, once more on the id
/// [`MediaRef::retry_after_missing`] names. `lookup` yields `None` for
/// an absent media. No id at all asks AniList nothing.
pub(crate) async fn with_missing_retry<T, F, Fut>(
    mal: Option<u32>,
    anilist: Option<u32>,
    lookup: F,
) -> Result<Option<T>>
where
    F: Fn(MediaRef) -> Fut,
    Fut: std::future::Future<Output = Result<Option<T>>>,
{
    let Some(first) = MediaRef::preferring_mal(mal, anilist) else {
        return Ok(None);
    };
    if let Some(found) = lookup(first).await? {
        return Ok(Some(found));
    }
    match first.retry_after_missing(anilist) {
        Some(retry) => lookup(retry).await,
        None => Ok(None),
    }
}

/// The AniList banner for a show identified either way — see
/// [`MediaRef`]. `None` when AniList has no media for the id, or the
/// media has no banner.
///
/// # Errors
/// Network / Upstream / ParseFailed — same as [`banner_for_mal_id`](crate::meta::anilist::banner_for_mal_id).
pub async fn banner_for_media(
    client: &reqwest::Client,
    media: MediaRef,
    base_override: Option<&str>,
) -> Result<Option<String>> {
    Ok(banner_lookup(client, media, base_override).await?.flatten())
}

/// The banner for a show by whichever ids Kitsu's mappings carry: the
/// MAL id first, AniList's own id when there is no MAL id or AniList
/// does not index the MAL one. `None` when neither id is known or no
/// lookup finds a banner.
///
/// # Errors
/// Network / Upstream / ParseFailed from whichever lookup failed.
pub async fn banner_for_ids(
    client: &reqwest::Client,
    mal: Option<u32>,
    anilist: Option<u32>,
    base_override: Option<&str>,
) -> Result<Option<String>> {
    let found = with_missing_retry(mal, anilist, |media| {
        banner_lookup(client, media, base_override)
    })
    .await?;
    Ok(found.flatten())
}

/// One banner query. Outer `None`: AniList has no such media; inner
/// `None`: the media has no banner.
async fn banner_lookup(
    client: &reqwest::Client,
    media: MediaRef,
    base_override: Option<&str>,
) -> Result<Option<Option<String>>> {
    let body = match media {
        MediaRef::Mal(mal_id) => serde_json::json!({
            "query": BANNER_BY_MAL_GQL,
            "variables": { "idMal": mal_id },
        }),
        MediaRef::AniList(id) => serde_json::json!({
            "query": BANNER_BY_ID_GQL,
            "variables": { "id": id },
        }),
    };
    let url = base_override.unwrap_or(ANILIST_API);
    let bytes = post_graphql_public(client, url, &body).await?;
    if media_is_absent(&bytes) {
        return Ok(None);
    }
    parse_banner_response(&bytes).map(Some)
}

/// AniList's real answer to a single-`Media` query for an id it does
/// not index: HTTP 404 with a GraphQL `errors` array beside
/// `data.Media: null` (captured from graphql.anilist.co). Tests mock
/// with it so the absent case is exercised as production sees it.
#[cfg(test)]
pub(crate) const ANILIST_NOT_FOUND_BODY: &str = r#"{"errors":[{"message":"Not Found.","status":404,"locations":[{"line":1,"column":20}]}],"data":{"Media":null}}"#;

#[cfg(test)]
#[path = "anilist_media_test.rs"]
mod tests;
