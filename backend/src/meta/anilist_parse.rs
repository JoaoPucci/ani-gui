//! Reading AniList's GraphQL replies — trending, banners, and the MAL-
//! id and media-id mappings; split from [`super`] so each file stays
//! inside the CRAP gate's per-file bar.

use super::*;

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

/// Pure parser for a banner response, by MAL id or by AniList id.
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
