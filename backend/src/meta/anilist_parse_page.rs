//! Reading AniList's batched by-MAL `Page` replies: every media each
//! MAL id is recorded by, and whether AniList cut the page. Split from
//! [`super`] so each file stays inside the CRAP gate's per-file bar.

use super::*;
use crate::meta::present::present;

/// Whether a `{ data: { Page: { pageInfo: { hasNextPage } } } }`
/// response says AniList cut the page: more media matched than it holds.
/// The query asks for the flag, so a page that leaves out `pageInfo` or
/// `hasNextPage`, or gives either as null, does not say whether it was
/// cut: no answer, never a whole page.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the expected
/// envelope, or its page does not state `hasNextPage` as a boolean.
pub fn page_has_next(body: &[u8]) -> Result<bool> {
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
        #[serde(rename = "pageInfo")]
        info: Info,
    }
    #[derive(Deserialize)]
    struct Info {
        #[serde(rename = "hasNextPage")]
        has_next: bool,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist page info: {e}"),
    })?;
    Ok(parsed.data.page.info.has_next)
}

/// The `(id, idMal)` of every media in a `{ data: { Page: { media:
/// [{ id, idMal }] } } }` response.
fn page_media(body: &[u8]) -> Result<Vec<(u32, Option<u32>)>> {
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
    // Every media is asked for its idMal: one that leaves it out cannot
    // say which MAL id it records; null records none.
    #[derive(Deserialize)]
    struct Media {
        id: u32,
        #[serde(rename = "idMal", deserialize_with = "present")]
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
        .map(|m| (m.id, m.id_mal))
        .collect())
}

/// Every media in a batched by-MAL response, by the MAL id it records:
/// each MAL id's media in AniList id order, each once. Entries without
/// an `idMal` are skipped.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the expected
/// envelope.
pub fn parse_media_listed_by_mal_response(
    body: &[u8],
) -> Result<std::collections::HashMap<u32, Vec<u32>>> {
    let mut listed: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    for (id, mal) in page_media(body)? {
        if let Some(mal) = mal {
            listed.entry(mal).or_default().push(id);
        }
    }
    for ids in listed.values_mut() {
        ids.sort_unstable();
        ids.dedup();
    }
    Ok(listed)
}

/// Every media in a batched by-MAL response that records `mal_id`, in
/// AniList id order, each once.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the expected
/// envelope.
pub fn parse_media_ids_recording_mal(body: &[u8], mal_id: u32) -> Result<Vec<u32>> {
    let mut ids: Vec<u32> = page_media(body)?
        .into_iter()
        .filter(|(_, mal)| *mal == Some(mal_id))
        .map(|(id, _)| id)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

#[cfg(test)]
#[path = "anilist_parse_page_prop_test.rs"]
mod prop_tests;
