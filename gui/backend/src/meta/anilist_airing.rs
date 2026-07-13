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
    /// Episode number as AniList counts it (regular episodes only).
    pub episode: u32,
    /// Epoch seconds of this episode's scheduled airing.
    pub airing_at: u64,
}

/// Airing-schedule query for one show, addressable by EITHER id
/// space: pass `id` (AniList) or `idMal` and omit the other — the
/// detail page reaches shows Kitsu hasn't MAL-mapped through the
/// direct anilist mapping, so both routes must work.
const AIRING_GQL: &str = "query Airing($id: Int, $idMal: Int) { \
        Media(id: $id, idMal: $idMal, type: ANIME) { \
            status episodes nextAiringEpisode { episode airingAt } \
            airingSchedule(notYetAired: true, perPage: 25) { \
                nodes { episode airingAt } \
            } \
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

/// Batch variant of [`AIRING_GQL`]: one `Page` request answers
/// airing for a whole rail. Includes `id` so the response maps back
/// to the requested shows.
const AIRING_BATCH_GQL: &str = "query AiringBatch($ids: [Int]) { \
        Page(page: 1, perPage: 50) { \
            media(id_in: $ids, type: ANIME) { \
                id status episodes nextAiringEpisode { episode airingAt } \
                airingSchedule(notYetAired: true, perPage: 25) { \
                    nodes { episode airingAt } \
                } \
            } \
        } \
    }";

/// Fetch [`AiringStatus`] for many AniList ids in as few requests as
/// possible (chunks of 50 — the `Page` cap). Ids AniList doesn't
/// return are simply absent from the map. Empty input skips the
/// network entirely.
///
/// # Errors
/// Network / Upstream / ParseFailed from the underlying client.
pub async fn airing_status_batch(
    client: &reqwest::Client,
    anilist_ids: &[u32],
    base_override: Option<&str>,
) -> Result<std::collections::HashMap<u32, AiringStatus>> {
    let url = base_override.unwrap_or(ANILIST_API);
    let mut out = std::collections::HashMap::with_capacity(anilist_ids.len());
    for chunk in anilist_ids.chunks(50) {
        let body = serde_json::json!({
            "query": AIRING_BATCH_GQL,
            "variables": { "ids": chunk },
        });
        let bytes = post_graphql_public(client, url, &body).await?;
        out.extend(parse_airing_batch_response(&bytes)?);
    }
    Ok(out)
}

/// Raw serde shape of one `Media` node, shared by the single and
/// batch parsers. `id` is only present in the batch query's
/// selection set, hence the default.
#[derive(Deserialize)]
struct MediaShape {
    #[serde(default)]
    id: Option<u32>,
    status: Option<String>,
    episodes: Option<u32>,
    #[serde(rename = "nextAiringEpisode")]
    next_airing_episode: Option<ScheduleNode>,
    #[serde(rename = "airingSchedule", default)]
    airing_schedule: Option<Schedule>,
}

#[derive(Deserialize)]
struct Schedule {
    #[serde(default)]
    nodes: Vec<ScheduleNode>,
}

#[derive(Deserialize)]
struct ScheduleNode {
    episode: u32,
    #[serde(rename = "airingAt")]
    airing_at: u64,
}

/// Derivation shared by both parsers:
///   - a scheduled `nextAiringEpisode` → aired = episode - 1;
///   - else `FINISHED` → aired = the announced total (None when
///     AniList doesn't know it — stays ungated);
///   - else `NOT_YET_RELEASED` → aired = 0;
///   - else (releasing without schedule data, hiatus, cancelled) →
///     aired = None, deliberately: gating on a guess would hide real
///     episodes.
fn derive_status(media: MediaShape) -> AiringStatus {
    let upcoming: Vec<UpcomingEpisode> = media
        .airing_schedule
        .map(|s| {
            s.nodes
                .into_iter()
                .map(|n| UpcomingEpisode {
                    episode: n.episode,
                    airing_at: n.airing_at,
                })
                .collect()
        })
        .unwrap_or_default();
    if let Some(next) = media.next_airing_episode {
        AiringStatus {
            aired: Some(next.episode.saturating_sub(1)),
            next_episode: Some(next.episode),
            next_airing_at: Some(next.airing_at),
            upcoming,
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
            upcoming,
        }
    }
}

/// Pure parser + derivation for the single-show airing response.
/// `Media: null` → `Ok(None)`. Derivation rules live in
/// [`derive_status`].
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
        media: Option<MediaShape>,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist airing response: {e}"),
    })?;
    Ok(parsed.data.media.map(derive_status))
}

/// Pure parser for the batch response: every returned media node
/// with an `id` becomes a map entry via [`derive_status`].
///
/// # Errors
/// Returns [`AniError::ParseFailed`] when the body isn't the expected
/// envelope.
pub fn parse_airing_batch_response(
    body: &[u8],
) -> Result<std::collections::HashMap<u32, AiringStatus>> {
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
        #[serde(default)]
        media: Vec<MediaShape>,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist airing batch response: {e}"),
    })?;
    Ok(parsed
        .data
        .page
        .media
        .into_iter()
        .filter_map(|m| m.id.map(|id| (id, derive_status(m))))
        .collect())
}

#[cfg(test)]
#[path = "anilist_airing_test.rs"]
mod tests;
