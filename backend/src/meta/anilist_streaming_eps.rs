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

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::{AniError, Result};
use crate::meta::anilist_media::{graphql_body, media_is_absent, with_missing_retry, MediaRef};

#[path = "anilist_streaming_eps_parse.rs"]
mod parse;
pub use parse::parse_streaming_episodes_response;

const ANILIST_API: &str = "https://graphql.anilist.co";

/// By-MAL-id query for AniList's `streamingEpisodes` — the
/// Crunchyroll-style per-episode listing — with the media's own
/// episode count and airing status beside it, which say whether the
/// listing can describe this show at all.
const STREAMING_EPS_BY_MAL_GQL: &str = "query StreamingEpsByMal($idMal: Int!) { \
        Media(idMal: $idMal, type: ANIME) { episodes status streamingEpisodes { title thumbnail } } \
    }";

/// [`STREAMING_EPS_BY_MAL_GQL`] keyed by AniList's own id, for shows
/// Kitsu maps to AniList but not yet to MAL.
const STREAMING_EPS_BY_ID_GQL: &str = "query StreamingEpsById($id: Int!) { \
        Media(id: $id, type: ANIME) { episodes status streamingEpisodes { title thumbnail } } \
    }";

/// One numbered entry of a streaming listing: the episode's title as
/// the listing words it (the `Episode N - ` prefix removed) and its
/// still.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamingEntry {
    /// The entry's title after its `Episode N` prefix.
    pub title: String,
    /// The still's URL.
    pub thumbnail: String,
}

/// What AniList lists as a show's streaming episodes, keyed by the
/// episode number each entry's title carries (first entry wins), with
/// the episode count and status AniList records for the same media.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamingListing {
    /// AniList's `episodes`: the show's episode count, `None` while
    /// unknown (typically an ongoing show).
    pub episodes: Option<u32>,
    /// AniList's `status` is `FINISHED`.
    pub finished: bool,
    /// Numbered entries, by episode number.
    pub entries: BTreeMap<u32, StreamingEntry>,
}

/// Shared POST + parse behind both `streamingEpisodes` queries.
/// `None` when AniList has no such media.
async fn post_streaming_episodes(
    client: &reqwest::Client,
    body: &serde_json::Value,
    base_override: Option<&str>,
) -> Result<Option<StreamingListing>> {
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

/// The streaming listing for a show identified either way — see
/// [`MediaRef`]. Empty when AniList has no such media.
///
/// # Errors
/// - [`AniError::Network`] on connection failure.
/// - [`AniError::Upstream`] on non-2xx HTTP.
/// - [`AniError::ParseFailed`] when the response shape is wrong.
pub async fn streaming_listing_for_media(
    client: &reqwest::Client,
    media: MediaRef,
    base_override: Option<&str>,
) -> Result<StreamingListing> {
    Ok(streaming_eps_lookup(client, media, base_override)
        .await?
        .unwrap_or_default())
}

/// The streaming listing by whichever ids Kitsu's mappings carry:
/// the MAL id first, AniList's own id when there is no MAL id or
/// AniList does not index the MAL one. Empty when neither id is known
/// or no lookup finds the media.
///
/// # Errors
/// Same as [`streaming_listing_for_media`], from whichever lookup
/// failed.
pub async fn streaming_listing_for_ids(
    client: &reqwest::Client,
    mal: Option<u32>,
    anilist: Option<u32>,
    base_override: Option<&str>,
) -> Result<StreamingListing> {
    let found = with_missing_retry(mal, anilist, |media| {
        streaming_eps_lookup(client, media, base_override)
    })
    .await?;
    Ok(found.unwrap_or_default())
}

/// One `streamingEpisodes` query. `None` when AniList has no such
/// media.
async fn streaming_eps_lookup(
    client: &reqwest::Client,
    media: MediaRef,
    base_override: Option<&str>,
) -> Result<Option<StreamingListing>> {
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
    post_streaming_episodes(client, &body, base_override).await
}

#[cfg(test)]
#[path = "anilist_streaming_eps_test.rs"]
mod tests;
