//! The batched airing queries: many shows' [`AiringStatus`] per
//! request, by AniList id or by MAL id, for the home rail's seed. Split
//! from [`super`] so each file stays inside the CRAP gate's per-file
//! bar.

use serde::Deserialize;

use super::{airing_status, derive_status, AiringStatus, MediaShape};
use crate::error::{AniError, Result};
use crate::meta::anilist::{post_graphql_public, ANILIST_API};

/// Batch variant of [`super::AIRING_GQL`]: one `Page` request answers
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
    // Every field here is one the batch asks for: a page that leaves one
    // out is no answer, never an empty or whole one.
    #[derive(Deserialize)]
    struct Page {
        #[serde(rename = "pageInfo")]
        info: Info,
        media: Vec<serde_json::Value>,
    }
    #[derive(Deserialize)]
    struct Info {
        #[serde(rename = "hasNextPage")]
        has_next: bool,
    }
    let failed = |detail: String| AniError::ParseFailed {
        detail: format!("anilist airing batch response: {detail}"),
    };
    let parsed: Wrap = serde_json::from_slice(body).map_err(|e| failed(e.to_string()))?;
    let page = parsed.data.page;
    let key_name = if by_mal { "idMal" } else { "id" };
    let mut media = Vec::with_capacity(page.media.len());
    for node in page.media {
        // A node that leaves out the key it was asked for cannot say whose
        // schedule it is; one whose key is null names no show.
        if node.get(key_name).is_none() {
            return Err(failed(format!("a node without {key_name}")));
        }
        let m: MediaShape = serde_json::from_value(node).map_err(|e| failed(e.to_string()))?;
        let key = if by_mal { m.id_mal } else { m.id };
        if let Some(k) = key {
            media.push((k, derive_status(m)));
        }
    }
    Ok(BatchPage {
        cut: page.info.has_next,
        media,
    })
}

#[cfg(test)]
#[path = "anilist_airing_batch_prop_test.rs"]
mod prop_tests;
