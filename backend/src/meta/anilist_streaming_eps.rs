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
use crate::meta::anilist_media::{graphql_body, media_is_absent, with_missing_retry, MediaRef};

#[path = "anilist_streaming_eps_parse.rs"]
mod parse;
pub use parse::parse_streaming_episodes_response;

const ANILIST_API: &str = "https://graphql.anilist.co";

/// By-MAL-id query for AniList's `streamingEpisodes` — the
/// Crunchyroll-style per-episode listing. Field projection is the
/// minimum the parser needs: title (for the "Episode N" prefix) and
/// thumbnail (the URL itself).
const STREAMING_EPS_BY_MAL_GQL: &str = "query StreamingEpsByMal($idMal: Int!) { \
        Media(idMal: $idMal, type: ANIME) { streamingEpisodes { title thumbnail } } \
    }";

/// [`STREAMING_EPS_BY_MAL_GQL`] keyed by AniList's own id, for shows
/// Kitsu maps to AniList but not yet to MAL.
const STREAMING_EPS_BY_ID_GQL: &str = "query StreamingEpsById($id: Int!) { \
        Media(id: $id, type: ANIME) { streamingEpisodes { title thumbnail } } \
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
    let body = serde_json::json!({
        "query": STREAMING_EPS_BY_MAL_GQL,
        "variables": { "idMal": mal_id },
    });
    Ok(post_streaming_episodes(client, &body, base_override)
        .await?
        .unwrap_or_default())
}

/// Shared POST + parse behind both `streamingEpisodes` queries.
/// `None` when AniList has no such media.
async fn post_streaming_episodes(
    client: &reqwest::Client,
    body: &serde_json::Value,
    base_override: Option<&str>,
) -> Result<Option<Vec<(u32, String)>>> {
    let url = base_override.unwrap_or(ANILIST_API);
    let resp = client
        .post(url)
        .header(
            "user-agent",
            "ani-gui/0.1 (https://github.com/pucci/ani-gui)",
        )
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|_| AniError::Network)?;
    let bytes = graphql_body(resp).await?;
    if media_is_absent(&bytes) {
        return Ok(None);
    }
    parse_streaming_episodes_response(&bytes).map(Some)
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
    Ok(dedup_first_wins(pairs))
}

/// `(ep_number, url)` pairs → map, keeping the first URL per episode.
fn dedup_first_wins(pairs: Vec<(u32, String)>) -> HashMap<u32, String> {
    let mut map = HashMap::with_capacity(pairs.len());
    for (n, url) in pairs {
        map.entry(n).or_insert(url);
    }
    map
}

/// [`streaming_eps_map_for_mal_id`] for a show identified either
/// way — see [`MediaRef`].
///
/// # Errors
/// Same as [`streaming_episodes_for_mal_id`].
pub async fn streaming_eps_map_for_media(
    client: &reqwest::Client,
    media: MediaRef,
    base_override: Option<&str>,
) -> Result<HashMap<u32, String>> {
    Ok(streaming_eps_lookup(client, media, base_override)
        .await?
        .unwrap_or_default())
}

/// The episode-thumbnail map by whichever ids Kitsu's mappings carry:
/// the MAL id first, AniList's own id when there is no MAL id or
/// AniList does not index the MAL one. Empty when neither id is known
/// or no lookup finds the media.
///
/// # Errors
/// Same as [`streaming_episodes_for_mal_id`], from whichever lookup
/// failed.
pub async fn streaming_eps_map_for_ids(
    client: &reqwest::Client,
    mal: Option<u32>,
    anilist: Option<u32>,
    base_override: Option<&str>,
) -> Result<HashMap<u32, String>> {
    let found = with_missing_retry(mal, anilist, |media| {
        streaming_eps_lookup(client, media, base_override)
    })
    .await?;
    Ok(found.unwrap_or_default())
}

/// One `streamingEpisodes` query, deduped. `None` when AniList has no
/// such media.
async fn streaming_eps_lookup(
    client: &reqwest::Client,
    media: MediaRef,
    base_override: Option<&str>,
) -> Result<Option<HashMap<u32, String>>> {
    let body = match media {
        MediaRef::Mal(mal_id) => serde_json::json!({
            "query": STREAMING_EPS_BY_MAL_GQL,
            "variables": { "idMal": mal_id },
        }),
        MediaRef::AniList(id) => serde_json::json!({
            "query": STREAMING_EPS_BY_ID_GQL,
            "variables": { "id": id },
        }),
    };
    let pairs = post_streaming_episodes(client, &body, base_override).await?;
    Ok(pairs.map(dedup_first_wins))
}

#[cfg(test)]
#[path = "anilist_streaming_eps_test.rs"]
mod tests;
