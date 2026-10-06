//! [`MediaRef`] — which id an AniList query is keyed by — and the
//! by-`MediaRef` banner lookup. Kept beside `meta::anilist` rather than
//! inside it so that file's ccn stays under the CRAP ceiling, the same
//! reason `anilist_streaming_eps` lives apart.

use crate::error::Result;
use crate::meta::anilist::{
    banner_for_mal_id, parse_banner_response, post_graphql_public, ANILIST_API,
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
    match media {
        MediaRef::Mal(mal_id) => banner_for_mal_id(client, mal_id, base_override).await,
        MediaRef::AniList(id) => {
            let url = base_override.unwrap_or(ANILIST_API);
            let body = serde_json::json!({
                "query": BANNER_BY_ID_GQL,
                "variables": { "id": id },
            });
            let bytes = post_graphql_public(client, url, &body).await?;
            parse_banner_response(&bytes)
        }
    }
}

#[cfg(test)]
#[path = "anilist_media_test.rs"]
mod tests;
