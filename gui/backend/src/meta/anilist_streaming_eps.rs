//! AniList `Media.streamingEpisodes` client — the Crunchyroll-style
//! per-episode listing AniList carries for licensed shows. Used by
//! the episode-thumb backfill in
//! `commands::anilist_eps_thumbs` to fill nulls in Kitsu's episode
//! thumbnails.
//!
//! Lives in its own module instead of the catch-all `meta::anilist`
//! so the file's ccn stays under the CRAP ceiling — `meta::anilist`
//! already carries the trending + banner surface, and adding three
//! more network/parser functions there would tip it over.

use std::collections::HashMap;

use serde::Deserialize;

use crate::error::{AniError, Result};

const ANILIST_API: &str = "https://graphql.anilist.co";

/// By-MAL-id query for AniList's `streamingEpisodes` — the
/// Crunchyroll-style per-episode listing. Field projection is the
/// minimum the parser needs: title (for the "Episode N" prefix) and
/// thumbnail (the URL itself).
const STREAMING_EPS_BY_MAL_GQL: &str = "query StreamingEpsByMal($idMal: Int!) { \
        Media(idMal: $idMal, type: ANIME) { streamingEpisodes { title thumbnail } } \
    }";

/// Fetch the list of `streamingEpisodes` AniList has for a show
/// identified by its MyAnimeList id. Each entry yields a
/// `(episode_number, thumbnail_url)` pair; the parser drops any
/// entry whose `title` doesn't carry an integer "Episode N" prefix
/// (movies, OVAs, half-episode recaps) or whose thumbnail is null.
///
/// Returns an empty `Vec` when AniList has no media for the supplied
/// MAL id, or when the media exists but `streamingEpisodes` is null.
///
/// # Errors
/// - [`AniError::Network`] on connection failure.
/// - [`AniError::Upstream`] on non-2xx HTTP.
/// - [`AniError::ParseFailed`] when the response shape is wrong.
pub async fn streaming_episodes_for_mal_id(
    client: &reqwest::Client,
    mal_id: u32,
    base_override: Option<&str>,
) -> Result<Vec<(u32, String)>> {
    let url = base_override.unwrap_or(ANILIST_API);
    let body = serde_json::json!({
        "query": STREAMING_EPS_BY_MAL_GQL,
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
    parse_streaming_episodes_response(&bytes)
}

/// Convenience wrapper over [`streaming_episodes_for_mal_id`] that
/// dedups the pair list into a `HashMap<u32, String>` (ep_number →
/// thumbnail URL) with first-wins semantics — matches the natural
/// ordering AniList returns when a show has multiple language tracks
/// listed for the same episode.
///
/// # Errors
/// Same as [`streaming_episodes_for_mal_id`].
pub async fn streaming_eps_map_for_mal_id(
    client: &reqwest::Client,
    mal_id: u32,
    base_override: Option<&str>,
) -> Result<HashMap<u32, String>> {
    let pairs = streaming_episodes_for_mal_id(client, mal_id, base_override).await?;
    let mut map = HashMap::with_capacity(pairs.len());
    for (n, url) in pairs {
        map.entry(n).or_insert(url);
    }
    Ok(map)
}

/// Pure parser for the `streamingEpisodes` response body.
///
/// Filters in one pass: entries with a null thumbnail are dropped,
/// then entries whose title doesn't yield an integer episode number
/// via [`extract_integer_episode_number`] are dropped. Order is
/// preserved — callers downstream pick first-wins on collisions.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the
/// expected `{ data: { Media: { streamingEpisodes: [...] } } }`
/// envelope.
pub fn parse_streaming_episodes_response(body: &[u8]) -> Result<Vec<(u32, String)>> {
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
        #[serde(default, rename = "streamingEpisodes")]
        streaming_episodes: Option<Vec<StreamingEpisode>>,
    }
    #[derive(Deserialize)]
    struct StreamingEpisode {
        title: Option<String>,
        thumbnail: Option<String>,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist streamingEpisodes response: {e}"),
    })?;
    let eps = parsed
        .data
        .media
        .and_then(|m| m.streaming_episodes)
        .unwrap_or_default();
    let mut out = Vec::with_capacity(eps.len());
    for e in eps {
        let Some(thumb) = e.thumbnail else { continue };
        let Some(title) = e.title.as_deref() else {
            continue;
        };
        let Some(num) = extract_integer_episode_number(title) else {
            continue;
        };
        out.push((num, thumb));
    }
    Ok(out)
}

/// Extract the integer episode number from an AniList streaming-
/// episode title like `"Episode 1 - …"`. Returns `None` for titles
/// that don't start with `"Episode N"` (OVAs, movies, specials) and
/// for half-episode recap titles like `"Episode 1061.5 - …"` — Kitsu's
/// `KitsuEpisode::number` is `Option<u32>`, so half-eps can't merge.
///
/// Case-insensitive on the literal prefix. Whitespace-trimmed on the
/// input.
fn extract_integer_episode_number(title: &str) -> Option<u32> {
    let trimmed = title.trim();
    let rest = trimmed
        .strip_prefix("Episode ")
        .or_else(|| trimmed.strip_prefix("episode "))
        .or_else(|| trimmed.strip_prefix("EPISODE "))?;
    let digits_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if digits_end == 0 {
        return None;
    }
    let after = rest[digits_end..].chars().next();
    if after == Some('.') {
        return None;
    }
    rest[..digits_end].parse::<u32>().ok()
}

#[cfg(test)]
#[path = "anilist_streaming_eps_test.rs"]
mod tests;
