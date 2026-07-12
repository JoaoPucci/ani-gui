//! AniList airing-schedule client — the aired-count signal behind
//! the detail page's unaired episode placeholders. Split from
//! `anilist.rs` (same pattern as `anilist_streaming_eps.rs`) so the
//! feature's complexity doesn't count against that file's CRAP
//! budget; the shared GraphQL POST helper stays in `anilist.rs`.

use serde::{Deserialize, Serialize};

use super::anilist::{post_graphql_public, ANILIST_API};
use crate::error::{AniError, Result};

/// Airing progress for a show as AniList schedules it. Drives the
/// detail page's unaired-episode placeholders: tiles past `aired`
/// render greyed instead of clickable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AiringStatus {
    /// Episodes aired so far, when derivable: `nextAiringEpisode - 1`
    /// while releasing, the announced total once finished, `0` before
    /// premiere. `None` = unknown (no schedule data) — callers must
    /// NOT gate episodes on unknown.
    pub aired: Option<u32>,
    /// Number of the next episode to air, when one is scheduled.
    pub next_episode: Option<u32>,
    /// Epoch seconds of the next airing, when one is scheduled.
    pub next_airing_at: Option<u64>,
    /// Per-episode air dates for the announced future schedule, so
    /// every unaired tile can label itself (weekly shows publish a
    /// few weeks ahead; episodes past the published window simply
    /// have no row here). `serde(default)` tolerates payloads and
    /// cache rows written before the field existed.
    #[serde(default)]
    pub upcoming: Vec<UpcomingEpisode>,
}

/// One future schedule row: episode number + its air time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UpcomingEpisode {
    pub episode: u32,
    pub airing_at: u64,
}

/// Airing-schedule query for one show, addressable by EITHER id
/// space: pass `id` (AniList) or `idMal` and omit the other — the
/// detail page reaches shows Kitsu hasn't MAL-mapped through the
/// direct anilist mapping, so both routes must work.
const AIRING_GQL: &str = "query Airing($id: Int, $idMal: Int) { \
        Media(id: $id, idMal: $idMal, type: ANIME) { \
            status episodes nextAiringEpisode { episode airingAt } \
        } \
    }";

/// Fetch a show's [`AiringStatus`] by AniList id (preferred) or MAL
/// id. `Ok(None)` when AniList doesn't index the show or neither id
/// is supplied.
///
/// # Errors
/// Network / Upstream / ParseFailed — same as [`media_id_for_mal`].
pub async fn airing_status(
    client: &reqwest::Client,
    anilist_id: Option<u32>,
    mal_id: Option<u32>,
    base_override: Option<&str>,
) -> Result<Option<AiringStatus>> {
    let variables = match (anilist_id, mal_id) {
        (Some(id), _) => serde_json::json!({ "id": id }),
        (None, Some(mal)) => serde_json::json!({ "idMal": mal }),
        (None, None) => return Ok(None),
    };
    let url = base_override.unwrap_or(ANILIST_API);
    let body = serde_json::json!({ "query": AIRING_GQL, "variables": variables });
    let bytes = post_graphql_public(client, url, &body).await?;
    parse_airing_response(&bytes)
}

/// Pure parser + derivation for the airing response. `Media: null` →
/// `Ok(None)`. Derivation:
///   - a scheduled `nextAiringEpisode` → aired = episode - 1;
///   - else `FINISHED` → aired = the announced total (None when
///     AniList doesn't know it — stays ungated);
///   - else `NOT_YET_RELEASED` → aired = 0;
///   - else (releasing without schedule data, hiatus, cancelled) →
///     aired = None, deliberately: gating on a guess would hide real
///     episodes.
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the expected
/// envelope.
pub fn parse_airing_response(body: &[u8]) -> Result<Option<AiringStatus>> {
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
        status: Option<String>,
        episodes: Option<u32>,
        #[serde(rename = "nextAiringEpisode")]
        next_airing_episode: Option<NextAiring>,
    }
    #[derive(Deserialize)]
    struct NextAiring {
        episode: u32,
        #[serde(rename = "airingAt")]
        airing_at: u64,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist airing response: {e}"),
    })?;
    let Some(media) = parsed.data.media else {
        return Ok(None);
    };
    let status = if let Some(next) = media.next_airing_episode {
        AiringStatus {
            aired: Some(next.episode.saturating_sub(1)),
            next_episode: Some(next.episode),
            next_airing_at: Some(next.airing_at),
            // Red stub: the green commit extracts airingSchedule here.
            upcoming: Vec::new(),
        }
    } else {
        let aired = match media.status.as_deref() {
            Some("FINISHED") => media.episodes,
            Some("NOT_YET_RELEASED") => Some(0),
            _ => None,
        };
        AiringStatus {
            aired,
            next_episode: None,
            next_airing_at: None,
            upcoming: Vec::new(),
        }
    };
    Ok(Some(status))
}

#[cfg(test)]
#[path = "anilist_airing_test.rs"]
mod tests;
