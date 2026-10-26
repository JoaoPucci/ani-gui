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
        Page(page: 1, perPage: 50) { pageInfo { hasNextPage } \
            media(id_in: $ids, type: ANIME) { \
                id status episodes nextAiringEpisode { episode airingAt } \
                airingSchedule(notYetAired: true, perPage: 25) { \
                    nodes { episode airingAt } \
                } \
            } \
        } \
    }";

/// [`AIRING_BATCH_GQL`] addressed by MAL id, for shows Kitsu maps to
/// MAL alone. A separate request rather than a second filter on the
/// same `media` field: AniList ANDs `id_in` with `idMal_in`. Selects
/// `idMal` so the response maps back.
const AIRING_BATCH_BY_MAL_GQL: &str = "query AiringBatchByMal($malIds: [Int]) { \
        Page(page: 1, perPage: 50) { pageInfo { hasNextPage } \
            media(idMal_in: $malIds, type: ANIME) { \
                idMal status episodes nextAiringEpisode { episode airingAt } \
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
    fetch_batch(client, BatchIds::AniList(anilist_ids), base_override).await
}

/// [`airing_status_batch`] keyed by MAL id: the map is keyed by the
/// MAL ids AniList returned. Several media can record one MAL id: such
/// an id is answered by the single lookup by MAL id
/// ([`airing_status`]), as the detail page asks it, never by whichever
/// node the page listed last — and left out when that lookup fails. A
/// page AniList cut (`hasNextPage`) is split and asked again.
///
/// # Errors
/// As [`airing_status_batch`].
pub async fn airing_status_batch_by_mal(
    client: &reqwest::Client,
    mal_ids: &[u32],
    base_override: Option<&str>,
) -> Result<std::collections::HashMap<u32, AiringStatus>> {
    fetch_batch(client, BatchIds::Mal(mal_ids), base_override).await
}

/// Which id space a batch request addresses.
#[derive(Clone, Copy)]
enum BatchIds<'a> {
    AniList(&'a [u32]),
    Mal(&'a [u32]),
}

async fn fetch_batch(
    client: &reqwest::Client,
    ids: BatchIds<'_>,
    base_override: Option<&str>,
) -> Result<std::collections::HashMap<u32, AiringStatus>> {
    let url = base_override.unwrap_or(ANILIST_API);
    let (all, query, var, by_mal) = match ids {
        BatchIds::AniList(ids) => (ids, AIRING_BATCH_GQL, "ids", false),
        BatchIds::Mal(ids) => (ids, AIRING_BATCH_BY_MAL_GQL, "malIds", true),
    };
    let mut out = std::collections::HashMap::with_capacity(all.len());
    let mut singly: Vec<u32> = Vec::new();
    let mut chunks: Vec<&[u32]> = all.chunks(50).rev().collect();
    while let Some(chunk) = chunks.pop() {
        let body = serde_json::json!({
            "query": query,
            "variables": { var: chunk },
        });
        let bytes = post_graphql_public(client, url, &body).await?;
        let page = parse_batch(&bytes, by_mal)?;
        if page.cut {
            match chunk.len() {
                1 => singly.extend_from_slice(chunk),
                n => {
                    let (first, rest) = chunk.split_at(n / 2);
                    chunks.push(rest);
                    chunks.push(first);
                }
            }
            continue;
        }
        let (once, several) = page.by_key();
        out.extend(once);
        singly.extend(several);
    }
    singly.sort_unstable();
    singly.dedup();
    for key in singly {
        let (anilist_id, mal_id) = if by_mal {
            (None, Some(key))
        } else {
            (Some(key), None)
        };
        match airing_status(client, anilist_id, mal_id, base_override).await {
            Ok(Some(status)) => {
                out.insert(key, status);
            }
            Ok(None) => {}
            Err(err) => tracing::debug!(key, error = ?err, "anilist airing: single lookup failed"),
        }
    }
    Ok(out)
}

/// One page of a batch answer: each media node's key (`idMal` or `id`)
/// and status, and whether AniList cut the page.
struct BatchPage {
    media: Vec<(u32, AiringStatus)>,
    cut: bool,
}

impl BatchPage {
    /// The keys one node answers, with their status, and the keys
    /// several nodes answer, which only a single lookup can settle.
    fn by_key(self) -> (std::collections::HashMap<u32, AiringStatus>, Vec<u32>) {
        let mut once = std::collections::HashMap::new();
        let mut several = Vec::new();
        for (key, status) in self.media {
            if several.contains(&key) {
                continue;
            }
            if once.insert(key, status).is_some() {
                once.remove(&key);
                several.push(key);
            }
        }
        (once, several)
    }
}

/// Raw serde shape of one `Media` node, shared by the single and
/// batch parsers. `id` and `idMal` are only present in the batch
/// queries' selection sets, hence the defaults.
#[derive(Deserialize)]
struct MediaShape {
    #[serde(default)]
    id: Option<u32>,
    #[serde(rename = "idMal", default)]
    id_mal: Option<u32>,
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
    Ok(parse_batch(body, false)?.media.into_iter().collect())
}

/// Batch parser keyed by `idMal` (`by_mal`) or `id`, in page order.
fn parse_batch(body: &[u8], by_mal: bool) -> Result<BatchPage> {
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
        #[serde(rename = "pageInfo", default)]
        info: Option<Info>,
        #[serde(default)]
        media: Vec<MediaShape>,
    }
    #[derive(Deserialize)]
    struct Info {
        #[serde(rename = "hasNextPage", default)]
        has_next: Option<bool>,
    }
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| AniError::ParseFailed {
        detail: format!("anilist airing batch response: {e}"),
    })?;
    let page = parsed.data.page;
    Ok(BatchPage {
        cut: page.info.and_then(|i| i.has_next).unwrap_or(false),
        media: page
            .media
            .into_iter()
            .filter_map(|m| {
                let key = if by_mal { m.id_mal } else { m.id };
                key.map(|k| (k, derive_status(m)))
            })
            .collect(),
    })
}

#[cfg(test)]
#[path = "anilist_airing_test.rs"]
mod tests;
